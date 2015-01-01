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
//!
//! ### Rooting
//!
//! Sometimes it is necessary to temporarily root GC things referenced by
//! pointers on the stack. Garbage collection can be triggered by allocating any
//! GC thing, and it isn't always clear which rust functions (or other functions
//! called by those functions, or even other functions called by those functions
//! called from the first function, and so on) might allocate a GC thing and
//! trigger collection. The situation we want to avoid is a rust function using a
//! temporary variable that references a GC thing, then calling another function
//! which triggers a collection and collects the GC thing that was referred to by
//! the temporary variable, and the temporary variable is now a dangling
//! pointer. If the rust function accesses it again, that is undefined behavior:
//! it might still get the value it was pointing at, or it might be a segfault,
//! or it might be a freshly allocated value used by something else! Not good!
//!
//! Here is what this scenario looks like in psuedo code:
//!
//!     let a = pointer_to_some_gc_thing;
//!     function_which_can_trigger_gc();
//!     // Oops! A collection was triggered and dereferencing this pointer leads
//!     // to undefined behavior!
//!     *a;
//!
//! There are two possible solutions to this problem. The first is *conservative*
//! garbage collection, where we walk the stack and if anything on the stack
//! looks like it might be a pointer and if coerced to a pointer happens to point
//! to a GC thing in the heap, we assume that it *is* a pointer and we consider
//! the GC thing that may or may not actually be pointed to by a variable on the
//! stack a GC root. The second is *precise rooting*. With precise rooting, it is
//! the responsibility of the rust function's author to explicitly root and
//! unroot pointers to GC things used in variables on the stack.
//!
//! Oxischeme uses precise rooting. Precise rooting is implemented with the
//! `Rooted<GcThingPtr>` smart pointer type, which roots its referent upon
//! construction and unroots it when the smart pointer goes out of scope and is
//! dropped.
//!
//! Using precise rooting and `Rooted`, we can solve the dangling pointer
//! problem like this:
//!
//!     {
//!         // The pointed to GC thing gets rooted when wrapped with `Rooted`.
//!         let a = Rooted::new(heap, pointer_to_some_gc_thing);
//!         function_which_can_trigger_gc();
//!         // Dereferencing `a` is now safe, because the referent is a GC root!
//!         *a;
//!     }
//!     // `a` goes out of scope, and its referent is unrooted.
//!
//! Tips for working with precise rooting if your function allocates GC things,
//! or calls other functions which allocate GC things:
//!
//! * Accept GC thing parameters as `&Rooted<T>` or `&mut Rooted<T>` to ensure
//!   that callers properly root them.
//!
//! * Accept a `&mut Heap` parameter and return `Rooted<T>` for getters and
//!   methods that return GC things. This greatly alleviates potential
//!   foot-guns, as a caller would have to explicitly unwrap the smart pointer
//!   and store that in a new variable to cause a dangling pointer. It also
//!   cuts down on `Rooted<T>` construction boiler plate.
//!
//! * Always root GC things whose lifetime spans a call which could trigger a
//!   collection!
//!
//! * When in doubt, Just Root It!

use std::cmp;
use std::collections::{HashMap, HashSet};
use std::default::{Default};
use std::fmt;
use std::ptr;
use std::vec::{IntoIter};

use environment::{Environment, EnvironmentPtr, RootedEnvironmentPtr};
use value::{Cons, ConsPtr, Procedure, ProcedurePtr, RootedConsPtr,
            RootedProcedurePtr, RootedValue, Value};

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
    pub fn new(capacity: uint) -> Box<Arena<T>> {
        assert!(capacity > 0);
        box Arena {
            pool: range(0, capacity).map(|_| Default::default()).collect(),
            free: range(0, capacity).collect(),
        }
    }
}

impl<T> Arena<T> {
    /// Get this heap's capacity for simultaneously allocated cons cells.
    pub fn capacity(&self) -> uint {
        self.pool.len()
    }

