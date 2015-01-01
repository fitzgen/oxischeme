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

//! Evaluating values.

use environment::{Environment, RootedEnvironmentPtr};
use heap::{Heap, Rooted};
use value::{RootedValue, SchemeResult, Value};

/// Return true if the value doesn't need to be evaluated because it is
/// "autoquoting" or "self evaluating", false otherwise.
fn is_auto_quoting(val: &RootedValue) -> bool {
    match **val {
        Value::EmptyList    => false,
        Value::Pair(_)      => false,
        Value::Symbol(_)    => false,
        _                   => true,
    }
}

/// Evaluate the given form in the global environment.
pub fn evaluate_in_global_env(heap: &mut Heap,
                              form: &RootedValue) -> SchemeResult {
    let mut env = heap.global_env();
    evaluate(heap, &mut env, form)
}

/// Evaluate the given form in the given environment.
pub fn evaluate(heap: &mut Heap,
                env: &mut RootedEnvironmentPtr,
                form: &RootedValue) -> SchemeResult {
    // NB: We use a loop to trampoline tail calls to `evaluate` to ensure that tail
    // calls don't take up more stack space. Instead of doing
    //
    //     return evaluate(heap, new_env, new_form);
    //
    // we do
    //
    //     env_.emplace(new_env);
    //     form_.emplace(new_form);
    //     continue;
    let mut env_ = &mut env.clone();
    let mut form_ = &mut form.clone();
    loop {
        if form_.is_atom() {
            return evaluate_atom(heap, env_, form_);
        }

        let pair = form_.to_pair(heap).expect(
            "If a value is not an atom, then it must be a pair.");

        let quote = heap.quote_symbol();
        let if_symbol = heap.if_symbol();
        let begin = heap.begin_symbol();
        let define = heap.define_symbol();
        let set_bang = heap.set_bang_symbol();
        let lambda = heap.lambda_symbol();

        match *pair.car(heap) {
            // Quoted forms.
            v if v == *quote => return evaluate_quoted(heap, form_),

            // Definitions. These are only supposed to be allowed at the top level
            // and at the beginning of a body, but we are punting on that
            // restriction for now.
            v if v == *define => return evaluate_definition(heap, env_, form_),

            // `set!` assignment.
            v if v == *set_bang => return evaluate_set(heap, env_, form_),

            // Lambda forms.
            v if v == *lambda => return evaluate_lambda(heap, env_, form_),

            // If expressions.
            v if v == *if_symbol => {
                let length = try!(form_.len().ok().ok_or(
                    "Improperly formed if expression".to_string()));
                if length != 4 {
                    return Err("Improperly formed if expression".to_string());
                }

                let condition_form = try!(pair.cadr(heap));
                let condition_val = try!(evaluate(heap, env_, &condition_form));

                form_.emplace(*try!(if *condition_val == Value::new_boolean(false) {
                    // Alternative.
                    pair.cadddr(heap)
                } else {
                    // Consequent.
                    pair.caddr(heap)
                }));
                continue;
            },

            // `(begin ...)` sequences.
            v if v == *begin => {
                let forms = pair.cdr(heap);
                form_.emplace(*try!(evaluate_sequence(heap, env_, &forms)));
                continue;
            },

            // Procedure invocations.
            procedure        => {
                // Ensure that the form is a proper list.
                try!(form_.len().ok().ok_or("Bad invocation form".to_string()));

                let proc_form = Rooted::new(heap, procedure);
                let proc_val = try!(evaluate(heap, env_, &proc_form));

                let args_form = pair.cdr(heap);
                let args_val = try!(evaluate_list(heap, env_, &args_form));

                match *proc_val {
                    Value::Primitive(primitive) => {
                        return primitive.call(heap, &args_val);
                    },

                    Value::Procedure(proc_ptr) => {
                        let proc_env = proc_ptr.get_env(heap);
                        let proc_params = proc_ptr.get_params(heap);
                        env_.emplace(*try!(Environment::extend(
                            heap,
                            &proc_env,
                            &proc_params,
                            &args_val)));

                        let proc_body = proc_ptr.get_body(heap);
                        form_.emplace(*try!(evaluate_sequence(heap,
                                                              env_,
                                                              &proc_body)));
                        continue;
                    },

                    _                           => {
                        return Err(format!("Expected a procedure, found {}",
                                           *proc_val));
                    }
                }
            },
        };
    }
}

