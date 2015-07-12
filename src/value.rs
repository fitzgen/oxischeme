// Copyright 2014 Nick Fitzgerald
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Scheme value implementation.

use std::collections::{HashSet};
use std::default::{Default};
use std::fmt;
use std::hash;

use environment::{ActivationPtr, RootedActivationPtr};
use eval::{Meaning, TrampolineResult};
use heap::{ArenaPtr, GcThing, Heap, IterGcThing, Rooted, RootedStringPtr,
           StringPtr, ToGcThing, Trace};
use primitives::{PrimitiveFunction};

/// A cons cell is a pair of `car` and `cdr` values. A list is one or more cons
/// cells, daisy chained together via the `cdr`. A list is "proper" if the last
/// `cdr` is `Value::EmptyList`, or the scheme value `()`. Otherwise, it is
/// "improper".
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct Cons {
    car: Value,
    cdr: Value,
}

impl Default for Cons {
    /// Do not use this method, instead allocate cons cells on the heap with
    /// `Heap::allocate_cons` and get back a `ConsPtr`.
    fn default() -> Cons {
        Cons {
            car: Value::EmptyList,
            cdr: Value::EmptyList,
        }
    }
}

impl Cons {
    /// Get the car of this cons cell.
    pub fn car(&self, heap: &mut Heap) -> RootedValue {
        Rooted::new(heap, self.car)
    }

    /// Get the cdr of this cons cell.
    pub fn cdr(&self, heap: &mut Heap) -> RootedValue {
        Rooted::new(heap, self.cdr)
    }

    /// Set the car of this cons cell.
    pub fn set_car(&mut self, car: &RootedValue) {
        self.car = **car;
    }

    /// Set the cdr of this cons cell.
    pub fn set_cdr(&mut self, cdr: &RootedValue) {
        self.cdr = **cdr;
    }
}

impl Trace for Cons {
    fn trace(&self) -> IterGcThing {
        let mut results = vec!();

        if let Some(car) = self.car.to_gc_thing() {
            results.push(car);
        }

        if let Some(cdr) = self.cdr.to_gc_thing() {
            results.push(cdr);
        }

        results.into_iter()
    }
}

/// A pointer to a cons cell on the heap.
pub type ConsPtr = ArenaPtr<Cons>;

impl ToGcThing for ConsPtr {
    fn to_gc_thing(&self) -> Option<GcThing> {
        Some(GcThing::from_cons_ptr(*self))
    }
}

/// A rooted pointer to a cons cell on the heap.
pub type RootedConsPtr = Rooted<ConsPtr>;

/// User defined procedures are represented by their body and a pointer to the
/// activation that they were defined within.
pub struct Procedure {
    pub arity: u32,
    pub body: Option<Box<Meaning>>,
    pub act: Option<ActivationPtr>,
}

impl Default for Procedure {
    fn default() -> Procedure {
        Procedure {
            body: None,
            act: None,
            arity: 0,
        }
    }
}

impl Trace for Procedure {
    fn trace(&self) -> IterGcThing {
        // We don't need to trace the body because a `Meaning` can only contain
        // rooted GC things to ensure that quotations will always return the
        // same object rather than generate new equivalent-but-not-`eq?`
        // objects.

        vec!(GcThing::from_activation_ptr(self.act.expect(
            "Should never trace an uninitialized Procedure"))).into_iter()
    }
}

impl hash::Hash for Procedure {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.arity.hash(state);
        self.act.hash(state);
        self.body.as_ref()
            .expect("Should never hash an uninitialized Procedure")
            .hash(state);
    }
}


/// A pointer to a `Procedure` on the heap.
pub type ProcedurePtr = ArenaPtr<Procedure>;
impl ToGcThing for ProcedurePtr {
    fn to_gc_thing(&self) -> Option<GcThing> {
        Some(GcThing::from_procedure_ptr(*self))
    }
}

/// A rooted pointer to a `Procedure` on the heap.
pub type RootedProcedurePtr = Rooted<ProcedurePtr>;

/// A primitive procedure, such as Scheme's `+` or `cons`.
pub struct Primitive {
    /// The function implementing the primitive.
    function: PrimitiveFunction,
    /// The name of the primitive.
    name: &'static str,
}

