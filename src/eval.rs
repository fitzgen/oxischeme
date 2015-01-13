// Copyright 2015 Nick Fitzgerald
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

//! TODO FITZGEN

use std::fmt;

use environment::{RootedActivationPtr};
use heap::{Heap, Rooted};
use value::{RootedValue, SchemeResult, Value};

/// Evaluate the given form in the global environment.
pub fn evaluate(heap: &mut Heap, form: &RootedValue) -> SchemeResult {
    let meaning = try!(analyze(heap, form));
    let mut act = heap.global_activation();
    meaning.evaluate(heap, &mut act)
}

/// Evaluate the file at the given path and return the value of the last form.
pub fn evaluate_file(heap: &mut Heap, file_path: &str) -> SchemeResult {
    use read::read_from_file;
    let mut reader = try!(read_from_file(file_path, heap).ok().ok_or(
        "Failed to read from file".to_string()));
    let mut result = Rooted::new(heap, Value::EmptyList);
    for form in reader {
        result.emplace(*try!(evaluate(heap, &form)));
    }
    if let Err(ref msg) = *reader.get_result() {
        return Err(msg.clone());
    }
    return Ok(result);
}

/// TODO FITZGEN
#[deriving(Show)]
pub enum Trampoline {
    Value(RootedValue),
    Thunk(Meaning),
}

/// TODO FITZGEN
pub type TrampolineResult = Result<Trampoline, String>;

/// TODO FITZGEN
#[deriving(Clone, Show)]
enum MeaningData {
    Quotation(RootedValue),
    Reference(u32, u32),
    Definition(Meaning),
    SetVariable(u32, u32, Meaning),
    Conditional(Meaning, Meaning, Meaning),
    Sequence(Meaning, Meaning),
    Lambda(u32, 
}

/// TODO FITZGEN
type MeaningEvaluatorFn = fn(&mut Heap,
                             &MeaningData,
                             &mut RootedActivationPtr) -> TrampolineResult;

fn evaluate_quotation(heap: &mut Heap,
                      data: &MeaningData,
                      act: &mut RootedActivationPtr) -> TrampolineResult {
    if let MeaningData::Quotation(ref val) = *data {
        return Ok(Trampoline::Value(Rooted::new(heap, **val)));
    }

    panic!("unsynchronized MeaningData and MeaningEvaluatorFn");
}

fn evaluate_reference(heap: &mut Heap,
                      data: &MeaningData,
                      act: &mut RootedActivationPtr) -> TrampolineResult {
    if let MeaningData::Reference(i, j) = *data {
        return Ok(Trampoline::Value(act.fetch(heap, i, j)));
    }

    panic!("unsynchronized MeaningData and MeaningEvaluatorFn");
}

fn evaluate_definition(heap: &mut Heap,
                       data: &MeaningData,
                       act: &mut RootedActivationPtr) -> TrampolineResult {
    if let MeaningData::Definition(ref definition_value_meaning) = *data {
        let val = try!(definition_value_meaning.evaluate(heap, act));
        act.push_value(*val);
        return Ok(Trampoline::Value(heap.unspecified_symbol()));
    }

    panic!("unsynchronized MeaningData and MeaningEvaluatorFn");
}

fn evaluate_set_variable(heap: &mut Heap,
                         data: &MeaningData,
                         act: &mut RootedActivationPtr) -> TrampolineResult {
    if let MeaningData::SetVariable(i, j, ref definition_value_meaning) = *data {
        let val = try!(definition_value_meaning.evaluate(heap, act));
        act.update(heap, i, j, &val);
        return Ok(Trampoline::Value(heap.unspecified_symbol()));
    }

    panic!("unsynchronized MeaningData and MeaningEvaluatorFn");
}

fn evaluate_conditional(heap: &mut Heap,
                        data: &MeaningData,
                        act: &mut RootedActivationPtr) -> TrampolineResult {
    if let MeaningData::Conditional(ref condition,
                                    ref consequent,
                                    ref alternative) = *data {
        let val = try!(condition.evaluate(heap, act));
        return Ok(Trampoline::Thunk(if *val == Value::new_boolean(false) {
            (*alternative).clone()
        } else {
            (*consequent).clone()
        }));
    }

    panic!("unsynchronized MeaningData and MeaningEvaluatorFn");
}

fn evaluate_sequence(heap: &mut Heap,
                     data: &MeaningData,
                     act: &mut RootedActivationPtr) -> TrampolineResult {
    if let MeaningData::Sequence(ref first, ref second) = *data {
        try!(first.evaluate(heap, act));
        return Ok(Trampoline::Thunk(second.clone()));
    }

    panic!("unsynchronized MeaningData and MeaningEvaluatorFn");
}

fn evaluate_lambda(heap: &mut Heap,
                   data: &MeaningData,
                   act: &mut RootedActivationPtr) -> TrampolineResult {
    if let MeaningData::Lambda(arity, ref body) = *data {
        return Ok(Trampoline::Value(
            Value::new_procedure(heap, arity, (*body).clone())));
    }

    panic!("unsynchronized MeaningData and MeaningEvaluatorFn");
}

/// TODO FITZGEN
pub struct Meaning {
    data: Box<MeaningData>,
    evaluator: MeaningEvaluatorFn,
}

/// ## `Meaning` Constructors
impl Meaning {
    /// TODO FITZGEN
    fn new_quotation(form: &RootedValue) -> Meaning {
        Meaning {
            data: box MeaningData::Quotation((*form).clone()),
            evaluator: evaluate_quotation,
        }
    }

