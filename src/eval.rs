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

        return Err(format_args!(format, "Cannot evaluate: {}", val));
    }

    let pair_val = val.to_pair()
        .expect("If a value is not an atom, then it must be a pair.");

    let quote = ctx.quote_symbol();
    let if_symbol = ctx.if_symbol();

    match pair_val.car() {
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
                let condition_form = try!(pair_val.cadr());
                let condition_val = try!(evaluate(ctx, condition_form));

                if condition_val == Value::new_boolean(false) {
                    let alternative_form = try!(pair_val.cadddr());
                    return evaluate(ctx, alternative_form);
                }

                let consequent_form = try!(pair_val.caddr());
                return evaluate(ctx, consequent_form);
            }

            return Err("Improperly formed if expression".to_string());
        },

        _                  => {
            return Err(format_args!(format, "Cannot evaluate: {}", val));
        },
    };
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
               Ok(Value::new_integer(1)))
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
               Ok(Value::new_integer(2)))
}