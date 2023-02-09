#[cfg(any(test, feature="clvmr"))]
use clvmr::allocator::{Allocator, NodePtr, SExp};
#[cfg(any(test, feature="clvmr"))]
use clvmr::reduction::EvalErr;

//
// An argument producer for clvm programs.
// Given a target argument structure, use whatever bits we were given to
// produce arguments based a series of u32 reads from an rng interface.
//
// Use it like:
//
// let nil = allocator.null();
// let prototype_bytes: Vec<u8> = <Vec<u8>>::from_hex(prototype_hex).unwrap();
// let prototype_node = node_from_bytes(allocator, &prototype_bytes).map_err(|_| EvalErr(nil, "bad conversion from bytes".to_string()))?;
// let mut builder = AllocatorSexpBuilder::new(allocator);
// let mut rng = FuzzPseudoRng::new(input);
// let cas: CollectArgumentStructure = rng.gen();
// let result_sexp = cas.to_sexp(&mut builder, prototype_node).expect("should generate");
//
use rand::distributions::Standard;
use rand::prelude::*;
use rand::Rng;

#[cfg(test)]
pub mod test;

pub const DEFAULT_READ_MAX: usize = 512;

#[derive(Debug, Clone)]
pub enum SexpContent<T> {
    SexpTypeAtom(Vec<u8>),
    SexpTypeCons(T, T)
}

pub trait SexpBuilder<T,E> {
    fn destructure(&mut self, value: T) -> Result<SexpContent<T>, E>;
    fn make_cons(&mut self, some: T, other: T) -> Result<T, E>;
    fn make_nil(&mut self) -> T;
    fn make_atom(&mut self, bytes: &[u8]) -> Result<T, E>;
}

// Produce arguments for an arbitrary function given its argument form.
// Build values until a 0 word, then use a selector for each atom argument.
#[derive(Debug, Clone)]
pub struct CollectArgumentStructure {
    choices: Vec<usize>,
    atoms: Vec<usize>,
    conses: Vec<usize>,
    lists: Vec<usize>,
    read_max: usize
}

impl Default for CollectArgumentStructure {
    fn default() -> Self {
        // Set a useful default for read_max
        CollectArgumentStructure {
            choices: Default::default(),
            atoms: Default::default(),
            conses: Default::default(),
            lists: Default::default(),
            read_max: DEFAULT_READ_MAX,
        }
    }
}

impl CollectArgumentStructure {
    // Maximum number of forms to read if nothing in the stream suggests
    // termination.
    pub fn new(read_max: usize) -> Self {
        CollectArgumentStructure { read_max, .. Default::default() }
    }
}

#[derive(Debug, Default, Clone)]
enum AtomPath {
    #[default]
    Nil,
    AndByte(u8, usize)
}

#[derive(Debug, Clone)]
enum ConsContent {
    WithAtom(usize),
    WithCons(usize),
    WithList(usize)
}

#[derive(Default, Debug, Clone)]
enum ConsPath {
    #[default]
    Nil,
    Consed(ConsContent, ConsContent)
}

#[derive(Default, Debug, Clone)]
enum ListPath {
    #[default]
    Nil,
    Enlisted(ConsContent, usize)
}

#[derive(Debug, Clone)]
enum RevisitStack<T> {
    GenerateFor(Option<T>),
    GeneratePair,
    Choice(ConsContent),
}

// Internal structure used to keep state for sexp generation.
pub struct CollectArgumentStructureReader<'a> {
    choice: usize,

    cas: &'a CollectArgumentStructure,

    atoms: Vec<AtomPath>,
    conses: Vec<ConsPath>,
    lists: Vec<ListPath>,
}

impl<'a> CollectArgumentStructureReader<'a> {
    pub fn new(cas: &'a CollectArgumentStructure) -> Self {
        CollectArgumentStructureReader {
            choice: 0,
            cas,

            atoms: vec![AtomPath::Nil], // Start with a nil atom.
            conses: vec![ConsPath::Nil], // Start with a nil.
            lists: vec![ListPath::Nil], // Start with an empty list
        }
    }

    fn choose_with_default<T>(&self, lst: &[T], choice: usize, default: T) -> T
    where
        T: Clone,
    {
        if lst.is_empty() {
            return default;
        }

        lst[choice % lst.len()].clone()
    }

