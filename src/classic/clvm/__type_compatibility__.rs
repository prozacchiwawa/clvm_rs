use std::cmp::min;
use std::cmp::Ordering;
use std::option::Option;
use std::string::String;

use bls12_381::G1Affine;
use hex;
use sha2::Sha256;
use sha2::Digest;

//import {Word32Array} from "jscrypto/Word32Array";
//import {SHA256} from "jscrypto/SHA256";

pub fn to_hexstr(r: &Vec<u8>) -> String {
    return hex::encode(r);
}

pub fn char_to_string(ch : char) -> String {
    match String::from_utf8(vec!(ch as u8)) {
        Ok(s) => s,
        _ => String::new()
    }
}

pub fn vec_to_string(r: &Vec<u8>) -> String {
    return String::from_utf8_lossy(r).as_ref().to_string();
}

/**
 * Get python's bytes.__repr__ style string.
 * @see https://github.com/python/cpython/blob/main/Objects/bytesobject.c#L1337
 * @param {Uint8Array} r - byteArray to stringify
 */
pub fn PyBytes_Repr(r : &Vec<u8>) -> String {
    let mut squotes = 0;
    let mut dquotes = 0;
    for i in 0..r.len() - 1 {
        let b = r[i];
        let c = b as char;
        match c {
            '\'' => squotes += 1,
            '\"' => dquotes += 1,
            _ => ()
        }
    }
    let mut quote = '\'';
    if squotes > 0 && dquotes == 0 {
        quote = '\"';
    }

    let mut s = "b".to_string() + char_to_string(quote).as_str();

    for i in 0..r.len() - 1 {
        let b = r[i];
        let c = b as char;
        if c == quote || c == '\\' {
            s = (s + "\\") + char_to_string(c).as_str();
        }
        else if c == '\t' {
            s += "\\t";
        }
        else if c == '\n' {
            s += "\\n";
        }
        else if c == '\r' {
            s += "\\r";
        }
        else if c < ' ' || b >= 0x7f {
            s += "\\x";
            s += hex::encode(vec!(b)).as_str();
        }
        else{
            s += char_to_string(c).as_str();
        }
    }

    s += char_to_string(quote).as_str();

    return s;
}

pub enum BytesFromType {
    Hex(String),
    Raw(Vec<u8>),
    G1Element(G1Affine)
}

pub struct Bytes {
    _b : Vec<u8>
}

pub fn ordering_to_int(o : Ordering) -> i32 {
    match o {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1
    }
}

/**
 * Unlike python, there is no immutable byte type in javascript.
 */
impl Bytes {
    fn new(value : Option<BytesFromType>) -> Self {
        match value {
            None => Bytes { _b: vec!() },
            Some(BytesFromType::Raw(v)) => Bytes { _b: v },
            Some(BytesFromType::Hex(hstr)) => {
                match hex::decode(hstr) {
                    Ok(d) => Bytes { _b: d },
                    _ => Bytes { _b: vec!() }
                }
            },
            Some(BytesFromType::G1Element(g1)) => Bytes { _b: g1.to_uncompressed().to_vec() },
        }
    }

    fn length(&self) -> usize {
        return self._b.len();
    }

    fn at(&self, i: usize) -> u8 {
        return self._b[i];
    }

    fn raw(&self) -> Vec<u8> {
        return self._b.clone();
    }

    fn repeat(&self, n : usize) -> Bytes {
        let capacity = self.length() * n;
        let set_size = self._b.len();
        let mut ret = Vec::<u8>::with_capacity(capacity);
        for i in 0..capacity - 1 {
            ret[i] = self._b[i%set_size];
        }
        return Bytes::new(Some(BytesFromType::Raw(ret)));
    }

    fn concat(&self, b: &Bytes) -> Bytes {
        let mut thisBin = self._b.clone();
        let mut thatBin = b.raw();
        let mut concatBin = Vec::<u8>::with_capacity(thisBin.len() + thatBin.len());
        concatBin.append(&mut thisBin);
        concatBin.append(&mut thatBin);
        return Bytes::new(Some(BytesFromType::Raw(concatBin)));
    }

    fn slice(&self, start: usize, length: Option<usize>) -> Self {
        let len =
            match length {
                Some(x) => {
                    if self._b.len() > start + x {
                        x
                    } else {
                        self._b.len() - start
                    }
                },
                None => self._b.len() - start
            };
        let mut ui8_clone = Vec::<u8>::with_capacity(len);
        for i in start..start + len - 1 {
            ui8_clone.push(self._b[i]);
        }
        return Bytes::new(Some(BytesFromType::Raw(ui8_clone)));
    }

    fn subarray(&self, start: usize, length: Option<usize>) -> Self {
        return self.slice(start, length);
    }

