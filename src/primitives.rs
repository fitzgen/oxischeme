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

//! Implementation of primitive procedures.

use environment::{ActivationPtr, Environment};
use heap::{Heap, Rooted};
use value::{RootedValue, SchemeResult, Value};

/// The function signature for primitives.
pub type PrimitiveFunction = fn(&mut Heap, &RootedValue) -> SchemeResult;

fn require_arity(name: &'static str,
                 args: &RootedValue,
                 n: u64) -> Result<(), String> {
    if let Ok(m) = args.len() {
        if m == n {
            return Ok(());
        }
    }

    return Err(format!("Bad arguments passed to {}: {}", name, args));
}

fn cons(heap: &mut Heap, args: &RootedValue) -> SchemeResult {
    try!(require_arity("cons", args, 2));
    let pair = args.to_pair(heap)
        .expect("If length = 2, it must be a pair");
    let car = pair.car(heap);
    let cdr = pair.cadr(heap).ok()
        .expect("If length = 2, then it has a cadr");
    return Ok(Value::new_pair(heap, &car, &cdr));
}

fn car(heap: &mut Heap, args: &RootedValue) -> SchemeResult {
    try!(require_arity("car", args, 1));
    let arg = args.car(heap)
        .expect("If length = 1, then there must be a car.");
    let car = try!(arg.car(heap).ok_or(
        format!("Cannot take car of non-cons: {}", arg)));
    return Ok(car);
}

fn cdr(heap: &mut Heap, args: &RootedValue) -> SchemeResult {
    try!(require_arity("cdr", args, 1));
    let arg = args.car(heap)
        .expect("If length = 1, then there must be a car.");
    let cdr = try!(arg.cdr(heap).ok_or(
        format!("Cannot take cdr of non-cons: {}", arg)));
    return Ok(cdr);
}

fn list(_: &mut Heap, args: &RootedValue) -> SchemeResult {
    return Ok(args.clone());
}

fn null_question(heap: &mut Heap, args: &RootedValue) -> SchemeResult {
    try!(require_arity("null?", args, 1));
    let arg = args.car(heap)
        .expect("If length = 1, then we must have a car");
    Ok(Rooted::new(heap, Value::new_boolean(match *arg {
        Value::EmptyList => true,
        _                => false,
    })))
}

fn pair_question(heap: &mut Heap, args: &RootedValue) -> SchemeResult {
    try!(require_arity("pair?", args, 1));
    let arg = args.car(heap)
        .expect("If length = 1, then we must have a car");
    Ok(Rooted::new(heap, Value::new_boolean(match *arg {
        Value::Pair(_) => true,
        _              => false,
    })))
}

fn atom_question(heap: &mut Heap, args: &RootedValue) -> SchemeResult {
    try!(require_arity("pair?", args, 1));
    let arg = args.car(heap)
        .expect("If length = 1, then we must have a car");
    Ok(Rooted::new(heap, Value::new_boolean(match *arg {
        Value::Pair(_) => false,
        _              => true,
    })))
}

fn eq_question(heap: &mut Heap, args: &RootedValue) -> SchemeResult {
    try!(require_arity("eq?", args, 2));
    let pair = args.to_pair(heap)
        .expect("If length = 2, it must be a pair");
    let a = pair.car(heap);
    let b = pair.cadr(heap).ok()
        .expect("If length = 2, then it has a cadr");
    Ok(Rooted::new(heap, Value::new_boolean(a == b)))
}

fn add(heap: &mut Heap, args: &RootedValue) -> SchemeResult {
    try!(require_arity("+", args, 2));
    let pair = args.to_pair(heap)
        .expect("If length = 2, it must be a pair");
    let a = try!(pair.car(heap).to_integer().ok_or(
        "Cannot use + with non-numbers".to_string()));
    let b = try!(pair.cadr(heap).ok()
                     .expect("If length = 2, then it has a cadr")
                     .to_integer().ok_or(
                         "Cannot use + with non-numbers".to_string()));
    return Ok(Rooted::new(heap, Value::new_integer(a + b)));
}

fn subtract(heap: &mut Heap, args: &RootedValue) -> SchemeResult {
    try!(require_arity("-", args, 2));
    let pair = args.to_pair(heap)
        .expect("If length = 2, it must be a pair");
    let a = try!(pair.car(heap).to_integer().ok_or(
        "Cannot use - with non-numbers".to_string()));
    let b = try!(pair.cadr(heap).ok()
                     .expect("If length = 2, then it has a cadr")
                     .to_integer().ok_or(
                         "Cannot use - with non-numbers".to_string()));
    return Ok(Rooted::new(heap, Value::new_integer(a - b)));
}

fn divide(heap: &mut Heap, args: &RootedValue) -> SchemeResult {
    try!(require_arity("/", args, 2));
    let pair = args.to_pair(heap)
        .expect("If length = 2, it must be a pair");
    let a = try!(pair.car(heap).to_integer().ok_or(
        "Cannot use + with non-numbers".to_string()));
    let b = try!(pair.cadr(heap).ok()
                     .expect("If length = 2, then it has a cadr")
                     .to_integer().ok_or(
                         "Cannot use / with non-numbers".to_string()));
    if b == 0 {
        return Err("Divide by zero".to_string());
    }
    return Ok(Rooted::new(heap, Value::new_integer(a / b)));
}

