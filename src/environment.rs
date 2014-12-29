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

//! The implementation of the Scheme environment that binds symbols to values.

use std::collections::{HashMap};
use std::default::{Default};
use std::fmt::{format};
use std::hash;

use heap::{ArenaPtr, GcThing, Heap, IterGcThing, Rooted, ToGcThing, Trace};
use value::{SchemeResult, Value, RootedValue};

/// The `Environment` associates symbols with values.
pub struct Environment {
    parent: Option<EnvironmentPtr>,
    bindings: HashMap<String, Value>
}

impl Environment {
    /// Extend the given environment with the names and associated values
    /// supplied, resulting in a new environment.
    pub fn extend(heap: &mut Heap,
                  parent: &RootedEnvironmentPtr,
                  names: &RootedValue,
                  values: &RootedValue) -> Result<RootedEnvironmentPtr, String> {
        let mut env = heap.allocate_environment();
        env.set_parent(parent);

        let names_len = try!(names.len().ok().ok_or(
            "Improperly formed parameters".to_string()));
        let values_len = try!(values.len().ok().ok_or(
            "Improperly formed values".to_string()));

        if names_len > values_len {
            return Err("Not enough values".to_string());
        } else if names_len < values_len {
            return Err("Too many values".to_string());
        }

        let mut names_ = names.clone();
        let mut values_ = values.clone();
        loop {
            let n = *names_;
            match n {
                Value::EmptyList  => {
                    return Ok(env);
                },
                Value::Pair(ref cons) => {
                    let sym = try!(cons.car(heap).to_symbol(heap).ok_or(
                        "Can't extend environment with non-symbol".to_string()));
                    let val = values_.car(heap).expect(
                        "Already verified that names.len() == values.len()");
                    env.define((**sym).clone(), &val);

                    names_.emplace(*cons.cdr(heap));

                    let next_val = values_.cdr(heap).expect(
                        "Already verified that names.len() == values.len()");
                    values_.emplace(*next_val);
                },
                _                 => {
                    return Err(
                        "Can't extend environment with improper list".to_string());
                }
            }
        }
    }

    /// Set the parent of this environment. When looking up bindings, if this
    /// environment doesn't have the target binding, and this environment has a
    /// parent environment, we will recurse to the parent and do a lookup in
    /// that environment, and so on until either there are no more environments
    /// or we find the binding.
    pub fn set_parent(&mut self, parent: &RootedEnvironmentPtr) {
        self.parent = Some(**parent);
    }

    /// Define a new variable bound to the given value.
    pub fn define(&mut self, sym: String, val: &RootedValue) {
        self.bindings.insert(sym, **val);
    }

    /// Update an *existing* binding to be associated with the new value.
    pub fn update(&mut self, sym: String, val: &RootedValue) -> Result<(), String> {
        if !self.bindings.contains_key(&sym) {
            let mut parent_env = try!(self.parent.ok_or(
                "Cannot set variable before its definition".to_string()));
            return parent_env.update(sym, val);
        }

        self.bindings.insert(sym, **val);
        return Ok(());
    }

    /// Lookup the value associated with the given symbol.
    pub fn lookup(&self, heap: &mut Heap, sym: &String) -> SchemeResult {
        if !self.bindings.contains_key(sym) {
            match self.parent {
                Some(env) => return env.lookup(heap, sym),
                _         => return Err(format_args!(
                    format, "Reference to undefined identifier: {}", sym)),
            };
        }

        let val = self.bindings.get(sym).expect(
            "self.bindings.contains(&sym), so we have to have the value.");
        return Ok(Rooted::new(heap, *val));
    }
}

impl<S: hash::Writer> hash::Hash<S> for Environment {
    fn hash(&self, state: &mut S) {
        self.parent.hash(state);
        for item in self.bindings.iter() {
            item.hash(state);
        }
    }
}

impl Default for Environment {
    fn default() -> Environment {
        Environment {
            parent: None,
            bindings: HashMap::new()
        }
    }
}

impl Trace for Environment {
    fn trace(&self) -> IterGcThing {
        let mut results = vec!();

        for val in self.bindings.values() {
            if let Some(gc_thing) = val.to_gc_thing() {
                results.push(gc_thing);
            }
        }

        if let Some(parent) = self.parent {
            results.push(GcThing::from_environment_ptr(parent));
        }

        results.into_iter()
    }
}

/// A pointer to an `Environment` on the heap.
pub type EnvironmentPtr = ArenaPtr<Environment>;

impl ToGcThing for EnvironmentPtr {
    fn to_gc_thing(&self) -> Option<GcThing> {
        Some(GcThing::from_environment_ptr(*self))
    }
}

/// A rooted pointer to an `Environment` on the heap.
pub type RootedEnvironmentPtr = Rooted<EnvironmentPtr>;