    fn data(&self) -> &Vec<u8> {
        return &self._b;
    }

    fn clone(&self) -> Self {
        return Bytes::new(Some(BytesFromType::Raw(self._b.clone())));
    }

    fn toString(&self) -> String {
        return PyBytes_Repr(&self._b);
    }

    fn hex(&self) -> String {
        return to_hexstr(&self._b);
    }

    fn decode(&self) -> String {
        return vec_to_string(&self._b);
    }

    fn startswith(&self, b: &Bytes) -> bool {
        for i in 0..min(b.length(), self._b.len()) - 1 {
            if b.at(i) != self._b[i] {
                return false
            }
        }
        return true
    }

    fn endswith(&self, b: &Bytes) -> bool {
        let blen = min(b.length(), self._b.len()) - 1;
        for i in 0..blen {
            if b.at(blen - i) != self._b[blen - i] {
                return false
            }
        }
        return true
    }

    fn equal_to(&self, b: Bytes) -> bool {
        let slen = self._b.len();
        let blen = b.length();
        if slen != blen {
            return false;
        } else {
            for i in 0..slen {
                if b.at(i) != self._b[i] {
                    return false;
                }
            }
        }
        return true;
    }

    /**
     * Returns:
     *   +1 if argument is smaller
     *   0 if this and argument is the same
     *   -1 if argument is larger
     * @param other
     */
    fn compare(&self, other: Bytes) -> Ordering {
        let slen = min(self._b.len(), other.length());

        for i in 0..slen - 1 {
            let diff = other.at(i) - self._b[i];
            if diff < 0 {
                return Ordering::Less;
            } else if diff > 0 {
                return Ordering::Greater;
            }
        }
        if self._b.len() < slen {
            return Ordering::Less;
        } else if slen < other.length() {
            return Ordering::Greater;
        }
        return Ordering::Equal;
    }
}

fn SHA256(value: Bytes) -> Bytes {
    let hashed = Sha256::digest(&value.data()[..]);
    let hashed_iter = hashed.into_iter();
    let newvec : Vec<u8> = hashed_iter.collect();
    return Bytes::new(Some(BytesFromType::Raw(newvec)));
}

//   public static from(value?: Uint8Array|Bytes|number[]|string|G1Element|None, type?: BytesFromType){
//     if(value === None || value === undefined){
//       return new Bytes(value);
//     }
//     else if(value instanceof Uint8Array){
//       return new Bytes(value.slice());
//     }
//     else if(isBytes(value)){
//       return new Bytes(value.data());
//     }
//     else if(Array.isArray(value) && value.every(v => typeof v === "number")){
//       if(value.some(v => (v < 0 || v > 255))){
//         throw new Error("Bytes must be in range [0, 256)");
//       }
//       return new Bytes(Uint8Array.from(value));
//     }
//     else if(typeof value === "string"){
//       if(!value){
//         return new Bytes();
//       }
//       if(type === "hex"){
//         value = value.replace(/^0x/, "");
//         return new Bytes(Hex.parse(value).toUint8Array());
//       }
//       else /* if(type === "utf8") */ {
//         return new Bytes(Utf8.parse(value).toUint8Array());
//       }
//     }
//     else if(type === "G1Element"){
//       if(typeof (value as G1Element).serialize !== "function"){
//         throw new Error("Invalid G1Element");
//       }
//       const uint8array = (value as G1Element).serialize();
//       return new Bytes(uint8array);
//     }
    
//     throw new Error(`Invalid value: ${JSON.stringify(value)}`);
//   }

//   public as_word(){
//     return new Word32Array(this._b);
//   }
// }

// export function b(utf8Str: string, type:"utf8"|"hex" = "utf8"){
//   return Bytes.from(utf8Str, type);
// }

// export function h(hexStr: string){
//   return Bytes.from(hexStr, "hex");
// }

// export function list<T = unknown>(iterable: Iterable<T>){
//   const arr: T[] = [];
//   for(const item of iterable){
//     arr.push(item);
//   }
//   return arr;
// }

// export function str(x: any){
//   if(typeof x.toString === "function"){
//     return x.toString();
//   }
//   return `${x}`;
// }

// export function repr(x: any){
//   if(typeof x.__repr__ === "function"){
//     return x.__repr__();
//   }
//   return str(x);
// }

// export class Tuple<T1, T2> extends Array<any> {
//   public constructor(...items: [T1, T2]) {
//     super(...items);
//     Object.freeze(this);
//     return this;
//   }
  
//   public toString(){
//     return `(${this[0]}, ${this[1]})`;
//   }
// }

// export function t<T1, T2>(v1: T1, v2: T2){
//   return new Tuple(v1, v2);
// }

// export function isTuple(v: unknown): v is Tuple<unknown, unknown> {
//   return v instanceof Array && Object.isFrozen(v) && v.length === 2;
// }

