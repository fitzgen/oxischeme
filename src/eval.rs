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
use value::{Value};

fn is_auto_quoting(val: &Value) -> bool {
    match *val {
        Value::EmptyList    => false,
        Value::Pair(_)      => false,
        Value::Symbol(_)    => false,
        Value::String(_)    => true,
        Value::Integer(_)   => true,
        Value::Boolean(_)   => true,
        Value::Character(_) => true,
    }
}

fn is_quoted(ctx: &mut Context, val: &Value) -> bool {
    let quote_symbol = ctx.get_or_create_symbol("quote".to_string());
    match val.car() {
        Some(s) if s == quote_symbol => {
            val.cdr().unwrap().is_pair()
        },
        _                                                 => false,
    }
}

pub type SchemeResult = Result<Value, String>;

/// Evaluate the given value.
pub fn evaluate(ctx: &mut Context, val: Value) -> SchemeResult {
    if is_auto_quoting(&val) {
        return Ok(val);
    }

    if is_quoted(ctx, &val) {
        return Ok(val.cdr().unwrap().car().unwrap());
    }

    return Err(format_args!(format, "Value is not quoted or auto-quoting: {}", val))
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
        ctx.get_or_create_symbol("quote".to_string()),
        val
    ];
    let quoted = list(&mut ctx, &mut items);
    assert_eq!(evaluate(&mut ctx, quoted),
               Ok(val));
}