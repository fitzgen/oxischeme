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
    let mut env_ = &mut Rooted::new(ctx.heap(), **env);
    let mut form_ = &Rooted::new(ctx.heap(), **form);
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

                let condition_form = try!(pair.cadr());
                let condition_val = try!(evaluate(ctx, env_, &condition_form));

                form_.emplace(*try!(if *condition_val == Value::new_boolean(false) {
                    // Alternative.
                    pair.cadddr()
                } else {
                    // Consequent.
                    pair.caddr()
                }));
                continue;
            },

            // `(begin ...)` sequences.
            v if v == *begin => {
                form_.emplace(
                    *try!(evaluate_sequence(ctx,
                                            env_,
                                            &Rooted::new(ctx.heap(),
                                                         pair.cdr()))));
                continue;
            },

            // Procedure invocations.
            procedure        => {
                // Ensure that the form is a proper list.
                try!(form_.len().ok().ok_or("Bad invocation form".to_string()));

                let proc_val = try!(evaluate(ctx,
                                             env_,
                                             &Rooted::new(ctx.heap(),
                                                          procedure)));
                let proc_ptr = try!(proc_val.to_procedure().ok_or(
                    format_args!(format,
                                 "Expected a procedure, found {}",
                                 *proc_val)));
                let args = try!(evaluate_list(ctx,
                                              env_,
                                              &Rooted::new(ctx.heap(),
                                                           pair.cdr())));

                let proc_env = try!(Environment::extend(
                    ctx.heap(),
                    &Rooted::new(ctx.heap(), proc_ptr.get_env()),
                    &Rooted::new(ctx.heap(), proc_ptr.get_params()),
                    &args));
                env_.emplace(*proc_env);
                form_.emplace(
                    *try!(evaluate_sequence(
                        ctx,
                        env_,
                        &Rooted::new(ctx.heap(),
                                     proc_ptr.get_body()))));
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
    let params = pair.cadr().ok().expect("Must be here since length >= 3");
    let body = pair.cddr().ok().expect("Must be here since length >= 3");
    return Ok(Value::new_procedure(ctx.heap(), &params, &body, env));
}

/// Evaluate a `set!` form.
fn evaluate_set(ctx: &mut Context,
                env: &mut RootedEnvironmentPtr,
                form: &RootedValue) -> SchemeResult {
    let mut env_ = env;
    if let Ok(3) = form.len() {
        let pair = form.to_pair().unwrap();
        let sym = try!(pair.cadr());

        if let Some(str) = sym.to_symbol() {
            let new_value_form = try!(pair.caddr());
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
        let sym = try!(pair.cadr());

        if let Some(str) = sym.to_symbol() {
            let def_value_form = try!(pair.caddr());
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
        return Ok(Rooted::new(ctx.heap(), **form));
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
    let mut e = Rooted::new(ctx.heap(), **exprs);
    loop {
        match *e {
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
fn test_eval_and_call_lambda() {
    use read::read_from_file;

    let mut ctx = Context::new();
    let mut reader = read_from_file("./tests/test_eval_and_call_lambda.scm",
                                    &mut ctx)
        .ok()
        .expect("Should be able to read from a file");
    let form = reader.next().expect("Should have a lambda form");
    let result = evaluate_in_global_env(&mut ctx, form)
        .ok()
        .expect("Should be able to evaluate a lambda.");
    assert_eq!(result, Value::new_integer(5));
}

#[test]
fn test_eval_closures() {
    use read::read_from_file;

    let mut ctx = Context::new();
    let mut reader = read_from_file("./tests/test_eval_closures.scm",
                                    &mut ctx)
        .ok()
        .expect("Should be able to read from a file");
    let form = reader.next().expect("Should have a lambda form");
    let result = evaluate_in_global_env(&mut ctx, form)
        .ok()
        .expect("Should be able to evaluate closures");
    assert_eq!(result, Value::new_integer(1));
}