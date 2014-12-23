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

use std::fmt::{format};
use std::collections::{HashMap};
use value::{SchemeResult, Value};

/// The `Environment` associates symbols with values.
pub struct Environment {
    bindings: HashMap<String, Value>
}

impl Environment {
    /// Create a new `Environment`.
    pub fn new() -> Environment {
        Environment {
            bindings: HashMap::new()
        }
    }

    /// Define a new variable bound to the given value.
    pub fn define(&mut self, sym: String, val: Value) {
        self.bindings.insert(sym, val);
    }

    /// Update an *existing* binding to be associated with the new value.
    pub fn update(&mut self, sym: String, val: Value) -> Result<(), String> {
        if !self.bindings.contains_key(&sym) {
            return Err("Cannot set variable before its definition".to_string());
        }

        self.bindings.insert(sym, val);
        return Ok(());
    }

    /// Lookup the value associated with the given symbol.
    pub fn lookup(&self, sym: &String) -> SchemeResult {
        let val = try!(self.bindings.get(sym).ok_or(
            format_args!(format, "Reference to undefined identifier: {}", sym)));
        return Ok(*val);
    }
}