    fn get_choice(&mut self) -> Option<usize> {
        if self.cas.choices.is_empty() {
            return None;
        }

        let this_choice = self.cas.choices[self.choice % self.cas.choices.len()];
        self.choice += 1;
        Some(this_choice)
    }

    fn make_choice<T: std::fmt::Debug, E>(
        &mut self,
        do_stack: &mut Vec<RevisitStack<T>>,
        done: &mut Vec<T>,
        builder: &mut dyn SexpBuilder<T,E>,
        raw_choice: usize,
        level: usize
    ) -> Result<(),E> {
        let kind = raw_choice & 3;
        let choice = raw_choice >> 2;
        if kind > 2 && level >= kind {
            let mut use_list = choice;
            done.push(builder.make_nil());
            while let ListPath::Enlisted(a,b) =
                self.choose_with_default(&self.lists, use_list, ListPath::Nil)
            {
                use_list = b % use_list;
                do_stack.push(RevisitStack::GeneratePair);
                do_stack.push(RevisitStack::Choice(a));
            }
        } else if kind > 1 && level >= kind {
            if let ConsPath::Consed(a,b) =
                self.choose_with_default(&self.conses, choice, ConsPath::Nil)
            {
                do_stack.push(RevisitStack::GeneratePair);
                do_stack.push(RevisitStack::Choice(a));
                do_stack.push(RevisitStack::Choice(b));
            } else {
                done.push(builder.make_nil());
            }
        } else {
            let mut atom_bytes = Vec::new();
            let mut use_atom = choice;
            while let AtomPath::AndByte(b, next) =
                self.choose_with_default(&self.atoms, use_atom, AtomPath::Nil)
            {
                use_atom = next % use_atom;
                atom_bytes.push(b);
            }
            done.push(builder.make_atom(&atom_bytes)?);
        }
        Ok(())
    }

    fn choose_agg_input(&self, choice: usize, level: usize) -> ConsContent {
        let choice_type = choice & 3;
        let choice_value = choice >> 2;
        if level == 3 && choice_type == 3 {
            ConsContent::WithList(choice_value % self.lists.len())
        } else if level > 1 && choice_type > 1 {
            ConsContent::WithCons(choice_value % self.conses.len())
        } else {
            ConsContent::WithAtom(choice_value)
        }
    }

    pub fn to_sexp<T: std::fmt::Debug,E>(
        &mut self,
        builder: &mut dyn SexpBuilder<T,E>,
        prototype: T
    ) -> Result<T,E> {
        // Make atoms.
        for a in self.cas.atoms.iter() {
            let this_byte = (a & 0xff) as u8;
            self.atoms.push(AtomPath::AndByte(this_byte, a >> 8));
        }

        // Make conses.
        for first in self.cas.conses.clone().iter() {
            let first_item = self.choose_agg_input(*first, 2);
            let first_choice = self.get_choice().unwrap_or(0);
            let second_item = self.choose_agg_input(first_choice, 2);
            self.conses.push(ConsPath::Consed(first_item, second_item));
        }

        for item in self.cas.lists.clone().iter() {
            let head_choice = self.get_choice().unwrap_or(0);
            let head = self.choose_agg_input(head_choice.into(), 3);
            self.lists.push(ListPath::Enlisted(head, *item));
        }

        let mut done = vec![];
        let mut do_stack = vec![RevisitStack::GenerateFor(Some(prototype))];

        while let Some(perform) = do_stack.pop() {
            match perform {
                RevisitStack::GenerateFor(None) => {
                    let choice = self.get_choice().unwrap_or(0);
                    self.make_choice(
                        &mut do_stack,
                        &mut done,
                        builder,
                        choice,
                        3
                    )?;
                }
                RevisitStack::GenerateFor(Some(v)) => {
                    match builder.destructure(v)? {
                        SexpContent::SexpTypeAtom(a) => {
                            if a.is_empty() {
                                done.push(builder.make_nil());
                                continue;
                            }

                            let choice = self.get_choice().unwrap_or(0);
                            self.make_choice(
                                &mut do_stack,
                                &mut done,
                                builder,
                                choice,
                                3
                            )?;
                        }
                        SexpContent::SexpTypeCons(a,b) => {
                            do_stack.push(RevisitStack::GeneratePair);
                            do_stack.push(RevisitStack::GenerateFor(Some(a)));
                            do_stack.push(RevisitStack::GenerateFor(Some(b)));
                        }
                    }
                }
                RevisitStack::GeneratePair => {
                    let a = done.pop().unwrap_or_else(|| builder.make_nil());
                    let b = done.pop().unwrap_or_else(|| builder.make_nil());
                    done.push(builder.make_cons(a,b)?);
                }
                choice => {
                    let (kind, choice_number, level) =
                        match choice {
                            RevisitStack::Choice(ConsContent::WithAtom(c)) => (0, c, 1),
                            RevisitStack::Choice(ConsContent::WithCons(c)) => (2, c, 2),
                            RevisitStack::Choice(ConsContent::WithList(c)) => (3, c, 3),
                            _ => (0, 0, 0)
                        };
                    let choice =
                        self.make_choice(
                            &mut do_stack,
                            &mut done,
                            builder,
                            (choice_number << 2) | kind,
                            level
                        )?;
                    choice
                }
            }
        }

        Ok(done.pop().unwrap_or_else(|| builder.make_nil()))
    }
}

