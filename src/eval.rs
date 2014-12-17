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

use value;

/// Evaluate the given value.
pub fn evaluate(val: value::Value) -> value::Value {
    val
}

#[test]
fn test_eval_integer() {
    assert_eq!(evaluate(value::Value::new_integer(42 as i64)),
               value::Value::new_integer(42 as i64));
}