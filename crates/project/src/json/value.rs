use std::fmt::Write;

use crate::ProjectError;

/// A parsed document, kept generic so migrations can rewrite old layouts before
/// they are read into the current types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Null,
    Bool(bool),
    /// The model holds only integers, so fractions are never written or accepted.
    Integer(i128),
    String(String),
    Array(Vec<Value>),
    /// Keeps field order so saved files diff cleanly between saves.
    Object(Vec<(String, Value)>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Object(fields) => fields.iter().find(|(name, _)| name == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        match self {
            Self::Object(fields) => fields
                .iter_mut()
                .find(|(name, _)| name == key)
                .map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        write_value(self, 0, &mut out);
        out.push('\n');
        out
    }

    pub fn parse(text: &str) -> Result<Self, ProjectError> {
        // Notepad and other Windows editors start a UTF-8 file they save with a BOM.
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut parser = Parser {
            bytes: text.as_bytes(),
            at: 0,
        };
        let value = parser.value(0)?;
        parser.skip_space();
        if parser.at != parser.bytes.len() {
            return Err(parser.error("unexpected text after the document"));
        }
        Ok(value)
    }
}

const INDENT: &str = "  ";
/// Deep enough for any project, shallow enough that hostile files can't overflow the stack.
const MAX_DEPTH: usize = 128;

fn write_value(value: &Value, depth: usize, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        Value::Integer(number) => {
            let _ = write!(out, "{number}");
        }
        Value::String(text) => write_string(text, out),
        Value::Array(items) if items.is_empty() => out.push_str("[]"),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                newline(depth + 1, out);
                write_value(item, depth + 1, out);
            }
            newline(depth, out);
            out.push(']');
        }
        Value::Object(fields) if fields.is_empty() => out.push_str("{}"),
        Value::Object(fields) => {
            out.push('{');
            for (index, (name, item)) in fields.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                newline(depth + 1, out);
                write_string(name, out);
                out.push_str(": ");
                write_value(item, depth + 1, out);
            }
            newline(depth, out);
            out.push('}');
        }
    }
}

fn newline(depth: usize, out: &mut String) {
    out.push('\n');
    for _ in 0..depth {
        out.push_str(INDENT);
    }
}

fn write_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn error(&self, message: &str) -> ProjectError {
        let line = self.bytes[..self.at.min(self.bytes.len())]
            .iter()
            .filter(|&&b| b == b'\n')
            .count()
            + 1;
        ProjectError::Syntax {
            line,
            message: message.to_owned(),
        }
    }

    fn skip_space(&mut self) {
        while self
            .bytes
            .get(self.at)
            .is_some_and(|b| b.is_ascii_whitespace())
        {
            self.at += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_space();
        self.bytes.get(self.at).copied()
    }

    fn expect(&mut self, byte: u8) -> Result<(), ProjectError> {
        if self.peek() == Some(byte) {
            self.at += 1;
            Ok(())
        } else {
            Err(self.error(&format!("expected '{}'", byte as char)))
        }
    }

    fn keyword(&mut self, word: &str, value: Value) -> Result<Value, ProjectError> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(self.error("unknown keyword"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, ProjectError> {
        if depth > MAX_DEPTH {
            return Err(self.error("nested too deeply"));
        }
        match self.peek() {
            Some(b'n') => self.keyword("null", Value::Null),
            Some(b't') => self.keyword("true", Value::Bool(true)),
            Some(b'f') => self.keyword("false", Value::Bool(false)),
            Some(b'"') => self.string().map(Value::String),
            Some(b'[') => self.array(depth),
            Some(b'{') => self.object(depth),
            Some(b'-' | b'0'..=b'9') => self.integer(),
            Some(_) => Err(self.error("unexpected character")),
            None => Err(self.error("unexpected end of file")),
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, ProjectError> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.error("expected ',' or ']'")),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, ProjectError> {
        self.expect(b'{')?;
        let mut fields = Vec::new();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(Value::Object(fields));
        }
        loop {
            if self.peek() != Some(b'"') {
                return Err(self.error("expected a field name"));
            }
            let name = self.string()?;
            self.expect(b':')?;
            fields.push((name, self.value(depth + 1)?));
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(Value::Object(fields));
                }
                _ => return Err(self.error("expected ',' or '}'")),
            }
        }
    }

    fn integer(&mut self) -> Result<Value, ProjectError> {
        let start = self.at;
        if self.bytes.get(self.at) == Some(&b'-') {
            self.at += 1;
        }
        while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
            self.at += 1;
        }
        if matches!(self.bytes.get(self.at), Some(b'.' | b'e' | b'E')) {
            return Err(self.error("fractional numbers are not used in projects"));
        }
        std::str::from_utf8(&self.bytes[start..self.at])
            .ok()
            .and_then(|digits| digits.parse().ok())
            .map(Value::Integer)
            .ok_or_else(|| self.error("invalid number"))
    }

    fn string(&mut self) -> Result<String, ProjectError> {
        self.expect(b'"')?;
        let mut out = Vec::new();
        loop {
            let Some(&byte) = self.bytes.get(self.at) else {
                return Err(self.error("unterminated string"));
            };
            self.at += 1;
            match byte {
                b'"' => break,
                b'\\' => self.escape(&mut out)?,
                byte => out.push(byte),
            }
        }
        String::from_utf8(out).map_err(|_| self.error("invalid UTF-8"))
    }

    fn escape(&mut self, out: &mut Vec<u8>) -> Result<(), ProjectError> {
        let Some(&byte) = self.bytes.get(self.at) else {
            return Err(self.error("unterminated string"));
        };
        self.at += 1;
        let c = match byte {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'u' => self.unicode_escape()?,
            _ => return Err(self.error("invalid escape")),
        };
        let mut buffer = [0; 4];
        out.extend_from_slice(c.encode_utf8(&mut buffer).as_bytes());
        Ok(())
    }

    fn unicode_escape(&mut self) -> Result<char, ProjectError> {
        let high = self.hex4()?;
        let code = if (0xD800..0xDC00).contains(&high) {
            // Characters outside the basic plane arrive as a surrogate pair.
            if !self.bytes[self.at..].starts_with(b"\\u") {
                return Err(self.error("unpaired surrogate"));
            }
            self.at += 2;
            let low = self.hex4()?;
            if !(0xDC00..0xE000).contains(&low) {
                return Err(self.error("unpaired surrogate"));
            }
            0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00)
        } else {
            high
        };
        char::from_u32(code).ok_or_else(|| self.error("invalid character escape"))
    }

    fn hex4(&mut self) -> Result<u32, ProjectError> {
        let digits = self
            .bytes
            .get(self.at..self.at + 4)
            .and_then(|digits| std::str::from_utf8(digits).ok())
            .and_then(|digits| u32::from_str_radix(digits, 16).ok())
            .ok_or_else(|| self.error("invalid \\u escape"))?;
        self.at += 4;
        Ok(digits)
    }
}
