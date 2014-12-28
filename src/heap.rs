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

//! The `heap` module provides memory management for our Scheme implementation.
//!
//! ## Allocation
//!
//! Scheme has a variety of types that must be allocated on the heap: cons cells,
//! strings, procedures, and vectors (currently unimplemented). Oxischeme's
//! current allocation strategy is deliberately as simple as possible. We
//! represent the heap as a set of "arenas", one arena for each type that must be
//! allocated on the heap. An "arena" is a pre-allocated vector of objects. We
//! keep track of an arena's un-used objects with a "free list" of indices into
//! its vector. When we allocate a new object, we remove an entry from the free
//! list and return a pointer to the object at that entry's index. Garbage
//! collection adds new entries to the free list when reclaiming dead
//! objects. When allocating, if the arena's vector is already at capacity (ie,
//! the free list is empty), we panic.
//!
//! ## Garbage Collection
//!
//! Any type that is heap-allocated must be *garbage collected* so that the
//! memory of no-longer used instances of that type can be reclaimed for
//! reuse. This provides the illusion of infinite memory, and frees Scheme
//! programmers from manually managing allocations and frees. We refer to
//! GC-managed types as "GC things". Note that a GC thing does not need to be a
//! Scheme value type: environments are also managed by the GC, but are not a
//! first class Scheme value.
//!
//! Any structure that has references to a garbage collected type must
//! *participate in garbage collection* by telling the garbage collector about
//! all of the GC things it is holding alive. Participation is implemented via
//! the `Trace` trait. Note that the set of types that participate in garbage
//! collection is not the same as the set of all GC things. Some GC things do not
//! participate in garbage collection: strings do not hold references to any
//! other GC things.
//!
//! A "GC root" is a GC participant that is always reachable. For example, the
//! global environment is a root because global variables must always be
//! accessible.
//!
//! We use a simple *mark and sweep* garbage collection algorithm. In the mark
//! phase, we start from the roots and recursively trace every reachable object
//! in the heap graph, adding them to our "marked" set. If a GC thing is not
//! reachable, then it is impossible for the Scheme program to use it in the
//! future, and it is safe for the garbage collector to reclaim it. The
//! unreachable objects are the set of GC things that are not in the marked
//! set. We find these unreachable objects and return them to their respective
//! arena's free list in the sweep phase.

use std::cmp;
use std::collections::{HashMap, HashSet};
use std::default::{Default};
use std::fmt;
use std::ptr;
use std::vec::{IntoIter};

use context::{Context};
use environment::{Environment, EnvironmentPtr, RootedEnvironmentPtr};
use value::{Cons, ConsPtr, Procedure, ProcedurePtr, RootedConsPtr};

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
                let res = ArenaPtr::new(self_ptr, idx);
                println!("FITZGEN: allocated {}", &res);
                res
            },
        }
    }

    /// TODO FITZGEN
    pub fn sweep(&mut self, live: HashSet<uint>) {
        self.free = range(0, self.capacity())
            .filter(|n| !live.contains(n))
            .collect();
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

/// TODO FITZGEN
pub trait ToGcThing {
    /// TODO FITZGEN
    fn to_gc_thing(&self) -> Option<GcThing>;
}

/// TODO FITZGEN
pub struct Rooted<T> {
    heap: *mut Heap,
    ptr: T,
}

impl<T: ToGcThing> Rooted<T> {
    /// TODO FITZGEN
    pub fn new(heap: &mut Heap, ptr: T) -> Rooted<T> {
        let mut r = Rooted {
            heap: heap,
            ptr: ptr,
        };
        r.add_root();
        r
    }

    /// TODO FITZGEN
    fn add_root(&mut self) {
        if let Some(r) = self.ptr.to_gc_thing() {
            unsafe {
                self.heap.as_mut()
                    .expect("Rooted<T>::drop should always have a Context")
                    .add_root(r);
            }
        }
    }

    /// TODO FITZGEN
    fn drop_root(&mut self) {
        if let Some(r) = self.ptr.to_gc_thing() {
            unsafe {
                self.heap.as_mut()
                    .expect("Rooted<T>::drop should always have a Context")
                    .drop_root(r);
            }
        }
    }

    /// TODO FITZGEN
    pub fn emplace(&mut self, rhs: T) {
        self.drop_root();
        self.ptr = rhs;
        self.add_root();
    }
}

impl<T> Deref<T> for Rooted<T> {
    fn deref<'a>(&'a self) -> &'a T {
        &self.ptr
    }
}

