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

//! Scheme has a variety of types that must be allocated on the heap: cons
//! cells, strings, and vectors (currently unimplemented). Oxischeme's current
//! allocation strategy is deliberately as simple as possible. We represent the
//! heap as a pre-allocated vector of cons cells. We keep track of un-allocated
//! cons cells with a "free list" of indices into the pre-allocated vector of
//! cons cells. Whenever we need to allocate a new cons cell, we remove an entry
//! from this list and return a pointer to the object at that entry's index.
//! Garbage collection (currently unimplemented) will add new entries to the
//! free list whenever objects are reclaimed. If we run out of space in the
//! pre-allocated vector, we panic.

use std::cmp;
use std::default::{Default};
use std::fmt;

use environment::{Environment};
use value::{Cons};

/// We use a vector for our implementation of a free list. `Vector::push` to add
/// new entries, `Vector::pop` to remove the next entry when we allocate.
type FreeList = Vec<uint>;

/// An arena from which to allocate `T` objects from.
pub struct Arena<T> {
    pool: Vec<T>,
    free: FreeList,
}

impl<T: Default> Arena<T> {
    /// Create a new `Arena` with the capacity to allocate the given number of
    /// `T` instances.
    pub fn new(capacity: uint) -> Arena<T> {
        assert!(capacity > 0);
        Arena {
            pool: range(0, capacity).map(|_| Default::default()).collect(),
            free: range(0, capacity).collect(),
        }
    }

    /// Get this heap's capacity for simultaneously allocated cons cells.
    pub fn capacity(&self) -> uint {
        self.pool.capacity()
    }

    /// Allocate a new `T` instance and return a pointer to it.
    ///
    /// ## Panics
    ///
    /// Panics if the number of `T` instances allocated is already equal to this
    /// `Arena`'s capacity and no more `T` instances can be allocated.
    pub fn allocate(&mut self) -> ArenaPtr<T> {
        match self.free.pop() {
            None      => panic!("Arena::allocate: out of memory!"),
            Some(idx) => {
                let self_ptr : *mut Arena<T> = self;
                ArenaPtr::new(self_ptr, idx)
            },
        }
    }
}

/// A pointer to a `T` instance in an arena.
pub struct ArenaPtr<T> {
    arena: *mut Arena<T>,
    index: uint,
}

// XXX: We have to manually declar that ArenaPtr<T> is copy-able because if we
// use `#[deriving(Copy)]` it wants T to be copy-able as well, despite the fact
// that we only need to copy our pointer to the Arena<T>, not any T or the Arena
// itself.
impl <T> ::std::kinds::Copy for ArenaPtr<T> { }

impl<T: Default> ArenaPtr<T> {
    /// Create a new `ArenaPtr` to the `T` instance at the given index in the
    /// provided arena. **Not** publicly exposed, and should only be called by
    /// `Arena::allocate`.
    fn new(arena: *mut Arena<T>, index: uint) -> ArenaPtr<T> {
        unsafe {
            let arena_ref = arena.as_ref()
                .expect("ArenaPtr<T>::new should be passed a valid Arena.");
            assert!(index < arena_ref.capacity());
        }
        ArenaPtr {
            arena: arena,
            index: index,
        }
    }
}

impl<T> Deref<T> for ArenaPtr<T> {
    fn deref<'a>(&'a self) -> &'a T {
        unsafe {
            let arena = self.arena.as_ref()
                .expect("ArenaPtr::deref should always have an Arena.");
            &arena.pool[self.index]
        }
    }
}

impl<T> DerefMut<T> for ArenaPtr<T> {
    fn deref_mut<'a>(&'a mut self) -> &'a mut T {
        unsafe {
            let arena = self.arena.as_mut()
                .expect("ArenaPtr::deref_mut should always have an Arena.");
            &mut arena.pool[self.index]
        }
    }
}

impl<T> fmt::Show for ArenaPtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ArenaPtr({}, {})", self.arena.to_uint(), self.index)
    }
}

impl<T> cmp::PartialEq for ArenaPtr<T> {
    /// Note that `PartialEq` implements pointer object identity, not structural
    /// comparison. In other words, it is equivalent to the scheme function
    /// `eq?`, not the scheme function `equal?`
    fn eq(&self, other: &ArenaPtr<T>) -> bool {
        self.index == other.index && self.arena.to_uint() == other.arena.to_uint()
    }
}

/// A pointer to a cons cell on the heap.
pub type ConsPtr = ArenaPtr<Cons>;

/// A pointer to a string on the heap.
pub type StringPtr = ArenaPtr<String>;

/// A pointer to an `Environment` on the heap.
pub type EnvironmentPtr = ArenaPtr<Environment>;

/// The scheme heap, containing all allocated cons cells and strings (including
/// strings for symbols).
pub struct Heap {
    cons_cells: Arena<Cons>,
    strings: Arena<String>,
    environments: Arena<Environment>,
}

/// The default capacity of cons cells.
pub static DEFAULT_CONS_CAPACITY : uint = 1 << 12;

/// The default capacity of strings.
pub static DEFAULT_STRINGS_CAPACITY : uint = 1 << 12;

/// The default capacity of environments.
pub static DEFAULT_ENVIRONMENTS_CAPACITY : uint = 1 << 12;

impl Heap {
    /// Create a new `Heap` with the default capacity.
    pub fn new() -> Heap {
        Heap::with_arenas(Arena::new(DEFAULT_CONS_CAPACITY),
                          Arena::new(DEFAULT_STRINGS_CAPACITY),
                          Arena::new(DEFAULT_ENVIRONMENTS_CAPACITY))
    }

    /// Create a new `Heap` using the given arenas for allocating cons cells and
    /// strings within.
    pub fn with_arenas(cons_cells: Arena<Cons>,
                       strings: Arena<String>,
                       envs: Arena<Environment>) -> Heap {
        Heap {
            cons_cells: cons_cells,
            strings: strings,
            environments: envs,
        }
    }

    /// Allocate a new cons cell and return a pointer to it.
    ///
    /// ## Panics
    ///
    /// Panics if the `Arena` for cons cells has already reached capacity.
    pub fn allocate_cons(&mut self) -> ConsPtr {
        self.cons_cells.allocate()
    }

    /// Allocate a new string and return a pointer to it.
    ///
    /// ## Panics
    ///
    /// Panics if the `Arena` for strings has already reached capacity.
    pub fn allocate_string(&mut self) -> StringPtr {
        self.strings.allocate()
    }

    /// Allocate a new `Environment` and return a pointer to it.
    ///
    /// ## Panics
    ///
    /// Panics if the `Arena` for environments has already reached capacity.
    pub fn allocate_environment(&mut self) -> EnvironmentPtr {
        self.environments.allocate()
    }
}