fn multiply(heap: &mut Heap, args: &RootedValue) -> SchemeResult {
    try!(require_arity("*", args, 2));
    let pair = args.to_pair(heap)
        .expect("If length = 2, it must be a pair");
    let a = try!(pair.car(heap).to_integer().ok_or(
        "Cannot use + with non-numbers".to_string()));
    let b = try!(pair.cadr(heap).ok()
                     .expect("If length = 2, then it has a cadr")
                     .to_integer().ok_or(
                         "Cannot use * with non-numbers".to_string()));
    return Ok(Rooted::new(heap, Value::new_integer(a * b)));
}

fn define_primitive(env: &mut Environment,
                    act: &mut ActivationPtr,
                    name: &'static str,
                    function: PrimitiveFunction) {
    let (i, _) = env.define(name.to_string());
    assert!(i == 0, "All primitives should be defined on the global activation");
    act.push_value(Value::new_primitive(name, function));
}

pub fn define_primitives(env: &mut Environment, act: &mut ActivationPtr) {
    define_primitive(env, act, "cons", cons);
    define_primitive(env, act, "car", car);
    define_primitive(env, act, "cdr", cdr);
    define_primitive(env, act, "list", list);

    define_primitive(env, act, "null?", null_question);
    define_primitive(env, act, "pair?", pair_question);
    define_primitive(env, act, "atom?", atom_question);
    define_primitive(env, act, "eq?", eq_question);

    define_primitive(env, act, "+", add);
    define_primitive(env, act, "-", subtract);
    define_primitive(env, act, "/", divide);
    define_primitive(env, act, "*", multiply);
}

// TESTS -----------------------------------------------------------------------

#[test]
fn test_primitives_cons() {
    use eval::evaluate_file;

    let heap = &mut Heap::new();
    let result = evaluate_file(heap, "./tests/test_primitives_cons.scm")
        .ok()
        .expect("Should be able to eval a file.");
    let pair = result.to_pair(heap)
        .expect("Result should be a pair");
    assert_eq!(*pair.car(heap), Value::new_integer(1));
    assert_eq!(*pair.cdr(heap), Value::new_integer(2));
}

#[test]
fn test_primitives_car() {
    use eval::evaluate_file;

    let heap = &mut Heap::new();
    let result = evaluate_file(heap, "./tests/test_primitives_car.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::new_integer(1));
}

#[test]
fn test_primitives_cdr() {
    use eval::evaluate_file;

    let heap = &mut Heap::new();
    let result = evaluate_file(heap, "./tests/test_primitives_cdr.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::new_integer(2));
}

#[test]
fn test_primitives_list() {
    use eval::evaluate_file;

    let heap = &mut Heap::new();
    let result = evaluate_file(heap, "./tests/test_primitives_list.scm")
        .ok()
        .expect("Should be able to eval a file.");
    let pair = result.to_pair(heap)
        .expect("Result should be a pair");
    assert_eq!(*pair.car(heap),
               Value::new_integer(1));
    assert_eq!(*pair.cadr(heap).ok().expect("pair.cadr"),
               Value::new_integer(2));
    assert_eq!(*pair.caddr(heap).ok().expect("pair.caddr"),
               Value::new_integer(3));
    assert_eq!(*pair.cdddr(heap).ok().expect("pair.cdddr"),
               Value::EmptyList);
}

#[test]
fn test_primitives_null() {
    use eval::evaluate_file;

    let heap = &mut Heap::new();
    let result = evaluate_file(heap, "./tests/test_primitives_null.scm")
        .ok()
        .expect("Should be able to eval a file.");
    let pair = result.to_pair(heap)
        .expect("Result should be a pair");
    assert_eq!(*pair.car(heap), Value::new_boolean(true));
    assert_eq!(*pair.cdr(heap), Value::new_boolean(false));
}

#[test]
fn test_primitives_arithmetic() {
    use eval::evaluate_file;

    let heap = &mut Heap::new();
    let result = evaluate_file(heap, "./tests/test_primitives_arithmetic.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::new_integer(42));
}

#[test]
fn test_primitives_pair() {
    use eval::evaluate_file;

    let heap = &mut Heap::new();
    let result = evaluate_file(heap, "./tests/test_primitives_pair.scm")
        .ok()
        .expect("Should be able to eval a file.");
    let pair = result.to_pair(heap)
        .expect("Result should be a pair");
    assert_eq!(*pair.car(heap), Value::new_boolean(true));
    assert_eq!(*pair.cdr(heap), Value::new_boolean(false));
}

#[test]
fn test_primitives_atom() {
    use eval::evaluate_file;

    let heap = &mut Heap::new();
    let result = evaluate_file(heap, "./tests/test_primitives_atom.scm")
        .ok()
        .expect("Should be able to eval a file.");
    let pair = result.to_pair(heap)
        .expect("Result should be a pair");
    assert_eq!(*pair.car(heap), Value::new_boolean(true));
    assert_eq!(*pair.cdr(heap), Value::new_boolean(false));
}

#[test]
fn test_primitives_eq() {
    use eval::evaluate_file;

    let heap = &mut Heap::new();
    let result = evaluate_file(heap, "./tests/test_primitives_eq.scm")
        .ok()
        .expect("Should be able to eval a file.");
    let pair = result.to_pair(heap)
        .expect("Result should be a pair");
    assert_eq!(*pair.car(heap), Value::new_boolean(true));
    assert_eq!(*pair.cdr(heap), Value::new_boolean(false));
}