/// Evaluate a `lambda` form.
fn evaluate_lambda(heap: &mut Heap,
                   env: &RootedEnvironmentPtr,
                   form: &RootedValue) -> SchemeResult {
    let length = try!(form.len().ok().ok_or("Bad lambda form".to_string()));
    if length < 3 {
        return Err("Lambda is missing body".to_string());
    }

    let pair = form.to_pair(heap).unwrap();
    let params = pair.cadr(heap).ok().expect("Must be here since length >= 3");
    let body = pair.cddr(heap).ok().expect("Must be here since length >= 3");
    return Ok(Value::new_procedure(heap, &params, &body, env));
}

/// Evaluate a `set!` form.
fn evaluate_set(heap: &mut Heap,
                env: &mut RootedEnvironmentPtr,
                form: &RootedValue) -> SchemeResult {
    let mut env_ = env;
    if let Ok(3) = form.len() {
        let pair = form.to_pair(heap).unwrap();
        let sym = try!(pair.cadr(heap));

        if let Some(str) = sym.to_symbol(heap) {
            let new_value_form = try!(pair.caddr(heap));
            let new_value = try!(evaluate(heap, env_, &new_value_form));
            try!(env_.update((**str).clone(), &new_value));
            return Ok(heap.unspecified_symbol());
        }

        return Err("Can only set! symbols".to_string());
    }

    return Err("Improperly formed set! expression".to_string());
}

/// Evaluate a `define` form.
fn evaluate_definition(heap: &mut Heap,
                       env: &mut RootedEnvironmentPtr,
                       form: &RootedValue) -> SchemeResult {
    let mut env_ = env;
    if let Ok(3) = form.len() {
        let pair = form.to_pair(heap).unwrap();
        let sym = try!(pair.cadr(heap));

        if let Some(str) = sym.to_symbol(heap) {
            let def_value_form = try!(pair.caddr(heap));
            let def_value = try!(evaluate(heap, env_, &def_value_form));
            env_.define((**str).clone(), &def_value);
            return Ok(heap.unspecified_symbol());
        }

        return Err("Can only define symbols".to_string());
    }

    return Err("Improperly formed definition".to_string());
}

/// Evaluate a quoted form.
fn evaluate_quoted(heap: &mut Heap, form: &RootedValue) -> SchemeResult {
    if let Ok(2) = form.len() {
        return Ok(form.cdr(heap).unwrap()
                      .car(heap).unwrap());
    }

    return Err("Wrong number of parts in quoted form".to_string());
}

/// Evaluate an atom (ie anything that is not a list).
fn evaluate_atom(heap: &mut Heap,
                 env: &mut RootedEnvironmentPtr,
                 form: &RootedValue) -> SchemeResult {
    if is_auto_quoting(form) {
        return Ok(form.clone());
    }

    if let Value::Symbol(sym) = **form {
        return env.lookup(heap, sym.deref());
    }

    return Err(format!("Cannot evaluate: {}", **form));
}

/// Evaluate each given form, returning the resulting list of values.
fn evaluate_list(heap: &mut Heap,
                 env: &mut RootedEnvironmentPtr,
                 forms: &RootedValue) -> SchemeResult {
    match **forms {
        Value::EmptyList      => Ok(Rooted::new(heap, Value::EmptyList)),
        Value::Pair(ref cons) => {
            let car = cons.car(heap);
            let val = try!(evaluate(heap, env, &car));

            let cdr = cons.cdr(heap);
            let rest = try!(evaluate_list(heap, env, &cdr));

            Ok(Value::new_pair(heap, &val, &rest))
        },
        _                 => Err("Improper list".to_string()),
    }
}

