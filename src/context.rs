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

//! The `Context` is a collection of state required to run Scheme programs, such
//! as the `Heap` for allocating cons cells and strings within, as well as the
//! symbol table. The first step to running a Scheme program with Oxischeme, is
//! to create a `Context`.

use std::collections::{HashMap};
use std::mem;

use heap::{Heap, StringPtr, EnvironmentPtr};
use value::{Value};

/// A collection of state required to run Scheme programs, such as the `Heap`
/// and the symbol table.
pub struct Context {
    heap: *mut Heap,
    symbol_table: HashMap<String, StringPtr>,
    environment: EnvironmentPtr
}

impl<'a> Context {
    /// Create a new `Context` instance.
    pub fn new() -> Context {
        unsafe {
            Context::with_heap(mem::transmute(box Heap::new()))
        }
    }

    /// Create a new `Context` instance using the given `Heap`.
    pub fn with_heap(heap: *mut Heap) -> Context {
        unsafe {
            let mut heap_ref = heap.as_mut()
                .expect("Context::with_heap should always have a Heap");
            let env = heap_ref.allocate_environment();

            return Context {
                heap: heap,
                environment: env,
                symbol_table: HashMap::new()
            };
        }
    }

    /// Get the context's heap.
    pub fn heap(&'a mut self) -> &'a mut Heap {
        unsafe {
            self.heap.as_mut().expect("Context::heap should always have a Heap")
        }
    }

    /// Get the current environment. This is the dynamic environment, not the
    /// lexical environment.
    pub fn env(&self) -> EnvironmentPtr {
        self.environment
    }

    /// Ensure that there is an interned symbol extant for the given `String`
    /// and return it.
    pub fn get_or_create_symbol(&mut self, str: String) -> Value {
        if self.symbol_table.contains_key(&str) {
            return Value::new_symbol(self.symbol_table[str]);
        }

        let mut symbol = self.heap().allocate_string();
        symbol.clear();
        symbol.push_str(str.as_slice());
        self.symbol_table.insert(str, symbol);
        return Value::new_symbol(symbol);
    }
}

/// ## Getters for well known symbols.
impl Context {
    pub fn quote_symbol(&mut self) -> Value {
        self.get_or_create_symbol("quote".to_string())
    }

    pub fn if_symbol(&mut self) -> Value {
        self.get_or_create_symbol("if".to_string())
    }

    pub fn begin_symbol(&mut self) -> Value {
        self.get_or_create_symbol("begin".to_string())
    }

    pub fn define_symbol(&mut self) -> Value {
        self.get_or_create_symbol("define".to_string())
    }

    pub fn set_bang_symbol(&mut self) -> Value {
        self.get_or_create_symbol("set!".to_string())
    }

    pub fn unspecified_symbol(&mut self) -> Value {
        self.get_or_create_symbol("unspecified".to_string())
    }
}