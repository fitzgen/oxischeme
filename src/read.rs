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

use std::io::{BufferedReader, IoError, IoErrorKind, MemReader};
use std::iter::{Peekable};
use value::{Value, ValueKind, TRUE, FALSE};

/// `CharReader` reads characters one at a time from the given input `Reader`.
struct CharReader<R> {
    reader: BufferedReader<R>
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

/// Return true if the character is a delimiter between tokens.
fn is_delimiter(c: &char) -> bool {
    c.is_whitespace() || is_comment(c)
}

/// `Read` iteratively parses values from the input `Reader`.
///
/// TODO FITZGEN: rather than panicking on bad input, save an error message and
/// return None or something.
pub struct Read<R> {
    chars: Peekable<char, CharReader<R>>
}

impl<R: Reader> Read<R> {
    /// Create a new `Read` instance from the given `Reader` input source.
    pub fn new(reader: R) -> Read<R> {
        Read {
            chars: CharReader::new(reader).peekable()
        }
    }

    /// Skip to after the next newline character.
    fn skip_line(&mut self) {
        loop {
            match self.chars.peek() {
                None                  => return,
                Some(c) if *c == '\n' => return,
                _                     => { },
            }
            self.chars.next();
        }
    }

    /// Trim initial whitespace and skip comments.
    fn trim(&mut self) {
        loop {
            let skip_line = match self.chars.peek() {
                Some(c) if c.is_whitespace() => false,
                Some(c) if is_comment(c)     => true,
                _                            => return,
            };

            if skip_line {
                self.skip_line();
            } else {
                self.chars.next();
            }
        }
    }

    /// Read a boolean.
    fn read_boolean(&mut self) -> Option<Value> {
        match self.chars.next() {
            Some('#') => { },
            Some(c)   => panic!("Unexpected character: {}", c),
            None      => panic!("Unexpected EOF"),
        }

        match self.chars.next() {
            Some('t') => Some(TRUE),
            Some('f') => Some(FALSE),
            Some(c)   => panic!("Unexpected character: {}", c),
            None      => panic!("Unexpected EOF"),
        }
    }

    /// Read an integer.
    fn read_integer(&mut self) -> Option<Value> {
        let sign : i64 = match self.chars.peek() {
            None                 => return None,
            Some(c) if *c == '-' => -1,
            _                    => 1,
        };

        if sign == -1 {
            self.chars.next();
        }

        let mut abs_value : i64 = match self.chars.next() {
            None    => panic!("Unexpected EOF!"),
            Some(c) => match c.to_digit(10) {
                None    => panic!("Unexpected character: {}", c),
                Some(d) => d as i64
            }
        };

        loop {
            match self.chars.peek() {
                None                       => break,
                Some(c) if is_delimiter(c) => break,
                Some(c)                    => match c.to_digit(10) {
                    None    => panic!("Unexpected character: {}", c),
                    Some(d) => abs_value = (abs_value * 10) + (d as i64),
                }
            }
            self.chars.next();
        }

        Some(Value::new_integer(abs_value * sign))
    }
}

impl<R: Reader> Iterator<Value> for Read<R> {
    fn next(&mut self) -> Option<Value> {
        self.trim();

        let kind = match self.chars.peek() {
            None                                   => return None,
            Some(c) if *c == '#'                   => ValueKind::Boolean,
            Some(c) if c.is_digit(10) || *c == '-' => ValueKind::Integer,
            Some(c)                                => panic!("Unexpected character: {}", c),
        };

        match kind {
            ValueKind::Boolean => self.read_boolean(),
            ValueKind::Integer => self.read_integer(),
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