    /// TODO FITZGEN
    fn new_reference(i: u32, j: u32) -> Meaning {
        Meaning {
            data: box MeaningData::Reference(i, j),
            evaluator: evaluate_reference,
        }
    }

    /// TODO FITZGEN
    fn new_set_variable(i: u32, j: u32, val: Meaning) -> Meaning {
        Meaning {
            data: box MeaningData::SetVariable(i, j, val),
            evaluator: evaluate_set_variable,
        }
    }

    /// TODO FITZGEN
    fn new_conditional(condition: Meaning,
                       consquent: Meaning,
                       alternative: Meaning) -> Meaning {
        Meaning {
            data: box MeaningData::Conditional(condition,
                                               consquent,
                                               alternative),
            evaluator: evaluate_conditional,
        }
    }

    /// TODO FITZGEN
    fn new_sequence(first: Meaning, second: Meaning) -> Meaning {
        Meaning {
            data: box MeaningData::Sequence(first, second),
            evaluator: evaluate_sequence,
        }
    }

    /// TODO FITZGEN
    fn new_definition(defined: Meaning) -> Meaning {
        Meaning {
            data: box MeaningData::Definition(defined),
            evaluator: evaluate_definition,
        }
    }

    /// TODO FITZGEN
    fn new_lambda(arity: u32, body: Meaning) -> Meaning {
        Meaning {
            data: box MeaningData::Lambda(arity, body),
            evaluator: evaluate_lambda
        }
    }
}

/// ## `Meaning` Methods
impl Meaning {
    /// TODO FITZGEN
    fn evaluate_to_thunk(&self,
                         heap: &mut Heap,
                         act: &mut RootedActivationPtr) -> TrampolineResult {
        (self.evaluator)(heap, &*self.data, act)
    }

    /// TODO FITZGEN
    fn evaluate(&self,
                heap: &mut Heap,
                act: &mut RootedActivationPtr) -> SchemeResult {
        let mut trampoline = try!(self.evaluate_to_thunk(heap, act));
        loop {
            trampoline = match trampoline {
                Trampoline::Value(v) => { return Ok(v); },
                Trampoline::Thunk(m) => {
                    try!(m.evaluate_to_thunk(heap, act))
                }
            }
        }
    }
}

impl Clone for Meaning {
    fn clone(&self) -> Self {
        Meaning {
            data: box self.data.deref().clone(),
            evaluator: self.evaluator,
        }
    }
}

impl fmt::Show for Meaning {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Meaning(data: {}, evaluator: {})",
               self.data,
               self.evaluator as uint)
    }
}

/// TODO FITZGEN
pub type MeaningResult = Result<Meaning, String>;

/// TODO FITZGEN: impl Trace for Meaning

