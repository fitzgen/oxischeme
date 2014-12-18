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
use value::{Value, TRUE, FALSE};

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
    c.is_whitespace() || is_comment(c)
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

/// `Read` iteratively parses values from the input `Reader`.
pub struct Read<R> {
    chars: RefCell<Peekable<char, CharReader<R>>>,
    result: Result<(), String>
}

impl<'a, R: Reader> Read<R> {
    /// Create a new `Read` instance from the given `Reader` input source.
    pub fn new(reader: R) -> Read<R> {
        Read {
            chars: RefCell::new(CharReader::new(reader).peekable()),
            result: Ok(())
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
        self.result = Err(msg);
        None
    }

    /// Report an unexpected character.
    fn unexpected_character(&mut self, c: &char) -> Option<Value> {
        self.report_failure(format_args!(format, "Unexpected character: {}", c))
    }

    /// Report a bad character literal, e.g. `#\bad`.
    fn bad_character_literal(&mut self) -> Option<Value> {
        self.report_failure("Bad character value".to_string())
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

    /// Read an integer.
    fn read_integer(&mut self) -> Option<Value> {
        let sign : i64 = match self.peek_char() {
            None      => return None,
            Some('-') => -1,
            _         => 1,
        };

        if sign == -1 {
            self.next_char();
        }

        let mut abs_value : i64 = match self.next_char() {
            None    => panic!("Unexpected EOF!"),
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
}

impl<R: Reader> Iterator<Value> for Read<R> {
    fn next(&mut self) -> Option<Value> {
        self.trim();

        match self.peek_char() {
            None                                  => None,
            Some(c) if c.is_digit(10) || c == '-' => self.read_integer(),
            Some('#')                             => {
                self.next_char();
                // Deterimine if this is a boolean or a character.
                match self.next_char() {
                    None       => None,
                    Some('t')  => Some(TRUE),
                    Some('f')  => Some(FALSE),
                    Some('\\') => self.read_character(),
                    Some(c)    => self.unexpected_character(&c),
                }
            },
            Some(c)                               => self.unexpected_character(&c),
        }
    }
}

/// Create a `Read` instance from a byte vector.
pub fn read_from_bytes(bytes: Vec<u8>) -> Read<MemReader> {
    Read::new(MemReader::new(bytes))
}

/// Create a `Read` instance from a `String`.
pub fn read_from_string(string: String) -> Read<MemReader> {
    read_from_bytes(string.into_bytes())
}

/// Create a `Read` instance from a `&str`.
pub fn read_from_str(str: &str) -> Read<MemReader> {
    read_from_string(str.to_string())
}

#[test]
fn test_read_integers() {
    let input = "5 -5 789 -987";
    let results : Vec<Value> = read_from_str(input).collect();
    assert_eq!(results, vec!(Value::new_integer(5),
                             Value::new_integer(-5),
                             Value::new_integer(789),
                             Value::new_integer(-987)))
}

#[test]
fn test_read_booleans() {
    let input = "#t #f";
    let results : Vec<Value> = read_from_str(input).collect();
    assert_eq!(results, vec!(Value::new_boolean(true),
                             Value::new_boolean(false)))
}

#[test]
fn test_read_characters() {
    let input = "#\\a #\\0 #\\- #\\space #\\tab #\\newline #\\\n";
    let results : Vec<Value> = read_from_str(input).collect();
    assert_eq!(results, vec!(Value::new_character('a'),
                             Value::new_character('0'),
                             Value::new_character('-'),
                             Value::new_character(' '),
                             Value::new_character('\t'),
                             Value::new_character('\n'),
                             Value::new_character('\n')));
}