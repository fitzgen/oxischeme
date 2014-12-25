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

use std::fmt::{format};
use context::{Context};
use environment::{Environment};
use heap::{EnvironmentPtr, ProcedurePtr};
use value::{SchemeResult, Value};

fn is_auto_quoting(val: &Value) -> bool {
    match *val {
        Value::EmptyList    => false,
        Value::Pair(_)      => false,
        Value::Symbol(_)    => false,
        _                   => true,
    }
}

/// Evaluate the given form in the global environment.
pub fn evaluate_in_global_env(ctx: &mut Context, form: Value) -> SchemeResult {
    let mut env = ctx.global_env();
    evaluate(ctx, &mut env, form)
}

/// Evaluate the given form in the given environment.
pub fn evaluate(ctx: &mut Context,
                env: &mut EnvironmentPtr,
                form: Value) -> SchemeResult {
    if form.is_atom() {
        if is_auto_quoting(&form) {
            return Ok(form);
        }

        if let Value::Symbol(sym) = form {
            return env.lookup(sym.deref());
        }

        return Err(format_args!(format, "Cannot evaluate: {}", form));
    }

    let pair = form.to_pair()
        .expect("If a value is not an atom, then it must be a pair.");

    let quote = ctx.quote_symbol();
    let if_symbol = ctx.if_symbol();
    let begin = ctx.begin_symbol();
    let define = ctx.define_symbol();
    let set_bang = ctx.set_bang_symbol();
    let lambda = ctx.lambda_symbol();

    match pair.car() {
        // Quoted forms.
        v if v == quote => {
            if let Some(Value::EmptyList) = form.cdr().unwrap().cdr() {
                return Ok(form.cdr().unwrap().car().unwrap());
            }

            return Err("Wrong number of parts in quoted form".to_string());
        },

        // If expressions.
        v if v == if_symbol => {
            if let Ok(4) = form.len() {
                let condition_form = try!(pair.cadr());
                let condition_val = try!(evaluate(ctx, env, condition_form));

                if condition_val == Value::new_boolean(false) {
                    let alternative_form = try!(pair.cadddr());
                    return evaluate(ctx, env, alternative_form);
                }

                let consequent_form = try!(pair.caddr());
                return evaluate(ctx, env, consequent_form);
            }

            return Err("Improperly formed if expression".to_string());
        },

        // `(begin ...)` sequences.
        v if v == begin => {
            return evaluate_sequence(ctx, env, pair.cdr());
        },

        // Definitions. These are only supposed to be allowed at the top level
        // and at the beginning of a body, but we are punting on that
        // restriction for now.
        v if v == define => {
            if let Ok(3) = form.len() {
                let sym = try!(pair.cadr());

                if let Some(str) = sym.to_symbol() {
                    let def_value_form = try!(pair.caddr());
                    let def_value = try!(evaluate(ctx, env, def_value_form));
                    env.define(str.deref().clone(), def_value);
                    return Ok(ctx.unspecified_symbol());
                }

                return Err("Can only define symbols".to_string());
            }

            return Err("Improperly formed definition".to_string());
        },

        // `set!` assignment.
        v if v == set_bang => {
            if let Ok(3) = form.len() {
                let sym = try!(pair.cadr());

                if let Some(str) = sym.to_symbol() {
                    let new_value_form = try!(pair.caddr());
                    let new_value = try!(evaluate(ctx, env, new_value_form));
                    try!(env.update(str.deref().clone(), new_value));
                    return Ok(ctx.unspecified_symbol());
                }

                return Err("Can only set! symbols".to_string());
            }

            return Err("Improperly formed set! expression".to_string());
        },

        // Lambda forms.
        v if v == lambda => {
            let length = try!(form.len().ok().ok_or("Bad lambda form".to_string()));
            if length < 3 {
                return Err("Lambda is missing body".to_string());
            }

            let params = pair.cadr().ok().expect("Must be here since length >= 3");
            let body = pair.cddr().ok().expect("Must be here since length >= 3");
            return Ok(Value::new_procedure(ctx.heap(), params, body, *env));
        },

        // Invocations
        procedure        => {
            let length = try!(form.len().ok().ok_or(
                "Bad invocation form".to_string()));
            assert!(
                length >= 1,
                "We know length is at least 1 because we're matching on the car.");
            let proc_val = try!(evaluate(ctx, env, procedure));
            let proc_ptr = try!(proc_val.to_procedure().ok_or(
                format_args!(format, "Expected a procedure, found {}", proc_val)));
            let args = try!(evaluate_list(ctx, env, pair.cdr()));
            return invoke(ctx, proc_ptr, args);
        },
    };
}

/// TODO FITZGEN
fn invoke(ctx: &mut Context, procedure: ProcedurePtr, args: Value) -> SchemeResult {
    let mut env = try!(Environment::extend(ctx.heap(),
                                           procedure.get_env(),
                                           procedure.get_params(),
                                           args));
    evaluate_sequence(ctx, &mut env, procedure.get_body())
}

