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

use std::io;
use value;

/// Print the given value's text representation to the given writer.
pub fn print<W: Writer>(val: value::Value, writer: &mut W) -> io::IoResult<()> {
    match val {
        value::Value::Integer(i) => write!(writer, "{}", i),
    }
}