    /// Return true if this arena is at full capacity, and false otherwise.
    pub fn is_full(&self) -> bool {
        self.free.is_empty()
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

    /// Sweep the arena and add any reclaimed objects back to the free list.
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

impl<T> ArenaPtr<T> {
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

/// A trait for types that can be coerced to a `GcThing`.
pub trait ToGcThing {
    /// Coerce this value to a `GcThing`.
    fn to_gc_thing(&self) -> Option<GcThing>;
}

/// A smart pointer wrapping the pointer type `T`. It keeps its referent rooted
/// while the smart pointer is in scope to prevent dangling pointers caused by a
/// garbage collection within the pointers lifespan. For more information see
/// the module level documentation about rooting.
pub struct Rooted<T> {
    heap: *mut Heap,
    ptr: T,
}

impl<T: ToGcThing> Rooted<T> {
    /// Create a new `Rooted<T>`, rooting the referent.
    pub fn new(heap: &mut Heap, ptr: T) -> Rooted<T> {
        let mut r = Rooted {
            heap: heap,
            ptr: ptr,
        };
        r.add_root();
        r
    }

    /// Unroot the current referent and replace it with the given referent,
    /// which then gets rooted.
    pub fn emplace(&mut self, rhs: T) {
        self.drop_root();
        self.ptr = rhs;
        self.add_root();
    }

    /// Add the current referent as a GC root.
    fn add_root(&mut self) {
        if let Some(r) = self.ptr.to_gc_thing() {
            unsafe {
                self.heap.as_mut()
                    .expect("Rooted<T>::drop should always have a Heap")
                    .add_root(r);
            }
        }
    }

    /// Unroot the current referent.
    fn drop_root(&mut self) {
        if let Some(r) = self.ptr.to_gc_thing() {
            unsafe {
                self.heap.as_mut()
                    .expect("Rooted<T>::drop should always have a Heap")
                    .drop_root(r);
            }
        }
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

impl<T: Copy + ToGcThing> Clone for Rooted<T> {
    fn clone(&self) -> Self {
        unsafe {
            let heap = self.heap.as_mut()
                .expect("Rooted<T>::clone should always have a Heap");
            Rooted::new(heap, self.ptr)
        }
    }
}

impl<T: PartialEq> PartialEq<Self> for Rooted<T> {
    fn eq(&self, rhs: &Self) -> bool {
        **self == **rhs
    }
}
impl<T: PartialEq + Eq> Eq<Self> for Rooted<T> { }

impl<T: PartialEq> PartialEq<T> for Rooted<T> {
    fn eq(&self, rhs: &T) -> bool {
        **self == *rhs
    }
}
impl<T: PartialEq + Eq> Eq<T> for Rooted<T> { }

impl<T: fmt::Show> fmt::Show for Rooted<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Rooted({})", self.ptr)
    }
}

/// A pointer to a string on the heap.
pub type StringPtr = ArenaPtr<String>;

impl ToGcThing for StringPtr {
    fn to_gc_thing(&self) -> Option<GcThing> {
        Some(GcThing::from_string_ptr(*self))
    }
}

/// A rooted pointer to a string on the heap.
pub type RootedStringPtr = Rooted<StringPtr>;

/// The scheme heap and GC runtime, containing all allocated cons cells,
/// environments, procedures, and strings (including strings for symbols).
pub struct Heap {
    cons_cells: Box<Arena<Cons>>,
    strings: Box<Arena<String>>,
    environments: Box<Arena<Environment>>,
    procedures: Box<Arena<Procedure>>,
    roots: HashMap<GcThing, uint>,
    symbol_table: HashMap<String, StringPtr>,
    global_environment: EnvironmentPtr,
    allocations: uint,
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
    pub fn with_arenas(cons_cells: Box<Arena<Cons>>,
                       strings: Box<Arena<String>>,
                       envs: Box<Arena<Environment>>,
                       procs: Box<Arena<Procedure>>) -> Heap {
        let mut e = envs;
        let global_env = e.allocate();
        Heap {
            cons_cells: cons_cells,
            strings: strings,
            environments: e,
            procedures: procs,
            roots: HashMap::new(),
            symbol_table: HashMap::new(),
            global_environment: global_env,
            allocations: 0,
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
        self.on_allocation();
        let c = self.cons_cells.allocate();
        Rooted::new(self, c)
    }

    /// Allocate a new string and return a pointer to it.
    ///
    /// ## Panics
    ///
    /// Panics if the `Arena` for strings has already reached capacity.
    pub fn allocate_string(&mut self) -> RootedStringPtr {
        self.on_allocation();
        let s = self.strings.allocate();
        Rooted::new(self, s)
    }

    /// Allocate a new `Environment` and return a pointer to it.
    ///
    /// ## Panics
    ///
    /// Panics if the `Arena` for environments has already reached capacity.
    pub fn allocate_environment(&mut self) -> RootedEnvironmentPtr {
        self.on_allocation();
        let e = self.environments.allocate();
        Rooted::new(self, e)
    }

    /// Allocate a new `Procedure` and return a pointer to it.
    ///
    /// ## Panics
    ///
    /// Panics if the `Arena` for environments has already reached capacity.
    pub fn allocate_procedure(&mut self) -> RootedProcedurePtr {
        self.on_allocation();
        let p = self.procedures.allocate();
        Rooted::new(self, p)
    }
}

/// The maximum number of things to allocate before triggering a garbage
/// collection.
const MAX_GC_PRESSURE : uint = 1 << 8;

/// ## `Heap` Methods for Garbage Collection
impl Heap {
    /// Perform a garbage collection on the heap.
    pub fn collect_garbage(&mut self) {
        self.reset_gc_pressure();

        // First, trace the heap graph and mark everything that is reachable.

        let mut marked = HashSet::new();
        let mut pending_trace = self.get_roots();

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

    /// Explicitly add the given GC thing as a root.
    pub fn add_root(&mut self, root: GcThing) {
        let zero = 0u;
        let current_count = *self.roots.get(&root).unwrap_or(&zero);
        self.roots.insert(root, current_count + 1);
    }

    /// Unroot a GC thing that was explicitly rooted with `add_root`.
    pub fn drop_root(&mut self, root: GcThing) {
        let current_count = *(self.roots.get(&root))
            .expect("Should never drop_root a gc thing that isn't rooted");
        if current_count == 1 {
            self.roots.remove(&root);
        } else {
            self.roots.insert(root, current_count - 1);
        }
    }

    /// Apply pressure to the GC, and if enough pressure has built up, then
    /// perform a garbage collection.
    pub fn increase_gc_pressure(&mut self) {
        self.allocations += 1;
        if self.is_too_much_pressure() {
            self.collect_garbage();
        }
    }

    /// Get a vector of all of the GC roots.
    fn get_roots(&self) -> Vec<GcThing> {
        let mut roots: Vec<GcThing> = self.symbol_table
            .values()
            .map(|s| GcThing::from_string_ptr(*s))
            .collect();

        roots.push(GcThing::from_environment_ptr(self.global_environment));

        for root in self.roots.keys() {
            roots.push(*root);
        }

        roots
    }

    /// Returns true if any of the heap's arenas are full, and false otherwise.
    fn any_arena_is_full(&self) -> bool {
        self.cons_cells.is_full()
            || self.strings.is_full()
            || self.procedures.is_full()
            || self.environments.is_full()
    }

    /// A method that should be called on every allocation. If any arenas are
    /// already full, it triggers a GC immediately, otherwise it builds GC
    /// pressure.
    fn on_allocation(&mut self)  {
        if self.any_arena_is_full() {
            self.collect_garbage();
        } else {
            self.increase_gc_pressure();
        }
    }

    /// Returns true when we have built up too much GC pressure, and it is time
    /// to collect garbage. False otherwise.
    fn is_too_much_pressure(&mut self) -> bool {
        self.allocations > MAX_GC_PRESSURE
    }

    /// Resets the GC pressure, so that it must build all the way back up to the
    /// max again before a GC is triggered.
    fn reset_gc_pressure(&mut self) {
        self.allocations = 0;
    }
}

/// ## `Heap` Methods and Accessors.
impl Heap {
    /// Get the global environment.
    pub fn global_env(&mut self) -> RootedEnvironmentPtr {
        let env = self.global_environment;
        Rooted::new(self, env)
    }

    /// Ensure that there is an interned symbol extant for the given `String`
    /// and return it.
    pub fn get_or_create_symbol(&mut self, str: String) -> RootedValue {
        if self.symbol_table.contains_key(&str) {
            let sym_ptr = self.symbol_table[str];
            let rooted_sym_ptr = Rooted::new(self, sym_ptr);
            return Value::new_symbol(self, rooted_sym_ptr);
        }

        let mut symbol = self.allocate_string();
        symbol.clear();
        symbol.push_str(str.as_slice());
        self.symbol_table.insert(str, *symbol);
        return Value::new_symbol(self, symbol);
    }
}

/// ## Getters for well known symbols.
impl Heap {
    pub fn quote_symbol(&mut self) -> RootedValue {
        self.get_or_create_symbol("quote".to_string())
    }

    pub fn if_symbol(&mut self) -> RootedValue {
        self.get_or_create_symbol("if".to_string())
    }

    pub fn begin_symbol(&mut self) -> RootedValue {
        self.get_or_create_symbol("begin".to_string())
    }

    pub fn define_symbol(&mut self) -> RootedValue {
        self.get_or_create_symbol("define".to_string())
    }

    pub fn set_bang_symbol(&mut self) -> RootedValue {
        self.get_or_create_symbol("set!".to_string())
    }

    pub fn unspecified_symbol(&mut self) -> RootedValue {
        self.get_or_create_symbol("unspecified".to_string())
    }

    pub fn lambda_symbol(&mut self) -> RootedValue {
        self.get_or_create_symbol("lambda".to_string())
    }
}

/// An iterable of `GcThing`s.
pub type IterGcThing = IntoIter<GcThing>;

/// The `Trace` trait allows GC participants to inform the collector of their
/// references to other GC things.
///
/// For example, imagine we had a `Trio` type that contained three cons cells:
///
///     struct Trio {
///         first: ConsPtr,
///         second: ConsPtr,
///         third: ConsPtr,
///     }
///
/// `Trio`'s implementation of `Trace` must yield all of its cons pointers, or
/// else their referents could be reclaimed by the garbage collector, and the
/// `Trio` would have dangling pointers, leading to undefined behavior and bad
/// things when it dereferences them in the future.
///
///     impl Trace for Trio {
///         fn trace(&self) -> IterGcThing {
///             let refs = vec!(GcThing::from_cons_ptr(self.first),
///                             GcThing::from_cons_ptr(self.second),
///                             GcThing::from_cons_ptr(self.third));
///             refs.into_iter()
///         }
///     }
pub trait Trace {
    /// Return an iterable of all of the GC things referenced by this structure.
    fn trace(&self) -> IterGcThing;
}

/// The union of the various types that are GC things.
#[deriving(Copy, Eq, Hash, PartialEq, Show)]
pub enum GcThing {
    Cons(ConsPtr),
    String(StringPtr),
    Environment(EnvironmentPtr),
    Procedure(ProcedurePtr),
}

/// ## `GcThing` Constructors
impl GcThing {
    /// Create a `GcThing` from a `StringPtr`.
    pub fn from_string_ptr(str: StringPtr) -> GcThing {
        GcThing::String(str)
    }

    /// Create a `GcThing` from a `ConsPtr`.
    pub fn from_cons_ptr(cons: ConsPtr) -> GcThing {
        GcThing::Cons(cons)
    }

    /// Create a `GcThing` from a `ProcedurePtr`.
    pub fn from_procedure_ptr(procedure: ProcedurePtr) -> GcThing {
        GcThing::Procedure(procedure)
    }

    /// Create a `GcThing` from a `EnvironmentPtr`.
    pub fn from_environment_ptr(env: EnvironmentPtr) -> GcThing {
        GcThing::Environment(env)
    }
}

impl Trace for GcThing {
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