/// TODO FITZGEN
fn evaluate_list(ctx: &mut Context,
                 env: &mut EnvironmentPtr,
                 values: Value) -> SchemeResult {
    match values {
        Value::EmptyList  => Ok(Value::EmptyList),
        Value::Pair(cons) => {
            let val = try!(evaluate(ctx, env, cons.car()));
            let rest = try!(evaluate_list(ctx, env, cons.cdr()));
            Ok(Value::new_pair(ctx.heap(), val, rest))
        },
        _                 => Err("Improper list".to_string()),
    }
}

/// Evaluate each expression in the given cons list `exprs` and return the value
/// of the last expression.
fn evaluate_sequence(ctx: &mut Context,
                     env: &mut EnvironmentPtr,
                     exprs: Value) -> SchemeResult {
    let mut e = exprs;
    loop {
        match e {
            Value::EmptyList  => return Ok(Value::EmptyList),
            Value::Pair(pair) => {
                let v = try!(evaluate(ctx, env, pair.car()));
                if pair.cdr() == Value::EmptyList {
                    return Ok(v);
                } else {
                    e = pair.cdr();
                }
            },
            _                 => {
                return Err("Bad sequence of expressions".to_string());
            },
        }
    }
}

#[test]
fn test_eval_integer() {
    let mut ctx = Context::new();
    assert_eq!(evaluate_in_global_env(&mut ctx, Value::new_integer(42)),
               Ok(Value::new_integer(42)));
}

#[test]
fn test_eval_boolean() {
    let mut ctx = Context::new();
    assert_eq!(evaluate_in_global_env(&mut ctx, Value::new_boolean(true)),
               Ok(Value::new_boolean(true)));
}

#[test]
fn test_eval_quoted() {
    use value::list;

    let mut ctx = Context::new();
    let val = Value::new_integer(5);
    let mut items = [
        ctx.quote_symbol(),
        val
    ];
    let quoted = list(&mut ctx, &mut items);
    assert_eq!(evaluate_in_global_env(&mut ctx, quoted),
               Ok(val));
}

#[test]
fn test_eval_if_consequent() {
    use value::list;

    let mut ctx = Context::new();
    let mut items = [
        ctx.if_symbol(),
        Value::new_boolean(true),
        Value::new_integer(1),
        Value::new_integer(2)
    ];
    let if_form = list(&mut ctx, &mut items);
    assert_eq!(evaluate_in_global_env(&mut ctx, if_form),
               Ok(Value::new_integer(1)));
}

#[test]
fn test_eval_if_alternative() {
    use value::list;

    let mut ctx = Context::new();
    let mut items = [
        ctx.if_symbol(),
        Value::new_boolean(false),
        Value::new_integer(1),
        Value::new_integer(2)
    ];
    let if_form = list(&mut ctx, &mut items);
    assert_eq!(evaluate_in_global_env(&mut ctx, if_form),
               Ok(Value::new_integer(2)));
}

#[test]
fn test_eval_begin() {
    use value::list;

    let mut ctx = Context::new();
    let mut items = [
        ctx.begin_symbol(),
        Value::new_integer(1),
        Value::new_integer(2)
    ];
    let begin_form = list(&mut ctx, &mut items);
    assert_eq!(evaluate_in_global_env(&mut ctx, begin_form),
               Ok(Value::new_integer(2)));
}

#[test]
fn test_eval_variables() {
    use value::list;

    let mut ctx = Context::new();

    let define_symbol = ctx.define_symbol();
    let set_bang_symbol = ctx.set_bang_symbol();
    let foo_symbol = ctx.get_or_create_symbol("foo".to_string());

    let mut def_items = [
        define_symbol,
        foo_symbol,
        Value::new_integer(2)
    ];
    let def_form = list(&mut ctx, &mut def_items);
    evaluate_in_global_env(&mut ctx, def_form).ok()
        .expect("Should be able to define");

    let def_val = evaluate_in_global_env(&mut ctx, foo_symbol).ok()
        .expect("Should be able to get a defined symbol's value");
    assert_eq!(def_val, Value::new_integer(2));

    let mut set_items = [
        set_bang_symbol,
        foo_symbol,
        Value::new_integer(1)
    ];
    let set_form = list(&mut ctx, &mut set_items);
    evaluate_in_global_env(&mut ctx, set_form).ok()
        .expect("Should be able to define");

    let set_val = evaluate_in_global_env(&mut ctx, foo_symbol).ok()
        .expect("Should be able to get a defined symbol's value");
    assert_eq!(set_val, Value::new_integer(1));
}

#[test]
fn test_eval_lambda() {
    assert!(false, "TODO FITZGEN");
}