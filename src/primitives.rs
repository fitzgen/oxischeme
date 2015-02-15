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
use eval::{apply_invocation};
use heap::{Heap, Rooted};
use value::{RootedValue, SchemeResult, Value};

/// The function signature for primitives.
pub type PrimitiveFunction = fn(&mut Heap, Vec<RootedValue>) -> SchemeResult;

fn cons(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    if let [ref car, ref cdr] = args.as_slice() {
        Ok(Value::new_pair(heap, car, cdr))
    } else {
        Err("Bad arguments".to_string())
    }
}

fn car(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    if let [ref arg] = args.as_slice() {
        arg.car(heap).ok_or(
            format!("Cannot take car of non-cons: {}", **arg))
    } else {
        Err("Bad arguments".to_string())
    }
}

fn set_car_bang(heap: &mut Heap, mut args: Vec<RootedValue>) -> SchemeResult {
    if let [ref mut cons, ref val] = args.as_mut_slice() {
        if let &mut Value::Pair(ref mut cons) = &mut **cons {
            cons.set_car(val);
            return Ok(heap.unspecified_symbol());
        }
        return Err(format!("Can't set-car! on non-cons: {}", **cons));
    } else {
        Err("Bad arguments".to_string())
    }
}

fn cdr(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    if let [ref arg] = args.as_slice() {
        arg.cdr(heap).ok_or(
            format!("Cannot take cdr of non-cons: {}", **arg))
    } else {
        Err("Bad arguments".to_string())
    }
}

fn set_cdr_bang(heap: &mut Heap, mut args: Vec<RootedValue>) -> SchemeResult {
    if let [ref mut cons, ref val] = args.as_mut_slice() {
        if let &mut Value::Pair(ref mut cons) = &mut **cons {
            cons.set_cdr(val);
            return Ok(heap.unspecified_symbol());
        }
        return Err(format!("Can't set-cdr! on non-cons: {}", **cons));
    } else {
        Err("Bad arguments".to_string())
    }
}

fn list(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    use value;
    return Ok(value::list(heap, args.as_slice()));
}

fn length(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    if let [ref arg] = args.as_slice() {
        let len = try!(arg.len().ok().ok_or(
            format!("Can only take length of proper lists, got {}", **arg)));
        Ok(Rooted::new(heap, Value::new_integer(len as i64)))
    } else {
        Err("Bad arguments".to_string())
    }
}

fn apply(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    // Note: we don't support concatenating many argument lists yet:
    //
    //     (apply f '(1 2) '(3 4)) == (apply f '(1 2 3 4))
    //
    // We should suport that eventually.
    if let [ref proc_val, ref args] = args.as_slice() {
        let v : Vec<RootedValue> = try!(args.iter()
            .map(|result_val| {
                result_val
                    .map(|r| Rooted::new(heap, r))
                    .map_err(|_| "Must pass a proper list to `apply`".to_string())
            })
            .collect());
        let thunk = try!(apply_invocation(heap, proc_val, v));
        thunk.run(heap)
    } else {
        Err("Bad arguments".to_string())
    }
}

fn error(_: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    let mut string = String::from_str("ERROR!");
    for val in args.iter() {
        string.push_str(format!("\n\t{}", **val).as_slice());
    }
    Err(string)
}

fn print(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    for val in args.iter() {
        println!("{}", **val);
    }
    Ok(heap.unspecified_symbol())
}

fn not(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    if let [ref arg] = args.as_slice() {
        Ok(Rooted::new(heap, Value::new_boolean(match **arg {
            Value::Boolean(b) if b == false => true,
            _                               => false,
        })))
    } else {
        Err("Bad arguments".to_string())
    }
}

fn null_question(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    if let [ref arg] = args.as_slice() {
        Ok(Rooted::new(heap, Value::new_boolean(**arg == Value::EmptyList)))
    } else {
        Err("Bad arguments".to_string())
    }
}

fn pair_question(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    if let [ref arg] = args.as_slice() {
        Ok(Rooted::new(heap, Value::new_boolean(match **arg {
            Value::Pair(_) => true,
            _              => false,
        })))
    } else {
        Err("Bad arguments".to_string())
    }
}

fn atom_question(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    if let [ref arg] = args.as_slice() {
        Ok(Rooted::new(heap, Value::new_boolean(match **arg {
            Value::Pair(_) => false,
            _              => true,
        })))
    } else {
        Err("Bad arguments".to_string())
    }
}

fn eq_question(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    if let [ref a, ref b] = args.as_slice() {
        Ok(Rooted::new(heap, Value::new_boolean(*a == *b)))
    } else {
        Err("Bad arguments".to_string())
    }
}

fn symbol_question(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    if let [ref arg] = args.as_slice() {
        Ok(Rooted::new(heap, Value::new_boolean(match **arg {
            Value::Symbol(_) => true,
            _                => false
        })))
    } else {
        Err("Bad arguments".to_string())
    }
}

