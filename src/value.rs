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
//!
//! ## Allocation Strategy
//!
//! Scheme has a variety of types that must be allocated on the heap: cons
//! cells, strings (currently unimplemented), and vectors (currently
//! unimplemented). Oxischeme's current allocation strategy is deliberately as
//! simple as possible. We represent the heap as a pre-allocated vector of cons
//! cells. We keep track of un-allocated cons cells with a "free list" of
//! indices into the pre-allocated vector of cons cells. Whenever we need to
//! allocate a new cons cell, we remove an entry from this list and return a
//! pointer to the cons cell at that entry's index. Garbage collection
//! (currently unimplemented) will add new entries to the free list whenever
//! objects are reclaimed. If we run out of space in the pre-allocated vector,
//! we panic.

use std::cmp;
use std::fmt;
use std::iter::{range};

/// A cons cell is a pair of `car` and `cdr` values. A list is one or more cons
/// cells, daisy chained together via the `cdr`. A list is "proper" if the last
/// `cdr` is `Value::EmptyList`, or the scheme value `()`. Otherwise, it is
/// "improper".
#[deriving(PartialEq)]
struct Cons {
    car: Value,
    cdr: Value,
}

impl Cons {
    /// Create a new cons pair, with both `car` and `cdr` initialized to
    /// `Value::EmptyList`.
    pub fn new() -> Cons {
        Cons {
            car: Value::EmptyList,
            cdr: Value::EmptyList,
        }
    }
}

/// We use a vector for our implementation of a free list. `Vector::push` to add
/// new entries, `Vector::pop` to remove the next entry when we allocate.
type FreeList = Vec<uint>;

/// The scheme heap, containing all allocated cons cells. The first step of
/// running any scheme program involves first creating an instance of `Heap`.
pub struct Heap {
    pool: Vec<Cons>,
    free: FreeList,
}

/// The default heap capacity.
pub static DEFAULT_HEAP_CAPACITY : uint = 1 << 12;

impl Heap {
    /// Create a new `Heap` with the default capacity.
    pub fn new() -> Heap {
        Heap::with_capacity(DEFAULT_HEAP_CAPACITY)
    }

    /// Create a new `Heap` with the capacity to allocate the given number of
    /// cons cells.
    pub fn with_capacity(capacity: uint) -> Heap {
        assert!(capacity > 0);
        Heap {
            pool: range(0, capacity).map(|_| Cons::new()).collect(),
            free: range(0, capacity).collect(),
        }
    }

    /// Get this heap's capacity for simultaneously allocated cons cells.
    pub fn capacity(&self) -> uint {
        self.pool.capacity()
    }

    /// Allocate a new cons cell and return a pointer to it.
    ///
    /// ## Panics
    ///
    /// Panics if the number of cons cells allocated is already equal to this
    /// heap's capacity and no more cons cells can be allocated.
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

/// A pointer to a cons cell on the heap.
#[allow(raw_pointer_deriving)]
#[deriving(Copy)]
pub struct ConsPtr {
    heap: *mut Heap,
    index: uint,
}

impl ConsPtr {
    /// Create a new `ConsPtr` to the cons cell at the given index in the heap.
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

    /// Get the car of this cons cell.
    pub fn car(&self) -> Value {
        unsafe {
            let heap = self.heap.as_ref()
                .expect("ConsPtr::car should always have a Heap.");
            heap.pool[self.index].car
        }
    }

    /// Get the cdr of this cons cell.
    pub fn cdr(&self) -> Value {
        unsafe {
            let heap = self.heap.as_ref()
                .expect("ConsPtr::cdr should always have a Heap.");
            heap.pool[self.index].cdr
        }
    }

    /// Set the car of this cons cell.
    pub fn set_car(&mut self, car: Value) {
        unsafe {
            let heap = self.heap.as_mut()
                .expect("ConsPtr::set_car should always have access to the heap.");
            heap.pool[self.index].car = car;
        }
    }

    /// Set the cdr of this cons cell.
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
    /// Note that `PartialEq` implements pointer object identity, not structural
    /// comparison. In other words, it is equivalent to the scheme function
    /// `eq?`, not the scheme function `equal?`
    fn eq(&self, other: &ConsPtr) -> bool {
        self.index == other.index && self.heap.to_uint() == other.heap.to_uint()
    }
}

/// `Value` represents a scheme value of any type.
///
/// Note that `PartialEq` is object identity, not structural comparison, same as
/// with [`ConsPtr`](struct.ConsPtr.html).
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
}