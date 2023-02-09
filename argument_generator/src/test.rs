use hex::FromHex;

use rand::prelude::*;
use rand::Error;

use clvmr::allocator::{Allocator, NodePtr};
use clvmr::node::Node;
use clvmr::reduction::EvalErr;
use clvmr::serde::{node_from_bytes, node_to_bytes};

use crate::AllocatorSexpBuilder;
use crate::CollectArgumentStructure;

static INTERESTING_STRINGS: &[(&str, &[u8], &str)] = &[
    // Empty, produces nil
    ("41",
     &[],
     "80"
    ),
    ("41",
     &[0x01,0x85,0x00,0x14],
     "61"
    ),
    ("ffff41ff42ff43804480",
     &[],
     "ffff80ff80ff808080"
    ),
    ("ffff41ff42ff4380ff4480",
     &[0x01,0x85,0x00,0x014],
     "ffff61ff61ff6180ff6180"
    ),
    ("ffff41ff42ff4380ff4480",
     &[0x01,0x85,0x00,0x14,0x01,0xc5,0x00,0x24],
     "ffff71ff61ff7180ff6180"
    ),
    ("41",
     &[0x00,0x0a,0x00,0x0a,0x00,0x18],
     "ff8080"
    ),
    ("41",
     &[0x01, 0x85, 0x00, 0x07, 0x00, 0x03, 0x00, 0x10, 0x00, 0x30, 0x00, 0x7c],
     "ff61ff6180"
    ),
    // Random string, one argument
    ("41",
     &[0x1a,0x55,0xdf,0x26,0xe3,0xe6,0x40,0x1e,0x69,0x1c,0xe0,0x07,0xbc,0xfd,0xe2,0xf8],
     "ffffff8195ff8080ff8080ffffff8195ff8080ff8080ffffff8195ff8080ff8080ffffff8195ff8080ff8080ffffff8195ff8080ff808080"
    ),
    ("ffff41ff42ff4380ff4480",
     &[0x0a, 0xa2, 0x9e, 0x48, 0xf5, 0x39, 0x75, 0x48, 0x10, 0xf2, 0x01, 0x9b, 0x0a, 0x5b, 0x09, 0x02, 0xf5, 0xcd, 0xec, 0xc0],
     "ffff82734eff80ff8080ff82734e80"
    ),
    ("ffff41ff42ff4380ff4480",
     &[0x4c, 0xc7, 0x5e, 0xe5, 0x46, 0x8d, 0x36, 0xbd, 0xeb, 0x93, 0x10, 0x99, 0x94, 0x85, 0x5a, 0x00, 0x37, 0xa4, 0xb9, 0xd3],
     "ffff80ff84a321afb9ff8080ff84a321afb980"
    ),

];

fn produce_fuzz_args(
    allocator: &mut Allocator,
    prototype_hex: &str,
    input: &[u8]
) -> Result<NodePtr, EvalErr> {
    let nil = allocator.null();
    let prototype_bytes: Vec<u8> = <Vec<u8>>::from_hex(prototype_hex).unwrap();
    let prototype_node = node_from_bytes(allocator, &prototype_bytes).map_err(|_| EvalErr(nil, "bad conversion from bytes".to_string()))?;
    let mut builder = AllocatorSexpBuilder::new(allocator);
    let mut rng = FuzzPseudoRng::new(input);
    let cas: CollectArgumentStructure = rng.gen();
    cas.to_sexp(&mut builder, prototype_node)
}

#[test]
fn interesting_argument_generation_inputs() {
    for (args, input, wanted) in INTERESTING_STRINGS.iter() {
        let mut allocator = Allocator::new();
        let res = produce_fuzz_args(&mut allocator, args, input).expect("should work");
        let result_node = Node::new(&mut allocator, res);
        let result_bytes = node_to_bytes(&result_node).expect("should convert to bytes");
        let wanted_bytes = <Vec<u8>>::from_hex(wanted).unwrap();
        assert_eq!(result_bytes, wanted_bytes);
    }
}

// A pseudo RNG which uses a slice for all entropy.
pub struct FuzzPseudoRng<'slice> {
    slice: &'slice [u8],
    progress: usize,
}

impl<'slice> FuzzPseudoRng<'slice> {
    pub fn new(slice: &'slice [u8]) -> Self {
        FuzzPseudoRng { slice, progress: 0 }
    }

    fn next_u8_untreated(&mut self) -> u8 {
        if self.progress >= self.slice.len() {
            return 0;
        }
        let res = self.slice[self.progress];
        self.progress += 1;
        res
    }

    fn next_u32_untreated(&mut self) -> u32 {
        let mut result_u32: u32 = 0;
        for _ in 0..4 {
            result_u32 <<= 8;
            result_u32 |= self.next_u8_untreated() as u32;
        }
        result_u32
    }

    fn next_u64_untreated(&mut self) -> u64 {
        let result_u64: u64 = self.next_u32_untreated() as u64;
        result_u64 << 32 | self.next_u32_untreated() as u64
    }
}

impl<'slice> RngCore for FuzzPseudoRng<'slice> {
    #[inline(always)]
    fn next_u32(&mut self) -> u32 {
        self.next_u32_untreated()
    }

    #[inline(always)]
    fn next_u64(&mut self) -> u64 {
        self.next_u64_untreated()
    }

    #[inline(always)]
    #[allow(clippy::needless_range_loop)]
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for i in 0..dest.len() {
            dest[i] = self.next_u8_untreated()
        }
    }

    #[inline(always)]
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}
