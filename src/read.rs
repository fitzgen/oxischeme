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

//! Parsing values.

use std::cell::{RefCell};
use std::fmt::{format};
use std::io::{BufferedReader, IoError, IoErrorKind, MemReader};
use std::iter::{Peekable};

use context::{Context};
use value::{Value, list};

/// `CharReader` reads characters one at a time from the given input `Reader`.
struct CharReader<R> {
    reader: BufferedReader<R>,
}

impl<R: Reader> CharReader<R> {
    /// Create a new `CharReader` instance.
    pub fn new(reader: R) -> CharReader<R> {
        CharReader {
            reader: BufferedReader::new(reader)
        }
    }
}

impl<R: Reader> Iterator<char> for CharReader<R> {
    /// Returns `Some(c)` for each character `c` from the input reader. Upon
    /// reaching EOF, returns `None`.
    fn next(&mut self) -> Option<char> {
        match self.reader.read_char() {
            Ok(c)                   => Some(c),
            Err(ref e) if is_eof(e) => None,
            Err(e)                  => panic!("IO ERROR! {}", e),
        }
    }
}

/// Return true if the error is reaching the end of file, false otherwise.
fn is_eof(err : &IoError) -> bool {
    err.kind == IoErrorKind::EndOfFile
}

/// Return true if the character is the start of a comment, false otherwise.
fn is_comment(c: &char) -> bool {
    *c == ';'
}

/// Return true if the character is a delimiter between tokens, false otherwise.
fn is_delimiter(c: &char) -> bool {
    c.is_whitespace() || is_comment(c) || *c == ')' || *c == '('
}

/// Return true if we have EOF (`None`) or a delimiting character, false
/// otherwise.
fn is_eof_or_delimiter(oc: &Option<char>) -> bool {
    match *oc {
        None                           => true,
        Some(ref c) if is_delimiter(c) => true,
        _                              => false,
    }
}

fn is_symbol_initial(c: &char) -> bool {
    c.is_alphabetic() || is_symbol_special_initial(c) || is_symbol_peculiar(c)
}

fn is_symbol_peculiar(c: &char) -> bool {
    *c == '+' || *c == '-' || *c == '…'
}

fn is_symbol_special_initial(c: &char) -> bool {
    *c == '!' || *c == '$' || *c == '%' || *c == '&' || *c == '*' ||
        *c == '/' || *c == ':' || *c == '<' || *c == '=' || *c == '>' ||
        *c == '?' || *c == '~' || *c == '_' || *c == '^'
}

fn is_symbol_subsequent(c: &char) -> bool {
    is_symbol_initial(c) || c.is_digit(10) || *c == '.' || *c == '+' || *c == '-'
}

/// `Read` iteratively parses values from the input `Reader`.
pub struct Read<R> {
    chars: RefCell<Peekable<char, CharReader<R>>>,
    result: Result<(), String>,
    context: *mut Context,
}

