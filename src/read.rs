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

use std::io;
use value;

/// `Read` iteratively parses s-expressions from the input `Reader`.
///
/// TODO FITZGEN: build this on top of a Peekable iterator<(line, column, char)>
/// instead of BufferedReader directly.
///
/// TODO FITZGEN: rather than panicking on bad input, save an error message and
/// return None or something.
pub struct Read<R> {
    reader: io::BufferedReader<R>
}

enum CharType {
    Character(char),
    WhiteSpace,
    Comment,
    EOF
}

fn is_eof(err : &io::IoError) -> bool {
    err.kind == io::IoErrorKind::EndOfFile
}

impl<R: Reader> Read<R> {
    /// Create a new `Read` instance from the given `Reader` input source.
    pub fn new(reader: R) -> Read<R> {
        Read { reader: io::BufferedReader::new(reader) }
    }

    // Read a character in from our BufferedReader and translate it into the
    // character types we care about.
    fn read_char(&mut self) -> CharType {
        match self.reader.read_char() {
            Ok(c) if c.is_whitespace() => CharType::WhiteSpace,
            Ok(c) if c == ';'          => CharType::Comment,
            Ok(c)                      => CharType::Character(c),
            Err(ref e) if is_eof(e)    => CharType::EOF,
            Err(e)                     => panic!("IO ERROR! {}", e),
        }
    }

    // Skip to the end of the current line of input.
    fn skip_line(&mut self) {
        match self.reader.read_line() {
            Ok(_)                   => return,
            Err(ref e) if is_eof(e) => return,
            Err(e)                  => panic!("IO ERROR! {}", e),
        }
    }

    // Trim initial whitespace and skip comments. Return the first character of
    // the next value found, or None if we hit EOF.
    fn trim(&mut self) -> Option<char> {
        loop {
            match self.read_char() {
                CharType::WhiteSpace   => continue,
                CharType::Comment      => self.skip_line(),
                CharType::EOF          => return None,
                CharType::Character(c) => return Some(c),
            }
        }
    }
}

impl<R: Reader> Iterator<value::Value> for Read<R> {
    fn next(&mut self) -> Option<value::Value> {
        let mut sign = 1;
        let mut abs_value : i64 = 0;

        match self.trim() {
            None      => panic!("Unexpected EOF"),
            Some('-') => sign = -1,
            Some(c)   => match c.to_digit(10) {
                None    => panic!("Unexpected character: {}", c),
                Some(d) => abs_value = d as i64,
            },
        }

        loop {
            match self.read_char() {
                CharType::WhiteSpace | CharType::EOF => break,
                CharType::Comment                    => {
                    self.skip_line();
                    break;
                },
                CharType::Character(c)               => match c.to_digit(10) {
                    None    => panic!("Unexpected character: {}", c),
                    Some(d) => abs_value = (abs_value * 10) + (d as i64)
                },
            }
        }

        Some(value::Value::new_integer(abs_value * sign))
    }
}