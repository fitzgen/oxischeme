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

//! The implementation of the Scheme environment binding symbols to values.
//!
//! This is split into two pieces:
//!
//! 1. The `Environment` associates symbol names with a concrete location where
//! the symbol's value can be found at runime. This is static information used
//! during the syntactic analysis.
//!
//! 2. `Activation`s are instances of lexical blocks (either a lambda invocation,
//! or the global top level) at runtime. They only contain values and none of the
//! metadata mapping names to these values. After syntactic analysis, we only
//! deal with activations, and we no longer need the symbols nor the
//! `Environment`.

use std::collections::{HashMap};
use std::default::{Default};
use std::fmt;
use std::hash;

use heap::{ArenaPtr, GcThing, Heap, IterGcThing, Rooted, ToGcThing, Trace};
use value::{Value, RootedValue};

/// An `Activation` represents a runtime instance of a lexical block (either a
/// lambda or the global top-level).
pub struct Activation {
    /// The parent scope, or `None` if this is the global activation.
    parent: Option<ActivationPtr>,
    /// For a lambda with N arguments, the first N slots are those arguments
    /// respectively. The rest are local definitions. If a slot is `None`, then
    /// it's variable hasn't been defined yet (but is referenced by something
    /// and potentially will be defined in the future).
    vals: Vec<Option<Value>>,
}

impl Activation {
    /// Extend the given `Activation` with the values supplied, resulting in a
    /// new `Activation` instance.
    pub fn extend(heap: &mut Heap,
                  parent: &RootedActivationPtr,
                  values: Vec<RootedValue>) -> RootedActivationPtr {
        let mut act = heap.allocate_activation();
        act.parent = Some(**parent);
        act.vals = values.into_iter().map(|v| Some(*v)).collect();
        return act;
    }

    /// Fetch the j'th variable from the i'th lexical activation.
    ///
    /// Returns an error when trying to fetch the value of a variable that has
    /// not yet been defined.
    pub fn fetch(&self,
                 heap: &mut Heap,
                 i: u32,
                 j: u32) -> Result<RootedValue, ()> {
        if i == 0 {
            let jj = j as usize;
            if jj >= self.vals.len() {
                return Err(());
            }

            if let Some(val) = self.vals[jj] {
                return Ok(Rooted::new(heap, val));
            }

            return Err(());
        }

        return self.parent.expect("Activation::fetch: i out of bounds")
            .fetch(heap, i - 1, j);
    }

    /// Set the j'th variable from the i'th lexical activation to the given
    /// value.
    ///
    /// Returns an error when trying to set a variable that has not yet been
    /// defined.
    pub fn update(&mut self,
                  i: u32,
                  j: u32,
                  val: &RootedValue) -> Result<(), ()> {
        if i == 0 {
            let jj = j as usize;
            if jj >= self.vals.len() || self.vals[jj].is_none() {
                return Err(());
            }

            self.vals[jj] = Some(**val);
            return Ok(());
        }

        return self.parent.expect("Activation::update: i out of bounds")
            .update(i - 1, j, val);
    }

    fn fill_to(&mut self, n: u32) {
        while self.len() < n + 1 {
            self.vals.push(None);
        }
    }

    /// Define the j'th variable of this activation to be the given value.
    pub fn define(&mut self, j: u32, val: Value) {
        self.fill_to(j);
        self.vals[j as usize] = Some(val);
    }

    #[inline]
    fn len(&self) -> u32 {
        self.vals.len() as u32
    }
}

impl<S: hash::Writer + hash::Hasher> hash::Hash<S> for Activation {
    fn hash(&self, state: &mut S) {
        self.parent.hash(state);
        for v in self.vals.iter() {
            v.hash(state);
        }
    }
}

impl Default for Activation {
    fn default() -> Activation {
        Activation {
            parent: None,
            vals: vec!(),
        }
    }
}

impl Trace for Activation {
    fn trace(&self) -> IterGcThing {
        let mut results: Vec<GcThing> = self.vals.iter()
            .filter_map(|v| {
                if let Some(val) = *v {
                    val.to_gc_thing()
                } else {
                    None
                }
            })
            .collect();

        if let Some(parent) = self.parent {
            results.push(GcThing::from_activation_ptr(parent));
        }

        results.into_iter()
    }
}

impl fmt::Debug for Activation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "(activation :length {}\n", self.vals.len()));
        try!(write!(f, "            :parent "));
        if let Some(ref p) = self.parent {
            write!(f, "Some({:?}))", **p)
        } else {
            write!(f, "None)")
        }
    }
}

/// A pointer to an `Activation` on the heap.
pub type ActivationPtr = ArenaPtr<Activation>;

impl ToGcThing for ActivationPtr {
    fn to_gc_thing(&self) -> Option<GcThing> {
        Some(GcThing::from_activation_ptr(*self))
    }
}

/// A rooted pointer to an `Activation` on the heap.
pub type RootedActivationPtr = Rooted<ActivationPtr>;

/// The `Environment` represents what we know about bindings statically, during
/// syntactic analysis.
pub struct Environment {
    /// A hash map for each lexical block we are currently in, which maps from a
    /// variable name to its position in any activations that get created for
    /// this block.
    bindings: Vec<HashMap<String, u32>>,
}

impl Environment {
    /// Create a new `Environemnt`.
    pub fn new() -> Environment {
        Environment {
            bindings: vec!(HashMap::new())
        }
    }

    /// Extend the environment with a new lexical block with the given set of
    /// variables.
    pub fn extend(&mut self, names: Vec<String>) {
        self.bindings.push(HashMap::new());
        for n in names.into_iter() {
            self.define(n);
        }
    }

    /// Pop off the youngest lexical block.
    pub fn pop(&mut self) {
        assert!(self.bindings.len() > 1,
                "Should never pop off the global environment");
        self.bindings.pop();
    }

    /// Define a variable in the youngest block and return the coordinates to
    /// get its value from an activation at runtime.
    pub fn define(&mut self, name: String) -> (u32, u32) {
        if let Some(n) = self.youngest().get(&name) {
            return (0, *n);
        }

        let n = self.youngest().len() as u32;
        self.youngest().insert(name, n);
        return (0, n);
    }

    /// Define a global variable and return its activation coordinates.
    pub fn define_global(&mut self, name: String) -> (u32, u32) {
        let n = self.bindings[0].len() as u32;
        self.bindings[0].insert(name, n);
        return ((self.bindings.len() - 1) as u32, n);
    }

    /// Get the activation coordinates associated with the given variable name.
    pub fn lookup(&self, name: &String) -> Option<(u32, u32)> {
        for (i, bindings) in self.bindings.iter().rev().enumerate() {
            if let Some(j) = bindings.get(name) {
                return Some((i as u32, *j));
            }
        }
        return None;
    }

    fn youngest<'a>(&'a mut self) -> &'a mut HashMap<String, u32> {
        let last_idx = self.bindings.len() - 1;
        &mut self.bindings[last_idx]
    }
}