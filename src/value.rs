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

use std::default::{Default};

use heap::{ConsPtr, Heap, StringPtr};
use context::{Context};

/// A cons cell is a pair of `car` and `cdr` values. A list is one or more cons
/// cells, daisy chained together via the `cdr`. A list is "proper" if the last
/// `cdr` is `Value::EmptyList`, or the scheme value `()`. Otherwise, it is
/// "improper".
///
/// You cannot directly create a cons cell, you must allocate one on the heap
/// with an `Arena` and get back a `ConsPtr`.
#[deriving(Copy, PartialEq)]
pub struct Cons {
    car: Value,
    cdr: Value,
}

impl Default for Cons {
    /// Create a new cons pair, with both `car` and `cdr` initialized to
    /// `Value::EmptyList`.
    fn default() -> Cons {
        Cons {
            car: Value::EmptyList,
            cdr: Value::EmptyList,
        }
    }
}

impl Cons {
    /// Get the car of this cons cell.
    pub fn car(&self) -> Value {
        self.car
    }

    /// Get the cdr of this cons cell.
    pub fn cdr(&self) -> Value {
        self.cdr
    }

    /// Set the car of this cons cell.
    pub fn set_car(&mut self, car: Value) {
        self.car = car;
    }

    /// Set the cdr of this cons cell.
    pub fn set_cdr(&mut self, cdr: Value) {
        self.cdr = cdr;
    }
}

/// `Value` represents a scheme value of any type.
///
/// Note that `PartialEq` is object identity, not structural comparison, same as
/// with [`ArenaPtr`](struct.ArenaPtr.html).
#[deriving(Copy, PartialEq, Show)]
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
    pub fn new_pair(heap: &mut Heap, car: Value, cdr: Value) -> Value {
        let mut cons = heap.allocate_cons();
        cons.set_car(car);
        cons.set_cdr(cdr);
        Value::Pair(cons)
    }

    /// Create a new string value with the given string.
    pub fn new_string(heap: &mut Heap, str: String) -> Value {
        let mut value = heap.allocate_string();
        value.clear();
        value.push_str(str.as_slice());
        Value::String(value)
    }

    /// Create a new symbol value with the given string.
    pub fn new_symbol(str: StringPtr) -> Value {
        Value::Symbol(str)
    }
}

/// # `Value` Methods
impl Value {
    /// Assuming this value is a cons pair, get its car value. Otherwise, return
    /// `None`.
    pub fn car(&self) -> Option<Value> {
        match *self {
            Value::Pair(ref cons) => Some(cons.car()),
            _                     => None,
        }
    }

    /// Assuming this value is a cons pair, get its cdr value. Otherwise, return
    /// `None`.
    pub fn cdr(&self) -> Option<Value> {
        match *self {
            Value::Pair(ref cons) => Some(cons.cdr()),
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

    /// Convert this symbol value to a `StringPtr` to the symbol's string name.
    pub fn to_symbol(&self) -> Option<StringPtr> {
        match *self {
            Value::Symbol(sym) => Some(sym),
            _                  => None,
        }
    }

    /// Convert this pair value to a `ConsPtr` to the cons cell this pair is
    /// referring to.
    pub fn to_pair(&self) -> Option<ConsPtr> {
        match *self {
            Value::Pair(cons) => Some(cons),
            _                 => None,
        }
    }

    /// Assuming that this value is a proper list, get the length of the list.
    pub fn len(&self) -> Result<u64, ()> {
        match *self {
            Value::EmptyList => Ok(0),
            Value::Pair(p)   => {
                let cdr_len = try!(p.cdr().len());
                Ok(cdr_len + 1)
            },
            _                => Err(()),
        }
    }
}

/// Either a Scheme `Value`, or a `String` containing an error message.
pub type SchemeResult = Result<Value, String>;

/// A helper utility to create a cons list from the given values.
pub fn list(ctx: &mut Context, values: &[Value]) -> Value {
    list_helper(ctx, &mut values.iter())
}

fn list_helper<'a, T: Iterator<&'a Value>>(ctx: &mut Context,
                                           values: &mut T) -> Value {
    match values.next() {
        None      => Value::EmptyList,
        Some(car) => {
            let cdr = list_helper(ctx, values);
            Value::new_pair(ctx.heap(), *car, cdr)
        },
    }
}

/// ## The 28 car/cdr compositions.
impl Cons {
    pub fn cddr(&self) -> SchemeResult {
        self.cdr.cdr().ok_or("bad cddr".to_string())
    }

    pub fn cdddr(&self) -> SchemeResult {
        let cddr = try!(self.cddr());
        cddr.cdr().ok_or("bad cdddr".to_string())
    }

    // TODO FITZGEN: cddddr

    pub fn cadr(&self) -> SchemeResult {
        self.cdr.car().ok_or("bad cadr".to_string())
    }

    pub fn caddr(&self) -> SchemeResult {
        let cddr = try!(self.cddr());
        cddr.car().ok_or("bad caddr".to_string())
    }

    pub fn cadddr(&self) -> SchemeResult {
        let cdddr = try!(self.cdddr());
        cdddr.car().ok_or("bad caddr".to_string())
    }

    // TODO FITZGEN ...
}

