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

//! Printing values' text representations.

use std::io::{IoResult};
use value::{Value};

/// Print the given value's text representation to the given writer.
pub fn print<W: Writer>(val: Value, writer: &mut W) -> IoResult<()> {
    match val {
        Value::Integer(i) => write!(writer, "{}", i),
        Value::Boolean(b) => write!(writer, "{}", if b { "#t" } else { "#f" }),
        Value::Character(c) => match c {
            '\n' => write!(writer, "#\\newline"),
            '\t' => write!(writer, "#\\tab"),
            ' '  => write!(writer, "#\\space"),
            _    => write!(writer, "#\\{}", c),
        }
    }
}