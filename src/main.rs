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

//! A Scheme implementation, in Rust.

#![feature(unsafe_destructor)]
#![allow(unstable)]

use std::io;
use std::os;

pub mod environment;
pub mod eval;
pub mod heap;
pub mod primitives;
pub mod read;
pub mod value;

/// Start a Read -> Evaluate -> Print loop.
pub fn repl(heap: &mut heap::Heap) {
    println!("Welcome to oxischeme!");
    println!("C-c to exit.");
    println!("");

    loop {
        let stdin = io::stdio::stdin();
        let mut reader = read::Read::new(stdin, heap);

        print!("oxischeme> ");
        for form in reader {
            match eval::evaluate(heap, &form) {
                Ok(val) => println!("{}", *val),
                Err(e)  => println!("Error: {}", e),
            };

            heap.collect_garbage();
            print!("oxischeme> ");
        }

        match *reader.get_result() {
            Ok(_) => return,
            Err(ref msg) => println!("{}", msg),
        }
    }
}

/// Given no arguments, start the REPL. Otherwise, treat each argument as a file
/// path and read and evaluate each of them in turn.
pub fn main() {
    let heap = &mut heap::Heap::new();

    let mut args_were_passed = false;

    for file_path in os::args().iter().skip(1) {
        args_were_passed = true;

        match eval::evaluate_file(heap, file_path.as_slice()) {
            Ok(_) => { },
            Err(msg) => {
                let mut stderr = io::stdio::stderr();
                (write!(&mut stderr, "Error: {}", msg)).ok().expect("IO ERROR!");
                return;
            }
        }
    }

    if !args_were_passed {
        repl(heap);
    }
}
