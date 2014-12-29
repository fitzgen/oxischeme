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
use environment::{Environment, RootedEnvironmentPtr};
use heap::{Rooted};
use value::{SchemeResult, RootedValue, Value};

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
pub fn evaluate_in_global_env(ctx: &mut Context,
                              form: &RootedValue) -> SchemeResult {
    let mut env = ctx.global_env();
    evaluate(ctx, &mut env, form)
}

/// Evaluate the given form in the given environment.
pub fn evaluate(ctx: &mut Context,
                env: &mut RootedEnvironmentPtr,
                form: &RootedValue) -> SchemeResult {
    // NB: We use a loop to trampoline tail calls to `evaluate` to ensure that tail
    // calls don't take up more stack space. Instead of doing
    //
    //     return evaluate(ctx, new_env, new_form);
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
            return evaluate_atom(ctx, env_, form_);
        }

        let pair = Rooted::new(
            ctx.heap(),
            form_.to_pair().expect(
                "If a value is not an atom, then it must be a pair."));

        let quote = ctx.quote_symbol();
        let if_symbol = ctx.if_symbol();
        let begin = ctx.begin_symbol();
        let define = ctx.define_symbol();
        let set_bang = ctx.set_bang_symbol();
        let lambda = ctx.lambda_symbol();

        match pair.car() {
            // Quoted forms.
            v if v == *quote => return evaluate_quoted(ctx, form_),

            // Definitions. These are only supposed to be allowed at the top level
            // and at the beginning of a body, but we are punting on that
            // restriction for now.
            v if v == *define => return evaluate_definition(ctx, env_, form_),

            // `set!` assignment.
            v if v == *set_bang => return evaluate_set(ctx, env_, form_),

            // Lambda forms.
            v if v == *lambda => return evaluate_lambda(ctx, env_, form_),

            // If expressions.
            v if v == *if_symbol => {
                let length = try!(form_.len().ok().ok_or(
                    "Improperly formed if expression".to_string()));
                if length != 4 {
                    return Err("Improperly formed if expression".to_string());
                }

                let condition_form = try!(pair.cadr(ctx));
                let condition_val = try!(evaluate(ctx, env_, &condition_form));

                form_.emplace(*try!(if *condition_val == Value::new_boolean(false) {
                    // Alternative.
                    pair.cadddr(ctx)
                } else {
                    // Consequent.
                    pair.caddr(ctx)
                }));
                continue;
            },

            // `(begin ...)` sequences.
            v if v == *begin => {
                let forms = Rooted::new(ctx.heap(), pair.cdr());
                form_.emplace(*try!(evaluate_sequence(ctx, env_, &forms)));
                continue;
            },

            // Procedure invocations.
            procedure        => {
                // Ensure that the form is a proper list.
                try!(form_.len().ok().ok_or("Bad invocation form".to_string()));

                let proc_form = Rooted::new(ctx.heap(), procedure);
                let proc_val = try!(evaluate(ctx, env_, &proc_form));
                let proc_ptr = try!(proc_val.to_procedure().ok_or(
                    format_args!(format,
                                 "Expected a procedure, found {}",
                                 *proc_val)));

                let args_form = Rooted::new(ctx.heap(), pair.cdr());
                let args_val = try!(evaluate_list(ctx, env_, &args_form));

                let proc_env = try!(Environment::extend(
                    ctx.heap(),
                    &Rooted::new(ctx.heap(), proc_ptr.get_env()),
                    &Rooted::new(ctx.heap(), proc_ptr.get_params()),
                    &args_val));

                let body = Rooted::new(ctx.heap(), proc_ptr.get_body());

                env_.emplace(*proc_env);
                form_.emplace(*try!(evaluate_sequence(ctx, env_, &body)));
                continue;
            },
        };
    }
}