/// Evaluate each expression in the given cons list `exprs` except for the last
/// expression, whose form is returned (so it can be trampolined to maintain
/// TCO).
fn evaluate_sequence(heap: &mut Heap,
                     env: &mut RootedEnvironmentPtr,
                     exprs: &RootedValue) -> SchemeResult {
    let mut e = exprs.clone();
    loop {
        let ee = *e;
        match ee {
            Value::Pair(ref pair) => {
                if *pair.cdr(heap) == Value::EmptyList {
                    return Ok(pair.car(heap));
                } else {
                    let car = pair.car(heap);
                    try!(evaluate(heap, env, &car));
                    e.emplace(*pair.cdr(heap));
                }
            },
            _                 => {
                return Err("Bad sequence of expressions".to_string());
            },
        }
    }
}

/// Evaluate the file at the given path and return the value of the last form.
pub fn evaluate_file(heap: &mut Heap, file_path: &str) -> SchemeResult {
    use read::read_from_file;

    let mut reader = try!(read_from_file(file_path, heap)
                              .ok()
                              .ok_or("Failed to read from file".to_string()));

    let mut result = Rooted::new(heap, Value::EmptyList);

    for form in reader {
        result.emplace(*try!(evaluate_in_global_env(heap, &form)));
    }

    if let Err(ref msg) = *reader.get_result() {
        return Err(msg.clone());
    }

    return Ok(result);
}

#[test]
fn test_eval_integer() {
    let mut heap = Heap::new();
    let result = evaluate_file(&mut heap, "./tests/test_eval_integer.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::new_integer(42));
}

#[test]
fn test_eval_boolean() {
    let mut heap = Heap::new();
    let result = evaluate_file(&mut heap, "./tests/test_eval_boolean.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::new_boolean(true));
}

#[test]
fn test_eval_quoted() {
    let mut heap = Heap::new();
    let result = evaluate_file(&mut heap, "./tests/test_eval_quoted.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::EmptyList);
}

#[test]
fn test_eval_if_consequent() {
    let mut heap = Heap::new();
    let result = evaluate_file(&mut heap, "./tests/test_eval_if_consequent.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::new_integer(1));
}

#[test]
fn test_eval_if_alternative() {
    let mut heap = Heap::new();
    let result = evaluate_file(&mut heap, "./tests/test_eval_if_alternative.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::new_integer(2));
}

#[test]
fn test_eval_begin() {
    let mut heap = Heap::new();
    let result = evaluate_file(&mut heap, "./tests/test_eval_begin.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::new_integer(2));
}

#[test]
fn test_eval_variables() {
    use value::list;

    let heap = &mut Heap::new();

    let define_symbol = heap.define_symbol();
    let set_bang_symbol = heap.set_bang_symbol();
    let foo_symbol = heap.get_or_create_symbol("foo".to_string());

    let mut def_items = [
        define_symbol,
        foo_symbol,
        Rooted::new(heap, Value::new_integer(2))
    ];
    let def_form = list(heap, &mut def_items);
    evaluate_in_global_env(heap, &def_form).ok()
        .expect("Should be able to define");

    let foo_symbol_ = heap.get_or_create_symbol("foo".to_string());

    let def_val = evaluate_in_global_env(heap, &foo_symbol_).ok()
        .expect("Should be able to get a defined symbol's value");
    assert_eq!(*def_val, Value::new_integer(2));

    let mut set_items = [
        set_bang_symbol,
        foo_symbol_,
        Rooted::new(heap, Value::new_integer(1))
    ];
    let set_form = list(heap, &mut set_items);
    evaluate_in_global_env(heap, &set_form).ok()
        .expect("Should be able to define");

    let foo_symbol__ = heap.get_or_create_symbol("foo".to_string());

    let set_val = evaluate_in_global_env(heap, &foo_symbol__).ok()
        .expect("Should be able to get a defined symbol's value");
    assert_eq!(*set_val, Value::new_integer(1));
}

#[test]
fn test_eval_and_call_lambda() {
    let mut heap = Heap::new();
    let result = evaluate_file(&mut heap, "./tests/test_eval_and_call_lambda.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::new_integer(5));
}

#[test]
fn test_eval_closures() {
    let mut heap = Heap::new();
    let result = evaluate_file(&mut heap, "./tests/test_eval_closures.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::new_integer(1));
}