// /**
//  * Check whether an argument is a list and not a tuple
//  */
// export function isList(v: unknown): v is unknown[] {
//   return Array.isArray(v) && !isTuple(v);
// }

// export function isIterable(v: any): v is unknown[] {
//   if(Array.isArray(v)){ // Including Tuple.
//     return true;
//   }
//   else if(typeof v === "string"){
//     return false;
//   }
//   else if(typeof v[Symbol.iterator] === "function"){
//     return true;
//   }
//   return false;
// }

// export function isBytes(v: any): v is Bytes {
//   return v && typeof v.length === "number"
//     && typeof v.at === "function"
//     && typeof v.raw === "function"
//     && typeof v.data === "function"
//     && typeof v.hex === "function"
//     && typeof v.decode === "function"
//     && typeof v.equal_to === "function"
//     && typeof v.compare === "function"
//   ;
// }

// export class Stream {
//   public static readonly INITIAL_BUFFER_SIZE = 64*1024;
//   private _seek: number;
//   private _length: number;
//   private _buffer: Uint8Array;
//   private _bufAllocMultiplier = 4;
  
//   public constructor(b?: Bytes) {
//     this._seek = 0;
    
//     if(b){
//       if(b.length > Stream.INITIAL_BUFFER_SIZE){
//         this._buffer = new Uint8Array(b.length*2);
//       }
//       else{
//         this._buffer = new Uint8Array(Stream.INITIAL_BUFFER_SIZE);
//       }
      
//       this._buffer.set(b.raw());
//       this._length = b.length;
//     }
//     else{
//       this._buffer = new Uint8Array(Stream.INITIAL_BUFFER_SIZE);
//       this._length = 0;
//     }
//   }
  
//   public get seek(){
//     return this._seek;
//   }
  
//   public set seek(value){
//     if(value < 0){
//       this._seek = this.length - 1;
//     }
//     else if(value > this.length - 1){
//       this._seek = this.length;
//     }
//     else{
//       this._seek = value;
//     }
//   }
  
//   public get length(){
//     return this._length;
//   }
  
//   protected reAllocate(size?: number){
//     let s = typeof size === "number" ? size : this._buffer.length * this._bufAllocMultiplier;
//     /**
//      * @see https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Errors/Invalid_array_length
//      */
//     if(s > 4294967295){ // 4294967295 = 2**32 - 1
//       s = 4294967295;
//     }
//     const buf = new Uint8Array(s);
//     buf.set(this._buffer);
//     this._buffer = buf;
//   }
  
//   public write(b: Bytes){
//     const newLength = Math.max(this.length, b.length + this._seek);
//     if(newLength > this._buffer.length){
//       this.reAllocate(newLength * this._bufAllocMultiplier);
//     }
    
//     const offset = this.seek;
//     this._buffer.set(b.raw(), offset);
    
//     this._length = newLength;
//     this.seek += b.length; // Don't move this line prior to `this._length = newLength`!
//     return b.length;
//   }
  
//   public read(size: number): Bytes {
//     if(this.seek > this.length-1){
//       return new Bytes(); // Return empty byte
//     }
    
//     if(this.seek + size <= this.length){
//       const u8 = this._buffer.subarray(this.seek, this.seek + size);
//       this.seek += size;
//       return new Bytes(u8);
//     }
    
//     const u8 = this._buffer.subarray(this.seek, this.length);
//     this.seek += size;
//     return new Bytes(u8);
//   }
  
//   public getValue(): Bytes {
//     return new Bytes(this._buffer.subarray(0, this.length));
//   }
// }

// /**
//  * Python's style division.
//  * In javascript, `-8 / 5 === -1` while `-8 / 5 == -2` in Python
//  */
// export function division(a: bigint, b: bigint): bigint {
//   if(a === BigInt(0)){
//     return a;
//   }
//   else if(b === BigInt(0)){
//     throw new Error("Division by zero!");
//   }
//   else if(a > BigInt(0) && b > BigInt(0) && a < b){
//     return BigInt(0);
//   }
//   else if(a < BigInt(0) && b < BigInt(0) && a > b){
//     return BigInt(0);
//   }
  
//   const div = a / b;
//   if(a === div*b){
//     return div;
//   }
//   else if(div > BigInt(0)){
//     return div;
//   }
//   return div - BigInt(1);
// }

// /**
//  * Python's style modulo.
//  * In javascript, `-8 % 5 === -3` while `-8 % 5 == 2` in Python
//  */
// export function modulo(a: bigint, b: bigint): bigint {
//   const div = division(a, b);
//   return a - b*div;
// }

// export function divmod(a: bigint, b: bigint): Tuple<bigint, bigint> {
//   const div = division(a, b);
//   return t(div, a - b*div);
// }
