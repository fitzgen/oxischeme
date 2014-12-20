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

use std::cmp;
use std::fmt;
use std::iter::{range};

/// TODO FITZGEN
#[deriving(PartialEq)]
struct Cons {
    car: Value,
    cdr: Value,
}

impl Cons {
    /// TODO FITZGEN
    pub fn new() -> Cons {
        Cons {
            car: Value::EmptyList,
            cdr: Value::EmptyList,
        }
    }
}

type FreeList = Vec<uint>;

/// TODO FITZGEN
fn new_free_list(limit: uint) -> FreeList {
    range(0, limit).collect()
}

/// TODO FITZGEN
pub struct Heap {
    pool: Vec<Cons>,
    free: FreeList,
}

/// TODO FITZGEN
pub static DEFAULT_HEAP_CAPACITY : uint = 1 << 12;

impl Heap {
    /// TODO FITZGEN
    pub fn new() -> Heap {
        Heap::with_capacity(DEFAULT_HEAP_CAPACITY)
    }

    /// TODO FITZGEN
    pub fn with_capacity(capacity: uint) -> Heap {
        assert!(capacity > 0);
        Heap {
            pool: range(0, capacity).map(|_| Cons::new()).collect(),
            free: new_free_list(capacity),
        }
    }

    /// TODO FITZGEN
    pub fn capacity(&self) -> uint {
        self.pool.capacity()
    }

    /// TODO FITZGEN
    ///
    /// Panics if we run out of memory in this `Heap`.
    pub fn allocate_cons(&mut self) -> ConsPtr {
        match self.free.pop() {
            None       => panic!("Heap::allocate_cons out of memory!"),
            Some(idx)  => {
                let self_ptr : *mut Heap = self;
                ConsPtr::new(self_ptr, idx)
            },
        }
    }
}

/// TODO FITZGEN
#[allow(raw_pointer_deriving)]
#[deriving(Copy)]
pub struct ConsPtr {
    heap: *mut Heap,
    index: uint,
}

impl ConsPtr {
    /// TODO FITZGEN
    fn new(heap: *mut Heap, index: uint) -> ConsPtr {
        unsafe {
            let heap_ref = heap.as_ref()
                .expect("ConsPtr::new should be passed a valid Heap.");
            assert!(index < heap_ref.capacity());
        }
        ConsPtr {
            heap: heap,
            index: index,
        }
    }

    /// TODO FITZGEN
    pub fn car(&self) -> Value {
        unsafe {
            let heap = self.heap.as_ref()
                .expect("ConsPtr::car should always have a Heap.");
            heap.pool[self.index].car
        }
    }

    /// TODO FITZGEN
    pub fn cdr(&self) -> Value {
        unsafe {
            let heap = self.heap.as_ref()
                .expect("ConsPtr::cdr should always have a Heap.");
            heap.pool[self.index].cdr
        }
    }

    /// TODO FITZGEN
    pub fn set_car(&mut self, car: Value) {
        unsafe {
            let heap = self.heap.as_mut()
                .expect("ConsPtr::set_car should always have access to the heap.");
            heap.pool[self.index].car = car;
        }
    }

    /// TODO FITZGEN
    pub fn set_cdr(&mut self, cdr: Value) {
        unsafe {
            let heap = self.heap.as_mut()
                .expect("ConsPtr::set_cdr should always have access to the heap.");
            heap.pool[self.index].cdr = cdr;
        }
    }
}

impl fmt::Show for ConsPtr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ConsPtr({})", self.index)
    }
}

impl cmp::PartialEq for ConsPtr {
    fn eq(&self, other: &ConsPtr) -> bool {
        self.index == other.index && self.heap.to_uint() == other.heap.to_uint()
    }
}

/// `Value` represents a scheme value of any type.
///
/// Note that `PartialEq` is object identity, not structural identity. In other
/// words, it is equivalent to the scheme function `eq?`, not the scheme
/// function `equal?`.
#[deriving(Copy, PartialEq, Show)]
pub enum Value {
    /// The empty list: `()`.
    EmptyList,
    /// The scheme pair type is a pointer into our `Heap` to a GC-managed `Cons`
    /// cell.
    Pair(ConsPtr),
    /// Scheme integers are represented as 64 bit integers.
    Integer(i64),
    /// Scheme booleans are represented with `bool`.
    Boolean(bool),
    /// Scheme characters are `char`s.
    Character(char),
}

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

    /// TODO FITZGEN
    pub fn new_pair(heap: &mut Heap, car: Value, cdr: Value) -> Value {
        let mut cons = heap.allocate_cons();
        cons.set_car(car);
        cons.set_cdr(cdr);
        Value::Pair(cons)
    }

    /// TODO FITZGEN
    pub fn car(&self) -> Option<Value> {
        match *self {
            Value::Pair(ref cons) => Some(cons.car()),
            _                     => None,
        }
    }

    /// TODO FITZGEN
    pub fn cdr(&self) -> Option<Value> {
        match *self {
            Value::Pair(ref cons) => Some(cons.cdr()),
            _                     => None,
        }
    }
}