impl CollectArgumentStructure {
    // Use the associated builder to frugally create an sexp described by the
    // data absorbed in the CollectArgumentStructure.
    pub fn to_sexp<T: std::fmt::Debug,E>(&self, builder: &mut dyn SexpBuilder<T,E>, prototype: T) -> Result<T,E> {
        let mut reader = CollectArgumentStructureReader::new(self);
        reader.to_sexp(builder, prototype)
    }
}

// CollectArgumentStructure becomes populated by a constrained set of bits.
// Its to_sexp method uses an SexpBuilder to generate s-expressions based on
// the collected state.
impl Distribution<CollectArgumentStructure> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> CollectArgumentStructure {
        let mut iters = 0;
        let mut cas: CollectArgumentStructure = Default::default();

        loop {
            let input_32: u32 = rng.gen();
            let input: usize = input_32 as usize;

            // Stop if zero.  For random generators reading fuzz data, this
            // signals that the significant data ended.
            if input == 0 {
                break;
            }

            iters += 1;
            if iters > cas.read_max {
                break;
            }

            let inputs = [(input >> 16), ((input) & 0xffff)];
            for input in inputs.iter() {
                let kind = input & 3;
                let value = input >> 2;
                if kind == 0 {
                    cas.choices.push(value);
                } else if kind == 1{
                    cas.atoms.push(value);
                } else if kind == 2 {
                    cas.conses.push(value);
                } else {
                    cas.lists.push(value);
                }
            }
        }

        cas
    }
}

#[cfg(any(test, feature="clvmr"))]
pub struct AllocatorSexpBuilder<'a> {
    allocator: &'a mut Allocator
}

#[cfg(any(test, feature="clvmr"))]
impl<'a> AllocatorSexpBuilder<'a> {
    pub fn new(allocator: &'a mut Allocator) -> Self {
        AllocatorSexpBuilder { allocator }
    }
}

#[cfg(any(test, feature="clvmr"))]
impl<'a> SexpBuilder<NodePtr, EvalErr> for AllocatorSexpBuilder<'a> {
    fn destructure(&mut self, n: NodePtr) -> Result<SexpContent<NodePtr>, EvalErr> {
        match self.allocator.sexp(n) {
            SExp::Atom(b) => Ok(SexpContent::SexpTypeAtom(self.allocator.buf(&b).to_vec())),
            SExp::Pair(f,r) => Ok(SexpContent::SexpTypeCons(f,r))
        }
    }

    fn make_cons(&mut self, some: NodePtr, other: NodePtr) -> Result<NodePtr, EvalErr> {
        self.allocator.new_pair(some, other)
    }

    fn make_nil(&mut self) -> NodePtr {
        self.allocator.null()
    }

    fn make_atom(&mut self, bytes: &[u8]) -> Result<NodePtr, EvalErr> {
        self.allocator.new_atom(bytes)
    }
}