/// TODO FITZGEN
pub fn analyze(heap: &mut Heap,
               form: &RootedValue) -> MeaningResult {
    if form.is_atom() {
        return analyze_atom(heap, form);
    }

    let pair = form.to_pair(heap).expect(
        "If a value is not an atom, then it must be a pair.");

    let quote = heap.quote_symbol();
    let if_symbol = heap.if_symbol();
    let begin = heap.begin_symbol();
    let define = heap.define_symbol();
    let set_bang = heap.set_bang_symbol();
    let lambda = heap.lambda_symbol();

    match *pair.car(heap) {
        v if v == *quote     => analyze_quoted(heap, form),
        v if v == *define    => analyze_definition(heap, form),
        v if v == *set_bang  => analyze_set(heap, form),
        v if v == *lambda    => analyze_lambda(heap, form),
        v if v == *if_symbol => analyze_conditional(heap, form),
        v if v == *begin     => analyze_sequence(heap, form),
        _                    => analyze_invocation(heap, form),
    }
}

/// Return true if the form doesn't need to be evaluated because it is
/// "autoquoting" or "self evaluating", false otherwise.
fn is_auto_quoting(form: &RootedValue) -> bool {
    match **form {
        Value::EmptyList    => false,
        Value::Pair(_)      => false,
        Value::Symbol(_)    => false,
        _                   => true,
    }
}

/// TODO FITZGEN
fn analyze_atom(heap: &mut Heap,
                form: &RootedValue) -> MeaningResult {
    if is_auto_quoting(form) {
        return Ok(Meaning::new_quotation(form));
    }

    if let Some(sym) = form.to_symbol(heap) {
        if let Some((i, j)) = heap.environment.lookup(&**sym) {
            return Ok(Meaning::new_reference(i, j));
        }

        // TODO FITZGEN: add to global environment. At runtime, ensure it is
        // defined before accessing or else error.
        return Err(format!("Static error: reference to unknown variable: {}",
                           **sym));
    }

    return Err(format!("Static error: Cannot evaluate: {}", **form));
}

/// TODO FITZGEN
fn analyze_quoted(heap: &mut Heap, form: &RootedValue) -> MeaningResult {
    if let Ok(2) = form.len() {
        return Ok(Meaning::new_quotation(
            &form.cdr(heap).unwrap().car(heap).unwrap()));
    }

    return Err(
        "Static error: Wrong number of parts in quoted form".to_string());
}

/// TODO FITZGEN
fn analyze_definition(heap: &mut Heap,
                      form: &RootedValue) -> MeaningResult {
    if let Ok(3) = form.len() {
        let pair = form.to_pair(heap).expect(
            "If len = 3, then form must be a pair");
        let sym = try!(pair.cadr(heap));

        if let Some(str) = sym.to_symbol(heap) {
            let def_value_form = try!(pair.caddr(heap));
            let def_value_meaning = try!(analyze(heap, &def_value_form));

            if let Some((0, j)) = heap.environment.lookup(&**str) {
                // The variable is already defined in this scope, just overwrite
                // it.
                return Ok(Meaning::new_set_variable(0, j, def_value_meaning));
            } else {
                heap.environment.define((**str).clone());
                return Ok(Meaning::new_definition(def_value_meaning));
            }
        }

        return Err("Static error: can only define symbols".to_string());
    }

    return Err("Static error: improperly formed definition".to_string());
}

/// TODO FITZGEN
fn analyze_set(heap: &mut Heap,
               form: &RootedValue) -> MeaningResult {
    if let Ok(3) = form.len() {
        let pair = form.to_pair(heap).expect(
            "If len = 3, then form must be a pair");
        let sym = try!(pair.cadr(heap));

        if let Some(str) = sym.to_symbol(heap) {
            let set_value_form = try!(pair.caddr(heap));
            let set_value_meaning = try!(analyze(heap, &set_value_form));
            if let Some((i, j)) = heap.environment.lookup(&**str) {
                return Ok(Meaning::new_set_variable(i, j, set_value_meaning));
            }

            // TODO FITZGEN: should add the global variable here, but mark it
            // undefined. Generate a meaning that ensures it is defined before
            // setting.
            return Err(format!(
                "Static error: cannot set! undefined variable: {}",
                *str));
        }

        return Err("Static error: can only set! symbols".to_string());
    }

    return Err("Static error: improperly formed set! expression".to_string());
}

