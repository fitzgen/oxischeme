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

use std::io;

pub mod context;
pub mod environment;
pub mod eval;
pub mod heap;
pub mod print;
pub mod read;
pub mod value;

/// The main Read-Eval-Print-Loop.
pub fn main() {
    println!("Welcome to oxischeme!");
    println!("C-c to exit.");
    println!("");

    let mut ctx = context::Context::new();

    loop {
        let mut stdout = io::stdio::stdout();
        let stdin = io::stdio::stdin();
        let mut reader = read::Read::new(stdin, &mut ctx);

        print!("oxischeme> ");
        for form in reader {
            match eval::evaluate_in_global_env(&mut ctx, form) {
                Ok(val) => {
                    print::print(val, &mut stdout).ok().expect("IO ERROR!");
                },
                Err(e) => {
                    (write!(&mut stdout, "Error: {}", e)).ok().expect("IO ERROR!");
                },
            };
            (write!(&mut stdout, "\n")).ok().expect("IO ERROR!");

            let heap = ctx.heap();
            heap.collect_garbage(&ctx);

            (write!(&mut stdout, "oxischeme> ")).ok().expect("IO ERROR!");
            stdout.flush().ok().expect("IO ERROR!");
        }

        match *reader.get_result() {
            Ok(_) => return,
            Err(ref msg) => {
                (write!(&mut stdout, "{}", msg)).ok().expect("IO ERROR!");
                (write!(&mut stdout, "\n")).ok().expect("IO ERROR!");
                stdout.flush().ok().expect("IO ERROR!");
            }
        }
    }
}