impl Clone for Primitive {
    fn clone(&self) -> Self {
        Primitive {
            function: self.function,
            name: self.name,
        }
    }
}

impl ::std::marker::Copy for Primitive { }

impl PartialEq for Primitive {
    fn eq(&self, rhs: &Self) -> bool {
        self.function as usize == rhs.function as usize
    }
}

impl Eq for Primitive { }

impl hash::Hash for Primitive {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        let u = self.function as usize;
        u.hash(state);
    }
}

impl Primitive {
    #[inline]
    pub fn call(&self, heap: &mut Heap, args: Vec<RootedValue>) -> TrampolineResult {
        (self.function)(heap, args)
    }
}

impl fmt::Debug for Primitive {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// `Value` represents a scheme value of any type.
///
/// Note that `Eq` and `PartialEq` are object identity, not structural
/// comparison, same as with [`ArenaPtr`](struct.ArenaPtr.html).
#[derive(Clone, Copy, Eq, Hash, PartialEq, Debug)]
pub enum Value {
    /// The empty list: `()`.
    EmptyList,

    /// The scheme pair type is a pointer to a GC-managed `Cons` cell.
    Pair(ConsPtr),

    /// The scheme string type is a pointer to a GC-managed `String`.
    String(StringPtr),

    /// Scheme symbols are also implemented as a pointer to a GC-managed
    /// `String`.
    Symbol(StringPtr),

    /// Scheme integers are represented as 64 bit integers.
    Integer(i64),

    /// Scheme booleans are represented with `bool`.
    Boolean(bool),

    /// Scheme characters are `char`s.
    Character(char),

    /// A user-defined Scheme procedure is a pointer to a GC-managed
    /// `Procedure`.
    Procedure(ProcedurePtr),

    /// A primitive Scheme procedure is just a pointer to a `Primitive` type
    /// function pointer.
    Primitive(Primitive),
}

/// # `Value` Constructors
impl Value {
    /// Create a new integer value.
    pub fn new_integer(i: i64) -> Value {
        Value::Integer(i)
    }

    /// Create a new boolean value.
    pub fn new_boolean(b: bool) -> Value {
        Value::Boolean(b)
    }

    /// Create a new character value.
    pub fn new_character(c: char) -> Value {
        Value::Character(c)
    }

    /// Create a new cons pair value with the given car and cdr.
    pub fn new_pair(heap: &mut Heap,
                    car: &RootedValue,
                    cdr: &RootedValue) -> RootedValue {
        let mut cons = heap.allocate_cons();
        cons.set_car(car);
        cons.set_cdr(cdr);
        Rooted::new(heap, Value::Pair(*cons))
    }

    /// Create a new procedure with the given parameter list and body.
    pub fn new_procedure(heap: &mut Heap,
                         arity: u32,
                         act: &RootedActivationPtr,
                         body: Meaning) -> RootedValue {
        let mut procedure = heap.allocate_procedure();
        procedure.arity = arity;
        procedure.act = Some(**act);
        procedure.body = Some(Box::new(body));
        Rooted::new(heap, Value::Procedure(*procedure))
    }

    pub fn new_primitive(name: &'static str,
                         function: PrimitiveFunction) -> Value {
        Value::Primitive(Primitive {
            name: name,
            function: function
        })
    }

    /// Create a new string value with the given string.
    pub fn new_string(heap: &mut Heap, str: String) -> RootedValue {
        let mut value = heap.allocate_string();
        value.clear();
        value.push_str(&*str);
        Rooted::new(heap, Value::String(*value))
    }

    /// Create a new symbol value with the given string.
    pub fn new_symbol(heap: &mut Heap, str: RootedStringPtr) -> RootedValue {
        Rooted::new(heap, Value::Symbol(*str))
    }
}

/// # `Value` Methods
impl Value {
    /// Assuming this value is a cons pair, get its car value. Otherwise, return
    /// `None`.
    pub fn car(&self, heap: &mut Heap) -> Option<RootedValue> {
        match *self {
            Value::Pair(ref cons) => Some(cons.car(heap)),
            _                     => None,
        }
    }