impl<'a, R: Reader> Read<R> {
    /// Create a new `Read` instance from the given `Reader` input source.
    pub fn new(reader: R, ctx: *mut Context) -> Read<R> {
        Read {
            chars: RefCell::new(CharReader::new(reader).peekable()),
            result: Ok(()),
            context: ctx
        }
    }

    /// Get the current context.
    fn ctx(&'a self) -> &'a mut Context {
        unsafe {
            self.context.as_mut()
                .expect("Read<R> should always have a valid Context")
        }
    }

    /// Peek at the next character in our input stream.
    fn peek_char(&self) -> Option<char> {
        match self.chars.borrow_mut().peek() {
            None    => None,
            Some(c) => Some(*c)
        }
    }

    /// Take the next character from the input stream.
    fn next_char(&mut self) -> Option<char> {
        self.chars.borrow_mut().next()
    }

    /// Skip to after the next newline character.
    fn skip_line(&mut self) {
        loop {
            match self.peek_char() {
                None       => return,
                Some('\n') => return,
                _          => { },
            }
            self.next_char();
        }
    }

    /// Trim initial whitespace and skip comments.
    fn trim(&mut self) {
        loop {
            let skip_line = match self.peek_char() {
                Some(c) if c.is_whitespace() => false,
                Some(c) if is_comment(&c)    => true,
                _                            => return,
            };

            if skip_line {
                self.skip_line();
            } else {
                self.next_char();
            }
        }
    }

    /// Get the results of parsing thus far. If there was an error parsing, a
    /// diagnostic message will be the value of the error.
    pub fn get_result(&'a self) -> &'a Result<(), String> {
        &self.result
    }

    /// Report a failure reading values.
    fn report_failure(&mut self, msg: String) -> Option<Value> {
        // Don't overwrite existing failures.
        match self.result {
            Ok(_) => self.result = Err(msg),
            _     => { },
        };

        None
    }

    /// Report an unexpected character.
    fn unexpected_character(&mut self, c: &char) -> Option<Value> {
        self.report_failure(format_args!(format, "Unexpected character: {}", c))
    }

    /// Expect that the next character is `c` and report a failure if it is not.
    fn expect_character(&mut self, c: char) -> Result<(), ()>{
        match self.next_char() {
            None                => {
                self.report_failure(
                    format_args!(format, "Expected '{}', but found EOF.", c));
                Err(())
            },
            Some(d) if d != c => {
                self.report_failure(
                    format_args!(format, "Expected '{}', found: '{}'", c, d));
                Err(())
            },
            _                   => Ok(()),
        }
    }

    /// Report an unexpected EOF.
    fn unexpected_eof(&mut self) -> Option<Value> {
        self.report_failure("Unexpected EOF".to_string())
    }

    /// Report a bad character literal, e.g. `#\bad`.
    fn bad_character_literal(&mut self) -> Option<Value> {
        self.report_failure("Bad character value".to_string())
    }

    /// Report an unterminated string literal.
    fn unterminated_string(&mut self) -> Option<Value> {
        self.report_failure("Unterminated string literal".to_string())
    }

    /// Read a character value, after the starting '#' and '\' characters have
    /// already been eaten.
    fn read_character(&mut self) -> Option<Value> {
        match [self.next_char(), self.peek_char()] {
            // Normal character, e.g. `#\f`.
            [Some(c), d] if is_eof_or_delimiter(&d) => Some(Value::new_character(c)),

            // Newline character: `#\newline`.
            [Some('n'), Some('e')] => match [self.next_char(),
                                             self.next_char(),
                                             self.next_char(),
                                             self.next_char(),
                                             self.next_char(),
                                             self.next_char(),
                                             self.peek_char()] {
                [Some('e'),
                 Some('w'),
                 Some('l'),
                 Some('i'),
                 Some('n'),
                 Some('e'),
                 d] if is_eof_or_delimiter(&d) => Some(Value::new_character('\n')),
                _                              => self.bad_character_literal(),
            },

            // Space character: `#\space`.
            [Some('s'), Some('p')] => match [self.next_char(),
                                             self.next_char(),
                                             self.next_char(),
                                             self.next_char(),
                                             self.peek_char()] {
                [Some('p'),
                 Some('a'),
                 Some('c'),
                 Some('e'),
                 d] if is_eof_or_delimiter(&d) => Some(Value::new_character(' ')),
                _                              => self.bad_character_literal(),
            },

            // Tab character: `#\tab`.
            [Some('t'), Some('a')] => match [self.next_char(),
                                             self.next_char(),
                                             self.peek_char()] {
                [Some('a'),
                 Some('b'),
                 d] if is_eof_or_delimiter(&d) => Some(Value::new_character('\t')),
                _                              => self.bad_character_literal(),
            },

            _ => self.bad_character_literal(),
        }
    }

    /// Given that we have already read a '#' character, read in either a
    /// boolean or a character.
    fn read_bool_or_char(&mut self) -> Option<Value> {
        self.next_char();
        // Deterimine if this is a boolean or a character.
        match [self.next_char(), self.peek_char()] {
            [Some('t'), d] if is_eof_or_delimiter(&d)  => {
                Some(Value::new_boolean(true))
            },
            [Some('f'), d] if is_eof_or_delimiter(&d)  => {
                Some(Value::new_boolean(false))
            },
            [Some('\\'), _]                            => {
                self.read_character()
            },
            [Some(c), _]                               => {
                self.unexpected_character(&c)
            },
            _                                          => None,
        }
    }

    /// Read an integer.
    fn read_integer(&mut self, is_negative: bool) -> Option<Value> {
        let sign : i64 = if is_negative { -1 } else { 1 };

        let mut abs_value : i64 = match self.next_char() {
            None    => return self.unexpected_eof(),
            Some(c) => match c.to_digit(10) {
                None    => return self.unexpected_character(&c),
                Some(d) => d as i64
            }
        };

        loop {
            match self.peek_char() {
                None                        => break,
                Some(c) if is_delimiter(&c) => break,
                Some(c)                     => match c.to_digit(10) {
                    None    => return self.unexpected_character(&c),
                    Some(d) => abs_value = (abs_value * 10) + (d as i64),
                }
            }
            self.next_char();
        }

        Some(Value::new_integer(abs_value * sign))
    }

    /// Read a pair, with the leading '(' already taken from the input.
    fn read_pair(&mut self) -> Option<Value> {
        self.trim();
        match self.peek_char() {
            None      => return self.unexpected_eof(),

            Some(')') => {
                self.next_char();
                return Some(Value::EmptyList);
            },

            _         => {
                let car = match self.next() {
                    None    => return self.unexpected_eof(),
                    Some(v) => v,
                };

                self.trim();
                match self.peek_char() {
                    None      => return self.unexpected_eof(),

                    // Improper list.
                    Some('.') => {
                        self.next_char();
                        let cdr = match self.next() {
                            None    => return self.unexpected_eof(),
                            Some(v) => v,
                        };

                        self.trim();
                        if self.expect_character(')').is_err() {
                            return None;
                        }

                        return Some(Value::new_pair(self.ctx().heap(), car, cdr));
                    },

                    // Proper list.
                    _         => {
                        let cdr = match self.read_pair() {
                            None    => return self.unexpected_eof(),
                            Some(v) => v,
                        };

                        return Some(Value::new_pair(self.ctx().heap(), car, cdr));
                    },
                };
            },
        };
    }

    /// Read a string in from the input.
    fn read_string(&mut self) -> Option<Value> {
        if self.expect_character('"').is_err() {
            return None;
        }

        let mut str = String::new();

        loop {
            match self.next_char() {
                None       => return self.unterminated_string(),
                Some('"')  => return Some(Value::new_string(self.ctx().heap(), str)),
                Some('\\') => {
                    match self.next_char() {
                        Some('n')  => str.push('\n'),
                        Some('t')  => str.push('\t'),
                        Some('\\') => str.push('\\'),
                        Some('"')  => str.push('"'),
                        Some(c)    => return self.unexpected_character(&c),
                        None       => return self.unterminated_string(),
                    }
                },
                Some(c)    => str.push(c),
            }
        }
    }

    /// Read a symbol in from the input. Optionally supply a prefix character
    /// that was already read from the symbol.
    fn read_symbol(&mut self, prefix: Option<char>) -> Option<Value> {
        let mut str = String::new();

        if prefix.is_some() {
            str.push(prefix.unwrap());
        } else {
            match self.next_char() {
                Some(c) if is_symbol_initial(&c) => str.push(c),
                Some(c)                          => {
                    return self.unexpected_character(&c);
                },
                None                             => {
                    return self.unexpected_eof();
                },
            };
        }

        loop {
            match self.peek_char() {
                Some(c) if is_symbol_subsequent(&c) => {
                    self.next_char();
                    str.push(c)
                },
                _                                   => break,
            };
        }

        return Some(self.ctx().get_or_create_symbol(str));
    }

    /// Read a quoted form from input, e.g. `'(1 2 3)`.
    fn read_quoted(&mut self) -> Option<Value> {
        if self.expect_character('\'').is_err() {
            None
        }

        match self.next() {
            None      => self.unexpected_eof(),
            Some(val) => Some(list(self.ctx(), &mut [
                self.ctx().get_or_create_symbol("quote".to_string()),
                val
            ])),
        }
    }
}

impl<R: Reader> Iterator<Value> for Read<R> {
    fn next(&mut self) -> Option<Value> {
        if self.result.is_err() {
            return None;
        }

        self.trim();

        match self.peek_char() {
            None                             => None,
            Some('\'')                       => self.read_quoted(),
            Some('-')                        => {
                self.next_char();
                match self.peek_char() {
                    Some(c) if c.is_digit(10) => {
                        self.read_integer(true)
                    },
                    _                         => self.read_symbol(Some('-')),
                }
            },
            Some(c) if c.is_digit(10)        => self.read_integer(false),
            Some('#')                        => self.read_bool_or_char(),
            Some('"')                        => self.read_string(),
            Some('(')                        => {
                self.next_char();
                self.read_pair()
            },
            Some(c) if is_symbol_initial(&c) => self.read_symbol(None),
            Some(c)                          => self.unexpected_character(&c),
        }
    }
}

/// Create a `Read` instance from a byte vector.
pub fn read_from_bytes(bytes: Vec<u8>, ctx: *mut Context) -> Read<MemReader> {
    Read::new(MemReader::new(bytes), ctx)
}

/// Create a `Read` instance from a `String`.
pub fn read_from_string(string: String, ctx: *mut Context) -> Read<MemReader> {
    read_from_bytes(string.into_bytes(), ctx)
}

/// Create a `Read` instance from a `&str`.
pub fn read_from_str(str: &str, ctx: *mut Context) -> Read<MemReader> {
    read_from_string(str.to_string(), ctx)
}

#[test]
fn test_read_integers() {
    let input = "5 -5 789 -987";
    let mut ctx = Context::new();
    let results : Vec<Value> = read_from_str(input, &mut ctx).collect();
    assert_eq!(results, vec!(Value::new_integer(5),
                             Value::new_integer(-5),
                             Value::new_integer(789),
                             Value::new_integer(-987)))
}

#[test]
fn test_read_booleans() {
    let input = "#t #f";
    let mut ctx = Context::new();
    let results : Vec<Value> = read_from_str(input, &mut ctx).collect();
    assert_eq!(results, vec!(Value::new_boolean(true),
                             Value::new_boolean(false)))
}

#[test]
fn test_read_characters() {
    let input = "#\\a #\\0 #\\- #\\space #\\tab #\\newline #\\\n";
    let mut ctx = Context::new();
    let results : Vec<Value> = read_from_str(input, &mut ctx).collect();
    assert_eq!(results, vec!(Value::new_character('a'),
                             Value::new_character('0'),
                             Value::new_character('-'),
                             Value::new_character(' '),
                             Value::new_character('\t'),
                             Value::new_character('\n'),
                             Value::new_character('\n')));
}

#[test]
fn test_read_comments() {
    let input = "1 ;; this is a comment\n2";
    let mut ctx = Context::new();
    let results : Vec<Value> = read_from_str(input, &mut ctx).collect();

    assert_eq!(results.len(), 2);
    assert_eq!(results, vec!(Value::new_integer(1),
                             Value::new_integer(2)));
}

#[test]
fn test_read_pairs() {
    let input = "() (1 2 3) (1 (2) ((3)))";
    let mut ctx = Context::new();
    let results : Vec<Value> = read_from_str(input, &mut ctx).collect();
    assert_eq!(results.len(), 3);

    assert_eq!(results[0], Value::EmptyList);

    let v1 = &results[1];
    assert_eq!(v1.car(),
               Some(Value::new_integer(1)));
    assert_eq!(v1.cdr().expect("v1.cdr")
                 .car(),
               Some(Value::new_integer(2)));
    assert_eq!(v1.cdr().expect("v1.cdr")
                 .cdr().expect("v1.cdr.cdr")
                 .car(),
               Some(Value::new_integer(3)));
    assert_eq!(v1.cdr().expect("v1.cdr")
                 .cdr().expect("v1.cdr.cdr")
                 .cdr(),
               Some(Value::EmptyList));

    let v2 = &results[2];
    assert_eq!(v2.car(),
               Some(Value::new_integer(1)));
    assert_eq!(v2.cdr().expect("v2.cdr")
                 .car().expect("v2.cdr.car")
                 .car(),
               Some(Value::new_integer(2)));
    assert_eq!(v2.cdr().expect("v2.cdr")
                 .car().expect("v2.cdr.car")
                 .cdr(),
               Some(Value::EmptyList));
    assert_eq!(v2.cdr().expect("v2.cdr")
                 .cdr().expect("v2.cdr.cdr")
                 .car().expect("v2.cdr.cdr.car")
                 .car().expect("v2.cdr.cdr.car.car")
                 .car(),
               Some(Value::new_integer(3)));
    assert_eq!(v2.cdr().expect("v2.cdr")
                 .cdr().expect("v2.cdr.cdr")
                 .car().expect("v2.cdr.cdr.car")
                 .car().expect("v2.cdr.cdr.car.car")
                 .cdr(),
               Some(Value::EmptyList));
    assert_eq!(v2.cdr().expect("v2.cdr")
                 .cdr().expect("v2.cdr.cdr")
                 .car().expect("v2.cdr.cdr.car")
                 .cdr(),
               Some(Value::EmptyList));
    assert_eq!(v2.cdr().expect("v2.cdr")
                 .cdr().expect("v2.cdr.cdr")
                 .cdr(),
               Some(Value::EmptyList));
}

#[test]
fn test_read_improper_lists() {
    let input = "(1 . 2) (3 . ()) (4 . (5 . 6)) (1 2 . 3)";
    let mut ctx = Context::new();
    let results : Vec<Value> = read_from_str(input, &mut ctx).collect();
    assert_eq!(results.len(), 4);

    let v0 = &results[0];
    assert_eq!(v0.car(), Some(Value::new_integer(1)));
    assert_eq!(v0.cdr(), Some(Value::new_integer(2)));

    let v1 = &results[1];
    assert_eq!(v1.car(), Some(Value::new_integer(3)));
    assert_eq!(v1.cdr(), Some(Value::EmptyList));

    let v2 = &results[2];
    assert_eq!(v2.car(), Some(Value::new_integer(4)));
    assert_eq!(v2.cdr().expect("v2.cdr")
                 .car(),
               Some(Value::new_integer(5)));
    assert_eq!(v2.cdr().expect("v2.cdr")
                 .cdr(),
               Some(Value::new_integer(6)));

    let v3 = &results[3];
    assert_eq!(v3.car(), Some(Value::new_integer(1)));
    assert_eq!(v3.cdr().expect("v3.cdr")
                 .car(),
              Some(Value::new_integer(2)));
    assert_eq!(v3.cdr().expect("v3.cdr")
                 .cdr(),
              Some(Value::new_integer(3)));
}

#[test]
fn test_read_string() {
    let input = "\"\" \"hello\" \"\\\"\"";
    let mut ctx = Context::new();
    let results : Vec<Value> = read_from_str(input, &mut ctx).collect();
    assert_eq!(results.len(), 3);

    match results[0] {
        Value::String(str) => assert_eq!(str.deref().deref(), "".to_string()),
        _                  => assert!(false),
    }

    match results[1] {
        Value::String(str) => assert_eq!(str.deref().deref(), "hello".to_string()),
        _                  => assert!(false),
    }

    match results[2] {
        Value::String(str) => assert_eq!(str.deref().deref(), "\"".to_string()),
        _                  => assert!(false),
    }
}

#[test]
fn test_read_symbols() {
    let input = "foo + - * ? !";
    let mut ctx = Context::new();
    let results : Vec<Value> = read_from_str(input, &mut ctx).collect();
    assert_eq!(results.len(), 6);

    match results[0] {
        Value::Symbol(str) => assert_eq!(str.deref().deref(), "foo".to_string()),
        _                  => assert!(false),
    }

    match results[1] {
        Value::Symbol(str) => assert_eq!(str.deref().deref(), "+".to_string()),
        _                  => assert!(false),
    }

    match results[2] {
        Value::Symbol(str) => assert_eq!(str.deref().deref(), "-".to_string()),
        _                  => assert!(false),
    }

    match results[3] {
        Value::Symbol(str) => assert_eq!(str.deref().deref(), "*".to_string()),
        _                  => assert!(false),
    }

    match results[4] {
        Value::Symbol(str) => assert_eq!(str.deref().deref(), "?".to_string()),
        _                  => assert!(false),
    }

    match results[5] {
        Value::Symbol(str) => assert_eq!(str.deref().deref(), "!".to_string()),
        _                  => assert!(false),
    }
}

#[test]
fn test_read_same_symbol() {
    let input = "foo foo";
    let mut ctx = Context::new();
    let results : Vec<Value> = read_from_str(input, &mut ctx).collect();
    assert_eq!(results.len(), 2);

    // We should only allocate one StringPtr and share it between both parses of
    // the same symbol.
    assert_eq!(results[0], results[1]);
}

#[test]
fn test_read_quoted() {
    let input = "'foo";
    let mut ctx = Context::new();
    let results : Vec<Value> = read_from_str(input, &mut ctx).collect();
    assert_eq!(results.len(), 1);

    match results[0].car() {
        Some(Value::Symbol(str)) => assert_eq!(str.deref().deref(),
                                               "quote".to_string()),
        _                        => assert!(false),
    }
    match results[0].cdr().expect("results[0].cdr").car() {
        Some(Value::Symbol(str)) => assert_eq!(str.deref().deref(),
                                               "foo".to_string()),
        _                        => assert!(false),
    }
}