/// Evaluate a `lambda` form.
fn evaluate_lambda(ctx: &mut Context,
                   env: &RootedEnvironmentPtr,
                   form: &RootedValue) -> SchemeResult {
    let length = try!(form.len().ok().ok_or("Bad lambda form".to_string()));
    if length < 3 {
        return Err("Lambda is missing body".to_string());
    }

    let pair = form.to_pair().unwrap();
    let params = pair.cadr(ctx).ok().expect("Must be here since length >= 3");
    let body = pair.cddr(ctx).ok().expect("Must be here since length >= 3");
    return Ok(Value::new_procedure(ctx.heap(), &params, &body, env));
}

/// Evaluate a `set!` form.
fn evaluate_set(ctx: &mut Context,
                env: &mut RootedEnvironmentPtr,
                form: &RootedValue) -> SchemeResult {
    let mut env_ = env;
    if let Ok(3) = form.len() {
        let pair = form.to_pair().unwrap();
        let sym = try!(pair.cadr(ctx));

        if let Some(str) = sym.to_symbol() {
            let new_value_form = try!(pair.caddr(ctx));
            let new_value = try!(evaluate(ctx, env_, &new_value_form));
            try!(env_.update(str.deref().clone(), &new_value));
            return Ok(ctx.unspecified_symbol());
        }

        return Err("Can only set! symbols".to_string());
    }

    return Err("Improperly formed set! expression".to_string());
}

/// Evaluate a `define` form.
fn evaluate_definition(ctx: &mut Context,
                       env: &mut RootedEnvironmentPtr,
                       form: &RootedValue) -> SchemeResult {
    let mut env_ = env;
    if let Ok(3) = form.len() {
        let pair = form.to_pair().unwrap();
        let sym = try!(pair.cadr(ctx));

        if let Some(str) = sym.to_symbol() {
            let def_value_form = try!(pair.caddr(ctx));
            let def_value = try!(evaluate(ctx, env_, &def_value_form));
            env_.define(str.deref().clone(), &def_value);
            return Ok(ctx.unspecified_symbol());
        }

        return Err("Can only define symbols".to_string());
    }

    return Err("Improperly formed definition".to_string());
}

/// Evaluate a quoted form.
fn evaluate_quoted(ctx: &mut Context, form: &RootedValue) -> SchemeResult {
    if let Some(Value::EmptyList) = form.cdr().unwrap().cdr() {
        return Ok(Rooted::new(ctx.heap(),
                              form.cdr().unwrap().car().unwrap()));
    }

    return Err("Wrong number of parts in quoted form".to_string());
}

/// Evaluate an atom (ie anything that is not a list).
fn evaluate_atom(ctx: &Context,
                 env: &mut RootedEnvironmentPtr,
                 form: &RootedValue) -> SchemeResult {
    if is_auto_quoting(form) {
        return Ok(form.clone());
    }

    if let Value::Symbol(sym) = **form {
        return env.lookup(ctx.heap(), sym.deref());
    }

    return Err(format_args!(format, "Cannot evaluate: {}", **form));
}

/// Evaluate each given form, returning the resulting list of values.
fn evaluate_list(ctx: &mut Context,
                 env: &mut RootedEnvironmentPtr,
                 forms: &RootedValue) -> SchemeResult {
    match **forms {
        Value::EmptyList      => Ok(Rooted::new(ctx.heap(), Value::EmptyList)),
        Value::Pair(ref cons) => {
            let car = Rooted::new(ctx.heap(), cons.car());
            let val = try!(evaluate(ctx, env, &car));

            let cdr = Rooted::new(ctx.heap(), cons.cdr());
            let rest = try!(evaluate_list(ctx, env, &cdr));

            Ok(Value::new_pair(ctx.heap(), &val, &rest))
        },
        _                 => Err("Improper list".to_string()),
    }
}

