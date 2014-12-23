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
use value::{SchemeResult, Value};

fn is_auto_quoting(val: &Value) -> bool {
    match *val {
        Value::EmptyList    => false,
        Value::Pair(_)      => false,
        Value::Symbol(_)    => false,
        _                   => true,
    }
}

/// Evaluate the given value.
pub fn evaluate(ctx: &mut Context, val: Value) -> SchemeResult {
    if val.is_atom() {
        if is_auto_quoting(&val) {
            return Ok(val);
        }

        if let Value::Symbol(sym) = val {
            return ctx.env().lookup(sym.deref());
        }

        return Err(format_args!(format, "Cannot evaluate: {}", val));
    }

    let pair = val.to_pair()
        .expect("If a value is not an atom, then it must be a pair.");

    let quote = ctx.quote_symbol();
    let if_symbol = ctx.if_symbol();
    let begin = ctx.begin_symbol();
    let define = ctx.define_symbol();
    let set_bang = ctx.set_bang_symbol();

    match pair.car() {
        // Quoted forms.
        v if v == quote => {
            if let Some(Value::EmptyList) = val.cdr().unwrap().cdr() {
                return Ok(val.cdr().unwrap().car().unwrap());
            }

            return Err("Wrong number of parts in quoted form".to_string());
        },

        // If expressions.
        v if v == if_symbol => {
            if let Ok(4) = val.len() {
                let condition_form = try!(pair.cadr());
                let condition_val = try!(evaluate(ctx, condition_form));

                if condition_val == Value::new_boolean(false) {
                    let alternative_form = try!(pair.cadddr());
                    return evaluate(ctx, alternative_form);
                }

                let consequent_form = try!(pair.caddr());
                return evaluate(ctx, consequent_form);
            }

            return Err("Improperly formed if expression".to_string());
        },

        // `(begin ...)` sequences.
        v if v == begin => {
            return evaluate_sequence(ctx, pair.cdr());
        },

        // Definitions. These are only supposed to be allowed at the top level
        // and at the beginning of a body, but we are punting on that
        // restriction for now.
        v if v == define => {
            if let Ok(3) = val.len() {
                let sym = try!(pair.cadr());

                if let Some(str) = sym.to_symbol() {
                    let def_value_form = try!(pair.caddr());
                    let def_value = try!(evaluate(ctx, def_value_form));
                    ctx.env().define(str.deref().clone(), def_value);
                    return Ok(ctx.unspecified_symbol());
                }

                return Err("Can only define symbols".to_string());
            }

            return Err("Improperly formed definition".to_string());
        },

        // `set!` assignment.
        v if v == set_bang => {
            if let Ok(3) = val.len() {
                let sym = try!(pair.cadr());

                if let Some(str) = sym.to_symbol() {
                    let new_value_form = try!(pair.caddr());
                    let new_value = try!(evaluate(ctx, new_value_form));
                    try!(ctx.env().update(str.deref().clone(), new_value));
                    return Ok(ctx.unspecified_symbol());
                }

                return Err("Can only set! symbols".to_string());
            }

            return Err("Improperly formed set! expression".to_string());
        },

        _                  => {
            return Err(format_args!(format, "Cannot evaluate: {}", val));
        },
    };
}

/// Evaluate each expression in the given cons list `exprs` and return the value
/// of the last expression.
fn evaluate_sequence(ctx: &mut Context, exprs: Value) -> SchemeResult {
    let mut e = exprs;
    loop {
        match e {
            Value::EmptyList  => return Ok(Value::EmptyList),
            Value::Pair(pair) => {
                let v = try!(evaluate(ctx, pair.car()));
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
    assert_eq!(evaluate(&mut ctx, Value::new_integer(42)),
               Ok(Value::new_integer(42)));
}

#[test]
fn test_eval_boolean() {
    let mut ctx = Context::new();
    assert_eq!(evaluate(&mut ctx, Value::new_boolean(true)),
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
    assert_eq!(evaluate(&mut ctx, quoted),
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
    assert_eq!(evaluate(&mut ctx, if_form),
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
    assert_eq!(evaluate(&mut ctx, if_form),
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
    assert_eq!(evaluate(&mut ctx, begin_form),
               Ok(Value::new_integer(2)));
}