impl<T> DerefMut<T> for Rooted<T> {
    fn deref_mut<'a>(&'a mut self) -> &'a mut T {
        &mut self.ptr
    }
}

#[unsafe_destructor]
impl<T: ToGcThing> Drop for Rooted<T> {
    fn drop(&mut self) {
        self.drop_root();
    }
}

/// A pointer to a string on the heap.
pub type StringPtr = ArenaPtr<String>;

impl ToGcThing for StringPtr {
    fn to_gc_thing(&self) -> Option<GcThing> {
        Some(GcThing::from_string_ptr(*self))
    }
}

/// TODO FITZGEN
pub type RootedStringPtr = Rooted<StringPtr>;

/// The scheme heap, containing all allocated cons cells and strings (including
/// strings for symbols).
pub struct Heap {
    cons_cells: Arena<Cons>,
    strings: Arena<String>,
    environments: Arena<Environment>,
    procedures: Arena<Procedure>,
    roots: HashMap<GcThing, uint>,
}

/// The default capacity of cons cells.
pub static DEFAULT_CONS_CAPACITY : uint = 1 << 12;

/// The default capacity of strings.
pub static DEFAULT_STRINGS_CAPACITY : uint = 1 << 12;

/// The default capacity of environments.
pub static DEFAULT_ENVIRONMENTS_CAPACITY : uint = 1 << 12;

/// The default capacity of environments.
pub static DEFAULT_PROCEDURES_CAPACITY : uint = 1 << 12;

/// ## `Heap` Constructors
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
            roots: HashMap::new(),
        }
    }
}

/// ## `Heap` Allocation Methods
impl Heap {
    /// Allocate a new cons cell and return a pointer to it.
    ///
    /// ## Panics
    ///
    /// Panics if the `Arena` for cons cells has already reached capacity.
    pub fn allocate_cons(&mut self) -> RootedConsPtr {
        let cons = self.cons_cells.allocate();
        Rooted::new(self, cons)
    }

    /// Allocate a new string and return a pointer to it.
    ///
    /// ## Panics
    ///
    /// Panics if the `Arena` for strings has already reached capacity.
    pub fn allocate_string(&mut self) -> RootedStringPtr {
        let str = self.strings.allocate();
        Rooted::new(self, str)
    }

    /// Allocate a new `Environment` and return a pointer to it.
    ///
    /// ## Panics
    ///
    /// Panics if the `Arena` for environments has already reached capacity.
    pub fn allocate_environment(&mut self) -> RootedEnvironmentPtr {
        let env = self.environments.allocate();
        Rooted::new(self, env)
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

/// ## Garbage Collection
impl Heap {
    /// TODO FTIZGEN
    pub fn collect_garbage(&mut self, ctx: &Context) {
        // First, trace the heap graph and mark everything that is reachable.

        let mut marked = HashSet::new();
        // TODO FITZGEN: make a method for getting roots
        let mut pending_trace: Vec<GcThing> = ctx.trace().collect();
        for root in self.roots.keys() {
            pending_trace.push(*root);
        }

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

        // Second, divide the marked set by arena, and sweep each arena.

        let mut live_strings = HashSet::new();
        let mut live_envs = HashSet::new();
        let mut live_cons_cells = HashSet::new();
        let mut live_procs = HashSet::new();

        for thing in marked.into_iter() {
            match thing {
                GcThing::String(p)      => live_strings.insert(p.index),
                GcThing::Environment(p) => live_envs.insert(p.index),
                GcThing::Cons(p)        => live_cons_cells.insert(p.index),
                GcThing::Procedure(p)   => live_procs.insert(p.index),
            };
        }

        self.strings.sweep(live_strings);
        self.environments.sweep(live_envs);
        self.cons_cells.sweep(live_cons_cells);
        self.procedures.sweep(live_procs);
    }

    /// TODO FITZGEN
    pub fn add_root(&mut self, root: GcThing) {
        let zero = 0u;
        let current_count = *self.roots.get(&root).unwrap_or(&zero);
        self.roots.insert(root, current_count + 1);
    }

    /// TODO FITZGEN
    pub fn drop_root(&mut self, root: GcThing) {
        let current_count = *self.roots.get(&root)
            .expect("Shouldn't drop a gc thing that isn't rooted");
        if current_count == 1 {
            self.roots.remove(&root);
        } else {
            self.roots.insert(root, current_count - 1);
        }
    }
}

/// TODO FITZGEN
pub type IterGcThing = IntoIter<GcThing>;

/// TODO FITZGEN
pub trait Trace {
    /// TODO FITZGEN
    fn trace(&self) -> IterGcThing;
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