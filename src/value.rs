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

/// `Value` is a scheme value.
#[deriving(Copy, PartialEq, Show)]
pub enum Value {
    /// Scheme integers are represented as 64 bit integers.
    Integer(i64),
    /// Scheme booleans are represented with `bool`.
    Boolean(bool),
    /// Scheme characters are `char`s.
    Character(char),
}

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
}

/// The `#t` singleton value.
pub static TRUE : Value = Value::Boolean(true);
/// The `#f` singleton value.
pub static FALSE : Value = Value::Boolean(false);