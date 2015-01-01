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

use heap::{Heap, Rooted};
use value::{RootedConsPtr, RootedValue, Value};

/// Print the given value's text representation to the given writer.
pub fn print<W: Writer>(heap: &mut Heap,
                        writer: &mut W,
                        val: &RootedValue) -> IoResult<()> {
    match **val {
        Value::EmptyList        => write!(writer, "()"),
        Value::Pair(ref cons)   => {
            try!(write!(writer, "("));
            let rcons = Rooted::new(heap, *cons);
            try!(print_pair(heap, writer, &rcons));
            write!(writer, ")")
        },
        Value::String(ref str)  => {
            try!(write!(writer, "\""));
            try!(write!(writer, "{}", str.deref()));
            write!(writer, "\"")
        },
        Value::Symbol(ref s)    => write!(writer, "{}", s.deref()),
        Value::Integer(ref i)   => write!(writer, "{}", i),
        Value::Boolean(ref b)   => {
            write!(writer, "{}", if *b {
                "#t"
            } else {
                "#f"
            })
        },
        Value::Character(ref c) => match *c {
            '\n' => write!(writer, "#\\newline"),
            '\t' => write!(writer, "#\\tab"),
            ' '  => write!(writer, "#\\space"),
            _    => write!(writer, "#\\{}", c),
        },
        Value::Procedure(ref p) => write!(writer, "Procedure({})", p),
    }
}

/// Print the given cons pair, without the containing "(" and ")".
fn print_pair<W: Writer>(heap: &mut Heap,
                         writer: &mut W,
                         cons: &RootedConsPtr) -> IoResult<()> {
    let car = cons.car(heap);
    try!(print(heap, writer, &car));
    match *cons.cdr(heap) {
        Value::EmptyList => Ok(()),
        Value::Pair(cdr) => {
            try!(write!(writer, " "));
            let rcdr = Rooted::new(heap, cdr);
            print_pair(heap, writer, &rcdr)
        },
        val              => {
            try!(write!(writer, " . "));
            let rval = Rooted::new(heap, val);
            print(heap, writer, &rval)
        },
    }
}