fn number_equal(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    if let [ref a, ref b] = args.as_slice() {
        let first = try!(a.to_integer().ok_or(
            "Cannot use = with non-numbers".to_string()));
        let second = try!(b.to_integer().ok_or(
            "Cannot use = with non-numbers".to_string()));
        Ok(Rooted::new(heap, Value::new_boolean(first == second)))
    } else {
        Err("Bad arguments".to_string())
    }
}

fn gt(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    if let [ref a, ref b] = args.as_slice() {
        let first = try!(a.to_integer().ok_or(
            "Cannot use > with non-numbers".to_string()));
        let second = try!(b.to_integer().ok_or(
            "Cannot use > with non-numbers".to_string()));
        Ok(Rooted::new(heap, Value::new_boolean(first > second)))
    } else {
        Err("Bad arguments".to_string())
    }
}

fn lt(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    if let [ref a, ref b] = args.as_slice() {
        let first = try!(a.to_integer().ok_or(
            "Cannot use < with non-numbers".to_string()));
        let second = try!(b.to_integer().ok_or(
            "Cannot use < with non-numbers".to_string()));
        Ok(Rooted::new(heap, Value::new_boolean(first < second)))
    } else {
        Err("Bad arguments".to_string())
    }
}

fn add(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    if let [ref a, ref b] = args.as_slice() {
        let first = try!(a.to_integer().ok_or(
            "Cannot use + with non-numbers".to_string()));
        let second = try!(b.to_integer().ok_or(
            "Cannot use + with non-numbers".to_string()));
        Ok(Rooted::new(heap, Value::new_integer(first + second)))
    } else {
        Err("Bad arguments".to_string())
    }
}

fn subtract(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    if let [ref a, ref b] = args.as_slice() {
        let first = try!(a.to_integer().ok_or(
            "Cannot use - with non-numbers".to_string()));
        let second = try!(b.to_integer().ok_or(
            "Cannot use - with non-numbers".to_string()));
        Ok(Rooted::new(heap, Value::new_integer(first - second)))
    } else {
        Err("Bad arguments".to_string())
    }
}

fn divide(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    if let [ref a, ref b] = args.as_slice() {
        let first = try!(a.to_integer().ok_or(
            "Cannot use / with non-numbers".to_string()));
        let second = try!(b.to_integer().ok_or(
            "Cannot use / with non-numbers".to_string()));
        if second == 0 {
            return Err("Divide by zero".to_string());
        }
        Ok(Rooted::new(heap, Value::new_integer(first / second)))
    } else {
        Err("Bad arguments".to_string())
    }
}

fn multiply(heap: &mut Heap, args: Vec<RootedValue>) -> SchemeResult {
    if let [ref a, ref b] = args.as_slice() {
        let first = try!(a.to_integer().ok_or(
            "Cannot use * with non-numbers".to_string()));
        let second = try!(b.to_integer().ok_or(
            "Cannot use * with non-numbers".to_string()));
        Ok(Rooted::new(heap, Value::new_integer(first * second)))
    } else {
        Err("Bad arguments".to_string())
    }
}

fn define_primitive(env: &mut Environment,
                    act: &mut ActivationPtr,
                    name: &'static str,
                    function: PrimitiveFunction) {
    let (i, j) = env.define(name.to_string());
    assert!(i == 0, "All primitives should be defined on the global activation");
    act.define(j, Value::new_primitive(name, function));
}

pub fn define_primitives(env: &mut Environment, act: &mut ActivationPtr) {
    define_primitive(env, act, "cons", cons);
    define_primitive(env, act, "car", car);
    define_primitive(env, act, "set-car!", set_car_bang);
    define_primitive(env, act, "cdr", cdr);
    define_primitive(env, act, "set-cdr!", set_cdr_bang);

    define_primitive(env, act, "list", list);
    define_primitive(env, act, "length", length);

    define_primitive(env, act, "apply", apply);

    define_primitive(env, act, "error", error);
    define_primitive(env, act, "print", print);

    define_primitive(env, act, "not", not);
    define_primitive(env, act, "null?", null_question);
    define_primitive(env, act, "pair?", pair_question);
    define_primitive(env, act, "atom?", atom_question);
    define_primitive(env, act, "eq?", eq_question);
    define_primitive(env, act, "symbol?", symbol_question);

    define_primitive(env, act, "=", number_equal);
    define_primitive(env, act, ">", gt);
    define_primitive(env, act, "<", lt);

    define_primitive(env, act, "+", add);
    define_primitive(env, act, "-", subtract);
    define_primitive(env, act, "/", divide);
    define_primitive(env, act, "*", multiply);
}