    /// Assuming this value is a cons pair, get its cdr value. Otherwise, return
    /// `None`.
    pub fn cdr(&self, heap: &mut Heap) -> Option<RootedValue> {
        match *self {
            Value::Pair(ref cons) => Some(cons.cdr(heap)),
            _                     => None,
        }
    }

    /// Return true if this value is a pair, false otherwise.
    pub fn is_pair(&self) -> bool {
        match *self {
            Value::Pair(_) => true,
            _              => false,
        }
    }

    /// Return true if this value is an atom, false otherwise.
    pub fn is_atom(&self) -> bool {
        !self.is_pair()
    }

    /// Coerce this symbol value to a `StringPtr` to the symbol's string name.
    pub fn to_symbol(&self, heap: &mut Heap) -> Option<RootedStringPtr> {
        match *self {
            Value::Symbol(sym) => Some(Rooted::new(heap, sym)),
            _                  => None,
        }
    }

    /// Coerce this pair value to a `ConsPtr` to the cons cell this pair is
    /// referring to.
    pub fn to_pair(&self, heap: &mut Heap) -> Option<RootedConsPtr> {
        match *self {
            Value::Pair(cons) => Some(Rooted::new(heap, cons)),
            _                 => None,
        }
    }

    /// Coerce this procedure value to a `ProcedurePtr` to the `Procedure` this
    /// value is referring to.
    pub fn to_procedure(&self, heap: &mut Heap) -> Option<RootedProcedurePtr> {
        match *self {
            Value::Procedure(p) => Some(Rooted::new(heap, p)),
            _                   => None,
        }
    }

    /// Coerce this integer value to its underlying `i64`.
    pub fn to_integer(&self) -> Option<i64> {
        match *self {
            Value::Integer(ref i) => Some(*i),
            _                     => None,
        }
    }

    /// Assuming that this value is a proper list, get the length of the list.
    pub fn len(&self) -> Result<u64, ()> {
        match *self {
            Value::EmptyList => Ok(0),
            Value::Pair(p)   => {
                let cdr_len = try!(p.cdr.len());
                Ok(cdr_len + 1)
            },
            _                => Err(()),
        }
    }

    /// Iterate over this list value.
    pub fn iter(&self) -> ConsIterator {
        ConsIterator {
            val: *self
        }
    }
}

impl ToGcThing for Value {
    fn to_gc_thing(&self) -> Option<GcThing> {
        match *self {
            Value::String(str)  => Some(GcThing::from_string_ptr(str)),
            Value::Symbol(sym)  => Some(GcThing::from_string_ptr(sym)),
            Value::Pair(cons)   => Some(GcThing::from_cons_ptr(cons)),
            Value::Procedure(p) => Some(GcThing::from_procedure_ptr(p)),
            _                   => None,
        }
    }
}

fn print(f: &mut fmt::Formatter, val: &Value, seen: &mut HashSet<ConsPtr>) -> fmt::Result {
    match *val {
        Value::EmptyList        => write!(f, "()"),
        Value::Pair(ref cons)   => {
            try!(write!(f, "("));
            try!(print_pair(f, cons, seen));
            write!(f, ")")
        },
        Value::String(ref str)  => {
            try!(write!(f, "\""));
            try!(write!(f, "{}", **str));
            write!(f, "\"")
        },
        Value::Symbol(ref s)    => write!(f, "{}", **s),
        Value::Integer(ref i)   => write!(f, "{}", i),
        Value::Boolean(ref b)   => {
            write!(f, "{}", if *b {
                "#t"
            } else {
                "#f"
            })
        },
        Value::Character(ref c) => match *c {
            '\n' => write!(f, "#\\newline"),
            '\t' => write!(f, "#\\tab"),
            ' '  => write!(f, "#\\space"),
            _    => write!(f, "#\\{}", c),
        },
        Value::Procedure(ref p) => write!(f, "#<procedure {:?}>", p),
        Value::Primitive(ref p) => write!(f, "#<procedure {:?}>", p),
    }
}

/// Print the given cons pair, without the containing "(" and ")".
fn print_pair(f: &mut fmt::Formatter, cons: &ConsPtr, seen: &mut HashSet<ConsPtr>) -> fmt::Result {
    if seen.contains(cons) {
        return write!(f, "<cyclic value>");
    }
    seen.insert(*cons);

    try!(print(f, &cons.car, seen));

    if let Value::Pair(rest) = cons.cdr {
        if seen.contains(&rest) {
            return write!(f, " . <cyclic value>");
        }
    }

    match cons.cdr {
        Value::EmptyList => Ok(()),
        Value::Pair(ref cdr) => {
            try!(write!(f, " "));
            print_pair(f, cdr, seen)
        },
        ref val => {
            try!(write!(f, " . "));
            print(f, val, seen)
        },
    }
}

impl fmt::Display for Value {
    /// Print the given value's text representation to the given writer. This is
    /// the opposite of `Read`.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        print(f, self, &mut HashSet::new())
    }
}

