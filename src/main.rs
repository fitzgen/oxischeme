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

#![feature(collections)]
#![feature(core)]
#![feature(env)]
#![feature(old_io)]
#![feature(old_path)]
#![feature(test)]
#![feature(unicode)]
#![feature(unsafe_destructor)]

use std::old_io;
use std::env;

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
        let stdin = old_io::stdio::stdin();
        let reader = read::Read::new(stdin, heap, "stdin".to_string());

        print!("oxischeme> ");
        for (location, read_result) in reader {
            match read_result {
                Err(msg) => {
                    println!("{}", msg);
                    break;
                },
                Ok(form) => {
                    match eval::evaluate(heap, &form, location) {
                        Ok(val) => println!("{}", *val),
                        Err(e)  => println!("{}", e),
                    };

                }
            }

            heap.collect_garbage();
            print!("oxischeme> ");
        }
    }
}

/// Given no arguments, start the REPL. Otherwise, treat each argument as a file
/// path and read and evaluate each of them in turn.
pub fn main() {
    let heap = &mut heap::Heap::new();

    let mut args_were_passed = false;

    for file_path in env::args().skip(1) {
        args_were_passed = true;

        match eval::evaluate_file(heap, file_path.as_slice()) {
            Ok(_) => { },
            Err(msg) => {
                let mut stderr = old_io::stdio::stderr();
                (write!(&mut stderr, "{}", msg)).ok().expect("IO ERROR!");
                return;
            }
        }
    }

    if !args_were_passed {
        repl(heap);
    }
}
