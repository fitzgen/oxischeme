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
use std::collections::{HashSet};
use std::default::{Default};
use std::fmt;
use std::ptr;
use std::vec::{IntoIter};

use context::{Context};
use environment::{Environment};
use value::{Cons, Procedure};

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
#[allow(raw_pointer_deriving)]
#[deriving(Hash)]
pub struct ArenaPtr<T> {
    arena: *mut Arena<T>,
    index: uint,
}

// XXX: We have to manually declare that ArenaPtr<T> is copy-able because if we
// use `#[deriving(Copy)]` it wants T to be copy-able as well, despite the fact
// that we only need to copy our pointer to the Arena<T>, not any T or the Arena
// itself.
impl<T> ::std::kinds::Copy for ArenaPtr<T> { }

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

    /// Get the null ArenaPtr<T>. Should never actually be used, but sometimes
    /// it is needed for initializing a struct's default, uninitialized form.
    pub fn null() -> ArenaPtr<T> {
        ArenaPtr {
            arena: ptr::null_mut(),
            index: 0,
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
    /// `eq?`, not the scheme function `equal?`.
    fn eq(&self, other: &ArenaPtr<T>) -> bool {
        self.index == other.index && self.arena.to_uint() == other.arena.to_uint()
    }
}

impl<T> cmp::Eq for ArenaPtr<T> { }

/// A pointer to a cons cell on the heap.
pub type ConsPtr = ArenaPtr<Cons>;

/// A pointer to a string on the heap.
pub type StringPtr = ArenaPtr<String>;

/// A pointer to an `Environment` on the heap.
pub type EnvironmentPtr = ArenaPtr<Environment>;

/// A pointer to a `Procedure` on the heap.
pub type ProcedurePtr = ArenaPtr<Procedure>;

/// The scheme heap, containing all allocated cons cells and strings (including
/// strings for symbols).
pub struct Heap {
    cons_cells: Arena<Cons>,
    strings: Arena<String>,
    environments: Arena<Environment>,
    procedures: Arena<Procedure>,
}

/// The default capacity of cons cells.
pub static DEFAULT_CONS_CAPACITY : uint = 1 << 12;

/// The default capacity of strings.
pub static DEFAULT_STRINGS_CAPACITY : uint = 1 << 12;

/// The default capacity of environments.
pub static DEFAULT_ENVIRONMENTS_CAPACITY : uint = 1 << 12;

/// The default capacity of environments.
pub static DEFAULT_PROCEDURES_CAPACITY : uint = 1 << 12;

impl Heap {
    /// Create a new `Heap` with the default capacity.
    pub fn new() -> Heap {
        Heap::with_arenas(Arena::new(DEFAULT_CONS_CAPACITY),
                          Arena::new(DEFAULT_STRINGS_CAPACITY),
                          Arena::new(DEFAULT_ENVIRONMENTS_CAPACITY),
                          Arena::new(DEFAULT_PROCEDURES_CAPACITY))
    }

    /// Create a new `Heap` using the given arenas for allocating cons cells and
    /// strings within.
    pub fn with_arenas(cons_cells: Arena<Cons>,
                       strings: Arena<String>,
                       envs: Arena<Environment>,
                       procs: Arena<Procedure>) -> Heap {
        Heap {
            cons_cells: cons_cells,
            strings: strings,
            environments: envs,
            procedures: procs,
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

    /// Allocate a new `Procedure` and return a pointer to it.
    ///
    /// ## Panics
    ///
    /// Panics if the `Arena` for environments has already reached capacity.
    pub fn allocate_procedure(&mut self) -> ProcedurePtr {
        self.procedures.allocate()
    }
}

/// # Garbage Collection
///
/// TODO FITZGEN talk about `trace` API protocol.

pub type IterGcThing = IntoIter<GcThing>;

/// TODO FITZGEN
pub trait Trace {
    /// TODO FITZGEN
    fn trace(&self) -> IterGcThing;
}

impl Heap {
    /// TODO FTIZGEN
    pub fn collect_garbage(&mut self, ctx: &Context) {
        let mut marked = HashSet::new();
        let mut pending_trace: Vec<GcThing> = ctx.trace().collect();

        while !pending_trace.is_empty() {
            let mut newly_pending_trace = vec!();

            for thing in pending_trace.drain() {
                if !marked.contains(&thing) {
                    marked.insert(thing);
                    for referent in thing.trace() {
                        newly_pending_trace.push(referent);
                    }
                }
            }

            for thing in newly_pending_trace.drain() {
                pending_trace.push(thing);
            }
        }

        println!("-----------------------------------------------------------");
        for thing in marked.iter() {
            println!("FITZGEN: traced {}", thing);
        }
    }
}

/// TODO FITZGEN
#[deriving(Copy, Eq, Hash, PartialEq, Show)]
pub enum GcThing {
    /// TODO FITZGEN
    Cons(ConsPtr),

    /// TODO FITZGEN
    String(StringPtr),

    /// TODO FITZGEN
    Environment(EnvironmentPtr),

    /// TODO FITZGEN
    Procedure(ProcedurePtr),
}

/// ## `GcThing` Constructors
impl GcThing {
    /// TODO FITZGEN
    pub fn from_string_ptr(str: StringPtr) -> GcThing {
        GcThing::String(str)
    }

    /// TODO FITZGEN
    pub fn from_cons_ptr(cons: ConsPtr) -> GcThing {
        GcThing::Cons(cons)
    }

    /// TODO FITZGEN
    pub fn from_procedure_ptr(procedure: ProcedurePtr) -> GcThing {
        GcThing::Procedure(procedure)
    }

    /// TODO FITZGEN
    pub fn from_environment_ptr(env: EnvironmentPtr) -> GcThing {
        GcThing::Environment(env)
    }
}

impl Trace for GcThing {
    /// TODO FITZGEN
    fn trace(&self) -> IterGcThing {
        match *self {
            GcThing::Cons(cons)       => cons.trace(),
            GcThing::Environment(env) => env.trace(),
            GcThing::Procedure(p)     => p.trace(),
            // Strings don't hold any strong references to other `GcThing`s.
            GcThing::String(_)        => vec!().into_iter(),
        }
    }
}