pub type RootedValue = Rooted<Value>;

/// Either a Scheme `RootedValue`, or a `String` containing an error message.
pub type SchemeResult = Result<RootedValue, String>;

/// An iterator which yields `Ok` for each value in a cons-list and finishes
/// with `None` when the end of the list is reached (the scheme empty list
/// value) or `Err` when iterating over an improper list.
///
/// For example: the list `(1 2 3)` would yield `Ok(1)`, `Ok(2)`, `Ok(3)`,
/// `None`, while the improper list `(1 2 . 3)` would yield `Ok(1)`, `Ok(2)`,
/// `Err`.
#[derive(Clone, Copy)]
pub struct ConsIterator {
    val: Value
}

impl Iterator for ConsIterator {
    type Item = Result<Value, ()>;

    fn next(&mut self) -> Option<Result<Value, ()>> {
        match self.val {
            Value::EmptyList => None,
            Value::Pair(cons) => {
                let Cons { car, cdr } = *cons;
                self.val = cdr;
                Some(Ok(car))
            },
            _ => Some(Err(())),
        }
    }
}

/// A helper utility to create a cons list from the given values.
pub fn list(heap: &mut Heap, values: &[RootedValue]) -> RootedValue {
    list_helper(heap, &mut values.iter())
}

fn list_helper<'a, T: Iterator<Item=&'a RootedValue>>(heap: &mut Heap,
                                                      values: &mut T) -> RootedValue {
    match values.next() {
        None      => Rooted::new(heap, Value::EmptyList),
        Some(car) => {
            let rest = list_helper(heap, values);
            Value::new_pair(heap, car, &rest)
        },
    }
}

/// ## The 28 car/cdr compositions.
impl Cons {
    pub fn cddr(&self, heap: &mut Heap) -> SchemeResult {
        self.cdr.cdr(heap).ok_or("bad cddr".to_string())
    }

    pub fn cdddr(&self, heap: &mut Heap) -> SchemeResult {
        let cddr = try!(self.cddr(heap));
        cddr.cdr(heap).ok_or("bad cdddr".to_string())
    }

    // TODO FITZGEN: cddddr

    pub fn cadr(&self, heap: &mut Heap) -> SchemeResult {
        self.cdr.car(heap).ok_or("bad cadr".to_string())
    }

    pub fn caddr(&self, heap: &mut Heap) -> SchemeResult {
        let cddr = try!(self.cddr(heap));
        cddr.car(heap).ok_or("bad caddr".to_string())
    }

    pub fn cadddr(&self, heap: &mut Heap) -> SchemeResult {
        let cdddr = try!(self.cdddr(heap));
        cdddr.car(heap).ok_or("bad caddr".to_string())
    }

    // TODO FITZGEN ...
}

#[cfg(test)]
mod tests {
    use eval::{evaluate_file};
    use heap::{Heap};

    #[test]
    fn test_print_cycle() {
        let heap = &mut Heap::new();
        evaluate_file(heap, "./tests/test_print_cycle.scm")
            .ok()
            .expect("Should be able to eval a file.");
        assert!(true, "Shouldn't get stuck in an infinite loop printing a cyclic value");
    }
}