/// TODO FITZGEN
fn analyze_lambda(heap: &mut Heap,
                  form: &RootedValue) -> MeaningResult {
    let length = try!(form.len().ok().ok_or("Bad lambda form".to_string()));
    if length < 3 {
        return Err("Lambda is missing body".to_string());
    }

    let pair = form.to_pair(heap).unwrap();

    let params = pair.cadr(heap).ok().expect("Must be here since length >= 3");
    let arity = try!(params.len().ok().ok_or(
        "Bad lambda parameters"/.to_string()));

    // TODO FITZGEN: extend environment with parameters.

    let body = pair.cddr(heap).ok().expect("Must be here since length >= 3");
    let body_meaning = make_meaning_sequence(heap, &body);

    retur Ok(Meaning::new_lambda(arity, body_meaning));
}

/// TODO FITZGEN
fn analyze_conditional(heap: &mut Heap,
                       form: &RootedValue) -> MeaningResult {
    if let Ok(4) = form.len() {
        let pair = form.to_pair(heap).expect(
            "If len = 4, then form must be a pair");

        let condition_form = try!(pair.cadr(heap));
        let condition_meaning = try!(analyze(heap, &condition_form));

        let consequent_form = try!(pair.caddr(heap));
        let consequent_meaning = try!(analyze(heap, &consequent_form));

        let alternative_form = try!(pair.cadddr(heap));
        let alternative_meaning = try!(analyze(heap, &alternative_form));

        return Ok(Meaning::new_conditional(condition_meaning,
                                           consequent_meaning,
                                           alternative_meaning));
    }

    return Err("Static error: improperly formed if expression".to_string());
}

/// TODO FITZGEN
fn make_meaning_sequence(heap: &mut Heap,
                         forms: &RootedValue) -> MeaningResult {
    if let Some(ref cons) = forms.to_pair(heap) {
        let first_form = cons.car(heap);
        let first = try!(analyze(heap, &first_form));

        if *cons.cdr(heap) == Value::EmptyList {
            return Ok(first);
        } else {
            let rest_forms = cons.cdr(heap);
            let rest = try!(make_meaning_sequence(heap, &rest_forms));
            return Ok(Meaning::new_sequence(first, rest));
        }
    }

    return Err("Static error: improperly formed sequence".to_string());
}

/// TODO FITZGEN
fn analyze_sequence(heap: &mut Heap,
                    form: &RootedValue) -> MeaningResult {
    let forms = try!(form.cdr(heap).ok_or(
        "Static error: improperly formed sequence".to_string()));
    make_meaning_sequence(heap, &forms)
}

/// TODO FITZGEN
fn analyze_invocation(heap: &mut Heap,
                      form: &RootedValue) -> MeaningResult {
    return Err("TODO FITZGEN".to_string());
}

// TESTS -----------------------------------------------------------------------

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
    evaluate(heap, &def_form).ok()
        .expect("Should be able to define");

    let foo_symbol_ = heap.get_or_create_symbol("foo".to_string());

    let def_val = evaluate(heap, &foo_symbol_).ok()
        .expect("Should be able to get a defined symbol's value");
    assert_eq!(*def_val, Value::new_integer(2));

    let mut set_items = [
        set_bang_symbol,
        foo_symbol_,
        Rooted::new(heap, Value::new_integer(1))
    ];
    let set_form = list(heap, &mut set_items);
    evaluate(heap, &set_form).ok()
        .expect("Should be able to define");

    let foo_symbol__ = heap.get_or_create_symbol("foo".to_string());

    let set_val = evaluate(heap, &foo_symbol__).ok()
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

#[test]
fn test_ref_defined_later() {
    let mut heap = Heap::new();
    let result = evaluate_file( &mut heap, "./tests/test_ref_defined_later.scm")
        .ok()
        .expect("Should be able to eval a file.");
    assert_eq!(*result, Value::new_integer(1));
}