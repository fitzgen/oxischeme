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
//! 1. The `Environment` associates symbols with a concrete location. This is
//! used during the syntactic analysis.
//!
//! 2. `Activation`s are those concrete locations at runtime, and contain just
//! the values passed in at function invocation. After syntactic analysis, we
//! only deal with activations, and we no longer need the symbols or the
//! `Environment`.

use std::collections::{HashMap};
use std::default::{Default};
use std::fmt;
use std::hash;

use heap::{ArenaPtr, GcThing, Heap, IterGcThing, Rooted, ToGcThing, Trace};
use value::{Value, RootedValue};

/// An `Activation` represents the values extending the lexical environment on
/// each function invocation.
pub struct Activation {
    /// TODO FITZGEN
    parent: Option<ActivationPtr>,
    /// TODO FITZGEN
    args: Vec<Option<Value>>,
}

impl Activation {
    /// Extend the given `Activation` with the values supplied, resulting in a
    /// new `Activation` instance.
    pub fn extend(heap: &mut Heap,
                  parent: &RootedActivationPtr,
                  values: Vec<RootedValue>) -> RootedActivationPtr {
        let mut act = heap.allocate_activation();
        act.parent = Some(**parent);
        act.args = values.into_iter().map(|v| Some(*v)).collect();
        return act;
    }

    /// TODO FITZGEN
    ///
    /// Returns an error when trying to fetch a value that has not yet ben
    /// defined.
    pub fn fetch(&self,
                 heap: &mut Heap,
                 i: u32,
                 j: u32) -> Result<RootedValue, ()> {
        if i == 0 {
            let jj = j as usize;
            if jj >= self.args.len() {
                return Err(());
            }

            if let Some(val) = self.args[jj] {
                return Ok(Rooted::new(heap, val));
            }

            return Err(());
        }

        return self.parent.expect("Activation::fetch: i out of bounds")
            .fetch(heap, i - 1, j);
    }

    /// TODO FITZGEN
    ///
    /// Returns an error when trying to set a value that has not yet been
    /// defined.
    pub fn update(&mut self,
                  i: u32,
                  j: u32,
                  val: &RootedValue) -> Result<(), ()> {
        if i == 0 {
            let jj = j as usize;
            if jj >= self.args.len() || self.args[jj].is_none() {
                return Err(());
            }

            self.args[jj] = Some(**val);
            return Ok(());
        }

        return self.parent.expect("Activation::update: i out of bounds")
            .update(i - 1, j, val);
    }

    /// TODO FITZGEN
    fn fill_to(&mut self, n: u32) {
        while self.len() < n + 1 {
            self.args.push(None);
        }
    }

    /// TODO FITZGEN
    pub fn define(&mut self, j: u32, val: Value) {
        self.fill_to(j);
        self.args[j as usize] = Some(val);
    }

    /// TODO FITZGEN
    #[inline]
    pub fn len(&self) -> u32 {
        self.args.len() as u32
    }
}

impl<S: hash::Writer + hash::Hasher> hash::Hash<S> for Activation {
    fn hash(&self, state: &mut S) {
        self.parent.hash(state);
        for v in self.args.iter() {
            v.hash(state);
        }
    }
}

impl Default for Activation {
    fn default() -> Activation {
        Activation {
            parent: None,
            args: vec!(),
        }
    }
}

impl Trace for Activation {
    fn trace(&self) -> IterGcThing {
        let mut results: Vec<GcThing> = self.args.iter()
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

impl fmt::String for Activation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "(activation :length {}\n", self.args.len()));
        try!(write!(f, "            :parent "));
        if let Some(ref p) = self.parent {
            write!(f, "Some({}))", **p)
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

/// TODO FITZGEN
pub struct Environment {
    /// TODO FITZGEN
    bindings: Vec<HashMap<String, u32>>,
}

impl Environment {
    /// TODO FITZGEN
    pub fn new() -> Environment {
        Environment {
            bindings: vec!(HashMap::new())
        }
    }

    /// TODO FITZGEN
    pub fn extend(&mut self, names: Vec<String>) {
        self.bindings.push(HashMap::new());
        for n in names.into_iter() {
            self.define(n);
        }
    }

    /// TODO FITZGEN
    pub fn pop(&mut self) {
        assert!(self.bindings.len() > 1,
                "Should never pop off the global environment");
        self.bindings.pop();
    }

    /// TODO FITZGEN
    fn youngest<'a>(&'a mut self) -> &'a mut HashMap<String, u32> {
        let last_idx = self.bindings.len() - 1;
        &mut self.bindings[last_idx]
    }

    /// TODO FITZGEN
    pub fn define(&mut self, name: String) -> (u32, u32) {
        if let Some(n) = self.youngest().get(&name) {
            return (0, *n);
        }

        let n = self.youngest().len() as u32;
        self.youngest().insert(name, n);
        return (0, n);
    }

    /// TODO FITZGEN
    pub fn define_global(&mut self, name: String) -> (u32, u32) {
        let n = self.bindings[0].len() as u32;
        self.bindings[0].insert(name, n);
        return ((self.bindings.len() - 1) as u32, n);
    }

    /// TODO FITZGEN
    pub fn lookup(&self, name: &String) -> Option<(u32, u32)> {
        for (i, bindings) in self.bindings.iter().rev().enumerate() {
            if let Some(j) = bindings.get(name) {
                return Some((i as u32, *j));
            }
        }
        return None;
    }
}