/// Evaluate each expression in the given cons list `exprs` except for the last
/// expression, whose form is returned (so it can be trampolined to maintain
/// TCO).
fn evaluate_sequence(ctx: &mut Context,
                     env: &mut RootedEnvironmentPtr,
                     exprs: &RootedValue) -> SchemeResult {
    let mut e = exprs.clone();
    loop {
        let ee = *e;
        match ee {
            Value::Pair(ref pair) => {
                if pair.cdr() == Value::EmptyList {
                    return Ok(Rooted::new(ctx.heap(), pair.car()));
                } else {
                    let car = Rooted::new(ctx.heap(), pair.car());
                    try!(evaluate(ctx, env, &car));
                    e.emplace(pair.cdr());
                }
            },
            _                 => {
                return Err("Bad sequence of expressions".to_string());
            },
        }
    }
}

/// TODO FITZGEN
pub fn evaluate_file(ctx: &mut Context, file_path: &str) -> SchemeResult {
    use read::read_from_file;

    let mut reader = try!(read_from_file(file_path, ctx)
                              .ok()
                              .ok_or("Failed to read from file".to_string()));

    let mut result = Rooted::new(ctx.heap(), Value::EmptyList);

    for form in reader {
        result.emplace(*try!(evaluate_in_global_env(ctx, &form)));
    }

    if let Err(ref msg) = *reader.get_result() {
        return Err(msg.clone());
    }

    return Ok(result);
}

#[test]
fn test_eval_integer() {
    let mut ctx = Context::new();
    let result = evaluate_file(&mut ctx, "./tests/test_eval_integer.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::new_integer(42));
}

#[test]
fn test_eval_boolean() {
    let mut ctx = Context::new();
    let result = evaluate_file(&mut ctx, "./tests/test_eval_boolean.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::new_boolean(true));
}

#[test]
fn test_eval_quoted() {
    let mut ctx = Context::new();
    let result = evaluate_file(&mut ctx, "./tests/test_eval_quoted.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::EmptyList);
}

#[test]
fn test_eval_if_consequent() {
    let mut ctx = Context::new();
    let result = evaluate_file(&mut ctx, "./tests/test_eval_if_consequent.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::new_integer(1));
}

#[test]
fn test_eval_if_alternative() {
    let mut ctx = Context::new();
    let result = evaluate_file(&mut ctx, "./tests/test_eval_if_alternative.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::new_integer(2));
}

#[test]
fn test_eval_begin() {
    let mut ctx = Context::new();
    let result = evaluate_file(&mut ctx, "./tests/test_eval_begin.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::new_integer(2));
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
        Rooted::new(ctx.heap(), Value::new_integer(2))
    ];
    let def_form = list(&mut ctx, &mut def_items);
    evaluate_in_global_env(&mut ctx, &def_form).ok()
        .expect("Should be able to define");

    let foo_symbol_ = ctx.get_or_create_symbol("foo".to_string());

    let def_val = evaluate_in_global_env(&mut ctx, &foo_symbol_).ok()
        .expect("Should be able to get a defined symbol's value");
    assert_eq!(*def_val, Value::new_integer(2));

    let mut set_items = [
        set_bang_symbol,
        foo_symbol_,
        Rooted::new(ctx.heap(), Value::new_integer(1))
    ];
    let set_form = list(&mut ctx, &mut set_items);
    evaluate_in_global_env(&mut ctx, &set_form).ok()
        .expect("Should be able to define");

    let foo_symbol__ = ctx.get_or_create_symbol("foo".to_string());

    let set_val = evaluate_in_global_env(&mut ctx, &foo_symbol__).ok()
        .expect("Should be able to get a defined symbol's value");
    assert_eq!(*set_val, Value::new_integer(1));
}

#[test]
fn test_eval_and_call_lambda() {
    let mut ctx = Context::new();
    let result = evaluate_file(&mut ctx, "./tests/test_eval_and_call_lambda.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::new_integer(5));
}

#[test]
fn test_eval_closures() {
    let mut ctx = Context::new();
    let result = evaluate_file(&mut ctx, "./tests/test_eval_closures.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::new_integer(1));
}