// TESTS -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use eval::{evaluate_file};
    use heap::{Heap};
    use value::{Value};

    #[test]
    fn test_primitives_cons() {
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
        let heap = &mut Heap::new();
        let result = evaluate_file(heap, "./tests/test_primitives_car.scm")
            .ok()
            .expect("Should be able to eval a file.");
        assert_eq!(*result, Value::new_integer(1));
    }

    #[test]
    fn test_primitives_set_car() {
        let heap = &mut Heap::new();
        let result = evaluate_file(heap, "./tests/test_primitives_set_car.scm")
            .ok()
            .expect("Should be able to eval a file.");
        let pair = result.to_pair(heap)
            .expect("Result should be a pair");
        assert_eq!(*pair.car(heap), Value::new_integer(1));
        assert_eq!(*pair.cdr(heap), Value::new_integer(2));
    }

    #[test]
    fn test_primitives_cdr() {
        let heap = &mut Heap::new();
        let result = evaluate_file(heap, "./tests/test_primitives_cdr.scm")
            .ok()
            .expect("Should be able to eval a file.");
        assert_eq!(*result, Value::new_integer(2));
    }

    #[test]
    fn test_primitives_set_cdr() {
        let heap = &mut Heap::new();
        let result = evaluate_file(heap, "./tests/test_primitives_set_cdr.scm")
            .ok()
            .expect("Should be able to eval a file.");
        let pair = result.to_pair(heap)
            .expect("Result should be a pair");
        assert_eq!(*pair.car(heap), Value::new_integer(1));
        assert_eq!(*pair.cdr(heap), Value::new_integer(2));
    }

    #[test]
    fn test_primitives_list() {
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
    fn test_primitives_length() {
        let heap = &mut Heap::new();
        let result = evaluate_file(heap, "./tests/test_primitives_length.scm")
            .ok()
            .expect("Should be able to eval a file.");
        assert_eq!(*result, Value::new_integer(3));
    }

    #[test]
    fn test_primitives_apply() {
        let heap = &mut Heap::new();
        let result = evaluate_file(heap, "./tests/test_primitives_apply.scm")
            .ok()
            .expect("Should be able to eval a file.");
        assert_eq!(*result, Value::new_integer(3));
    }

    #[test]
    fn test_primitives_error() {
        let heap = &mut Heap::new();
        let error = evaluate_file(heap, "./tests/test_primitives_error.scm")
            .err()
            .expect("Should get an error evaluating this file.");
        assert_eq!(error, "ERROR!\n\t\"got an error:\"\n\t(1 2)");
    }

    #[test]
    fn test_primitives_not() {
        let heap = &mut Heap::new();
        let result = evaluate_file(heap, "./tests/test_primitives_not.scm")
            .ok()
            .expect("Should be able to eval a file.");
        let pair = result.to_pair(heap)
            .expect("Result should be a pair");
        assert_eq!(*pair.car(heap), Value::new_boolean(true));
        assert_eq!(*pair.cdr(heap), Value::new_boolean(false));
    }

    #[test]
    fn test_primitives_null() {
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
        let heap = &mut Heap::new();
        let result = evaluate_file(heap, "./tests/test_primitives_arithmetic.scm")
            .ok()
            .expect("Should be able to eval a file.");
        assert_eq!(*result, Value::new_integer(42));
    }

    #[test]
    fn test_primitives_pair() {
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
        let heap = &mut Heap::new();
        let result = evaluate_file(heap, "./tests/test_primitives_eq.scm")
            .ok()
            .expect("Should be able to eval a file.");
        let pair = result.to_pair(heap)
            .expect("Result should be a pair");
        assert_eq!(*pair.car(heap), Value::new_boolean(true));
        assert_eq!(*pair.cdr(heap), Value::new_boolean(false));
    }

    #[test]
    fn test_primitives_symbol_question() {
        let heap = &mut Heap::new();
        let result = evaluate_file(heap, "./tests/test_primitives_symbol_question.scm")
            .ok()
            .expect("Should be able to eval a file.");
        let pair = result.to_pair(heap)
            .expect("Result should be a pair");
        assert_eq!(*pair.car(heap), Value::new_boolean(true));
        assert_eq!(*pair.cdr(heap), Value::new_boolean(false));
    }

    #[test]
    fn test_primitives_number_equal() {
        let heap = &mut Heap::new();
        let result = evaluate_file(heap, "./tests/test_primitives_number_equal.scm")
            .ok()
            .expect("Should be able to eval a file.");
        let pair = result.to_pair(heap)
            .expect("Result should be a pair");
        assert_eq!(*pair.car(heap), Value::new_boolean(true));
        assert_eq!(*pair.cdr(heap), Value::new_boolean(false));
    }

    #[test]
    fn test_primitives_gt() {
        let heap = &mut Heap::new();
        let result = evaluate_file(heap, "./tests/test_primitives_gt.scm")
            .ok()
            .expect("Should be able to eval a file.");
        let pair = result.to_pair(heap)
            .expect("Result should be a pair");
        assert_eq!(*pair.car(heap), Value::new_boolean(true));
        assert_eq!(*pair.cdr(heap), Value::new_boolean(false));
    }

    #[test]
    fn test_primitives_lt() {
        let heap = &mut Heap::new();
        let result = evaluate_file(heap, "./tests/test_primitives_lt.scm")
            .ok()
            .expect("Should be able to eval a file.");
        let pair = result.to_pair(heap)
            .expect("Result should be a pair");
        assert_eq!(*pair.car(heap), Value::new_boolean(true));
        assert_eq!(*pair.cdr(heap), Value::new_boolean(false));
    }
}
