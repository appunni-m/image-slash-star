use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fmt::Write as _;

const MAX_DEPTH: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Self>),
    Object(BTreeMap<String, Self>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Number(value) => value.parse().ok(),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Self]> {
        match self {
            Self::Array(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&BTreeMap<String, Self>> {
        match self {
            Self::Object(value) => Some(value),
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => formatter.write_str("null"),
            Self::Bool(value) => value.fmt(formatter),
            Self::Number(value) => formatter.write_str(value),
            Self::String(value) => write_json_string(formatter, value),
            Self::Array(values) => {
                formatter.write_str("[")?;
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        formatter.write_str(",")?;
                    }
                    value.fmt(formatter)?;
                }
                formatter.write_str("]")
            }
            Self::Object(values) => {
                formatter.write_str("{")?;
                for (index, (key, value)) in values.iter().enumerate() {
                    if index != 0 {
                        formatter.write_str(",")?;
                    }
                    write_json_string(formatter, key)?;
                    formatter.write_str(":")?;
                    value.fmt(formatter)?;
                }
                formatter.write_str("}")
            }
        }
    }
}

fn write_json_string(formatter: &mut fmt::Formatter<'_>, value: &str) -> fmt::Result {
    formatter.write_str("\"")?;
    for character in value.chars() {
        match character {
            '"' => formatter.write_str("\\\"")?,
            '\\' => formatter.write_str("\\\\")?,
            '\u{0008}' => formatter.write_str("\\b")?,
            '\u{000c}' => formatter.write_str("\\f")?,
            '\n' => formatter.write_str("\\n")?,
            '\r' => formatter.write_str("\\r")?,
            '\t' => formatter.write_str("\\t")?,
            character if character <= '\u{001f}' => {
                write!(formatter, "\\u{:04x}", u32::from(character))?;
            }
            character => formatter.write_char(character)?,
        }
    }
    formatter.write_str("\"")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    message: String,
}

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn at(message: impl fmt::Display, offset: usize) -> Self {
        Self::new(format!("{message} at byte {offset}"))
    }

    fn field(self, field: &str) -> Self {
        Self::new(format!("field `{field}`: {}", self.message))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

pub trait FromJson: Sized {
    fn from_json(value: Value) -> Result<Self, Error>;
}

pub fn from_str<T: FromJson>(input: &str) -> Result<T, Error> {
    let mut parser = Parser { input, offset: 0 };
    let value = parser.value(0)?;
    parser.whitespace();
    if parser.offset != input.len() {
        return Err(Error::at("trailing JSON input", parser.offset));
    }
    T::from_json(value)
}

impl FromJson for Value {
    fn from_json(value: Value) -> Result<Self, Error> {
        Ok(value)
    }
}

impl FromJson for String {
    fn from_json(value: Value) -> Result<Self, Error> {
        match value {
            Value::String(value) => Ok(value),
            _ => Err(Error::new("expected a JSON string")),
        }
    }
}

impl FromJson for bool {
    fn from_json(value: Value) -> Result<Self, Error> {
        match value {
            Value::Bool(value) => Ok(value),
            _ => Err(Error::new("expected a JSON boolean")),
        }
    }
}

impl FromJson for u32 {
    fn from_json(value: Value) -> Result<Self, Error> {
        integer(&value)?.parse().map_err(|_| {
            Error::new(format!(
                "JSON number `{}` is outside the u32 range",
                integer(&value).unwrap_or_default()
            ))
        })
    }
}

impl FromJson for u16 {
    fn from_json(value: Value) -> Result<Self, Error> {
        integer(&value)?.parse().map_err(|_| {
            Error::new(format!(
                "JSON number `{}` is outside the u16 range",
                integer(&value).unwrap_or_default()
            ))
        })
    }
}

impl FromJson for u64 {
    fn from_json(value: Value) -> Result<Self, Error> {
        integer(&value)?.parse().map_err(|_| {
            Error::new(format!(
                "JSON number `{}` is outside the u64 range",
                integer(&value).unwrap_or_default()
            ))
        })
    }
}

impl FromJson for usize {
    fn from_json(value: Value) -> Result<Self, Error> {
        integer(&value)?.parse().map_err(|_| {
            Error::new(format!(
                "JSON number `{}` is outside the usize range",
                integer(&value).unwrap_or_default()
            ))
        })
    }
}

impl FromJson for i32 {
    fn from_json(value: Value) -> Result<Self, Error> {
        integer(&value)?.parse().map_err(|_| {
            Error::new(format!(
                "JSON number `{}` is outside the i32 range",
                integer(&value).unwrap_or_default()
            ))
        })
    }
}

fn integer(value: &Value) -> Result<&str, Error> {
    match value {
        Value::Number(value)
            if !value.contains(['.', 'e', 'E']) && value != "-" && !value.starts_with('+') =>
        {
            Ok(value)
        }
        _ => Err(Error::new("expected an integral JSON number")),
    }
}

impl<T: FromJson> FromJson for Option<T> {
    fn from_json(value: Value) -> Result<Self, Error> {
        match value {
            Value::Null => Ok(None),
            value => T::from_json(value).map(Some),
        }
    }
}

impl<T: FromJson> FromJson for Vec<T> {
    fn from_json(value: Value) -> Result<Self, Error> {
        let Value::Array(values) = value else {
            return Err(Error::new("expected a JSON array"));
        };
        values
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                T::from_json(value)
                    .map_err(|error| Error::new(format!("array item {index}: {error}")))
            })
            .collect()
    }
}

impl<T: FromJson, const LENGTH: usize> FromJson for [T; LENGTH] {
    fn from_json(value: Value) -> Result<Self, Error> {
        let values = Vec::<T>::from_json(value)?;
        values.try_into().map_err(|values: Vec<T>| {
            Error::new(format!(
                "expected an array of length {LENGTH}, found {}",
                values.len()
            ))
        })
    }
}

impl<T: FromJson> FromJson for HashMap<String, T> {
    fn from_json(value: Value) -> Result<Self, Error> {
        let Value::Object(values) = value else {
            return Err(Error::new("expected a JSON object"));
        };
        values
            .into_iter()
            .map(|(key, value)| {
                T::from_json(value)
                    .map(|value| (key.clone(), value))
                    .map_err(|error| error.field(&key))
            })
            .collect()
    }
}

pub struct Object {
    values: BTreeMap<String, Value>,
}

impl Object {
    pub fn new(value: Value) -> Result<Self, Error> {
        let Value::Object(values) = value else {
            return Err(Error::new("expected a JSON object"));
        };
        Ok(Self { values })
    }

    pub fn take<T: FromJson>(&mut self, name: &str) -> Result<T, Error> {
        T::from_json(self.values.remove(name).unwrap_or(Value::Null))
            .map_err(|error| error.field(name))
    }

    pub fn take_or_default<T: FromJson + Default>(&mut self, name: &str) -> Result<T, Error> {
        match self.values.remove(name) {
            Some(value) => T::from_json(value).map_err(|error| error.field(name)),
            None => Ok(T::default()),
        }
    }
}

struct Parser<'input> {
    input: &'input str,
    offset: usize,
}

impl Parser<'_> {
    fn value(&mut self, depth: usize) -> Result<Value, Error> {
        if depth >= MAX_DEPTH {
            return Err(Error::at("JSON nesting exceeds limit", self.offset));
        }
        self.whitespace();
        match self.byte() {
            Some(b'n') => self.keyword(b"null", Value::Null),
            Some(b't') => self.keyword(b"true", Value::Bool(true)),
            Some(b'f') => self.keyword(b"false", Value::Bool(false)),
            Some(b'"') => self.string().map(Value::String),
            Some(b'[') => self.array(depth.saturating_add(1)),
            Some(b'{') => self.object(depth.saturating_add(1)),
            Some(b'-' | b'0'..=b'9') => self.number().map(Value::Number),
            Some(_) => Err(Error::at("invalid JSON value", self.offset)),
            None => Err(Error::at("unexpected end of JSON input", self.offset)),
        }
    }

    fn keyword(&mut self, keyword: &[u8], value: Value) -> Result<Value, Error> {
        let end = self
            .offset
            .checked_add(keyword.len())
            .ok_or_else(|| Error::at("JSON offset overflow", self.offset))?;
        if self.input.as_bytes().get(self.offset..end) != Some(keyword) {
            return Err(Error::at("invalid JSON literal", self.offset));
        }
        self.offset = end;
        Ok(value)
    }

    fn string(&mut self) -> Result<String, Error> {
        self.expect(b'"')?;
        let mut output = String::new();
        loop {
            let Some(byte) = self.byte() else {
                return Err(Error::at("unterminated JSON string", self.offset));
            };
            match byte {
                b'"' => {
                    self.offset = self.offset.saturating_add(1);
                    return Ok(output);
                }
                b'\\' => {
                    self.offset = self.offset.saturating_add(1);
                    self.escape(&mut output)?;
                }
                0..=0x1f => {
                    return Err(Error::at(
                        "unescaped control character in JSON string",
                        self.offset,
                    ));
                }
                _ => {
                    let Some(character) = self.input[self.offset..].chars().next() else {
                        return Err(Error::at("invalid UTF-8 in JSON string", self.offset));
                    };
                    output.push(character);
                    self.offset = self.offset.saturating_add(character.len_utf8());
                }
            }
        }
    }

    fn escape(&mut self, output: &mut String) -> Result<(), Error> {
        let Some(byte) = self.byte() else {
            return Err(Error::at("truncated JSON escape", self.offset));
        };
        self.offset = self.offset.saturating_add(1);
        match byte {
            b'"' => output.push('"'),
            b'\\' => output.push('\\'),
            b'/' => output.push('/'),
            b'b' => output.push('\u{0008}'),
            b'f' => output.push('\u{000c}'),
            b'n' => output.push('\n'),
            b'r' => output.push('\r'),
            b't' => output.push('\t'),
            b'u' => output.push(self.unicode_escape()?),
            _ => {
                return Err(Error::at(
                    "invalid JSON escape",
                    self.offset.saturating_sub(1),
                ));
            }
        }
        Ok(())
    }

    fn unicode_escape(&mut self) -> Result<char, Error> {
        let first = self.hex_quad()?;
        let scalar = if (0xd800..=0xdbff).contains(&first) {
            self.expect(b'\\')?;
            self.expect(b'u')?;
            let second = self.hex_quad()?;
            if !(0xdc00..=0xdfff).contains(&second) {
                return Err(Error::at("invalid low surrogate", self.offset));
            }
            0x1_0000_u32
                .wrapping_add(u32::from(first).wrapping_sub(0xd800).wrapping_shl(10))
                .wrapping_add(u32::from(second).wrapping_sub(0xdc00))
        } else {
            if (0xdc00..=0xdfff).contains(&first) {
                return Err(Error::at("unexpected low surrogate", self.offset));
            }
            u32::from(first)
        };
        char::from_u32(scalar).ok_or_else(|| Error::at("invalid Unicode scalar", self.offset))
    }

    fn hex_quad(&mut self) -> Result<u16, Error> {
        let end = self
            .offset
            .checked_add(4)
            .ok_or_else(|| Error::at("JSON offset overflow", self.offset))?;
        let Some(digits) = self.input.get(self.offset..end) else {
            return Err(Error::at("truncated Unicode escape", self.offset));
        };
        if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(Error::at("invalid Unicode escape", self.offset));
        }
        self.offset = end;
        u16::from_str_radix(digits, 16)
            .map_err(|_| Error::at("invalid Unicode escape", self.offset))
    }

    fn array(&mut self, depth: usize) -> Result<Value, Error> {
        self.expect(b'[')?;
        self.whitespace();
        let mut values = Vec::new();
        if self.consume(b']') {
            return Ok(Value::Array(values));
        }
        loop {
            values.push(self.value(depth)?);
            self.whitespace();
            if self.consume(b']') {
                return Ok(Value::Array(values));
            }
            self.expect(b',')?;
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, Error> {
        self.expect(b'{')?;
        self.whitespace();
        let mut values = BTreeMap::new();
        if self.consume(b'}') {
            return Ok(Value::Object(values));
        }
        loop {
            self.whitespace();
            if self.byte() != Some(b'"') {
                return Err(Error::at("JSON object key must be a string", self.offset));
            }
            let key = self.string()?;
            self.whitespace();
            self.expect(b':')?;
            let value = self.value(depth)?;
            values.insert(key, value);
            self.whitespace();
            if self.consume(b'}') {
                return Ok(Value::Object(values));
            }
            self.expect(b',')?;
        }
    }

    fn number(&mut self) -> Result<String, Error> {
        let start = self.offset;
        self.consume(b'-');
        match self.byte() {
            Some(b'0') => {
                self.offset = self.offset.saturating_add(1);
                if self.byte().is_some_and(|byte| byte.is_ascii_digit()) {
                    return Err(Error::at("leading zero in JSON number", self.offset));
                }
            }
            Some(b'1'..=b'9') => self.digits(),
            _ => return Err(Error::at("invalid JSON number", self.offset)),
        }
        if self.consume(b'.') {
            if !self.byte().is_some_and(|byte| byte.is_ascii_digit()) {
                return Err(Error::at("empty JSON fraction", self.offset));
            }
            self.digits();
        }
        if self.byte().is_some_and(|byte| matches!(byte, b'e' | b'E')) {
            self.offset = self.offset.saturating_add(1);
            if self.byte().is_some_and(|byte| matches!(byte, b'+' | b'-')) {
                self.offset = self.offset.saturating_add(1);
            }
            if !self.byte().is_some_and(|byte| byte.is_ascii_digit()) {
                return Err(Error::at("empty JSON exponent", self.offset));
            }
            self.digits();
        }
        self.input
            .get(start..self.offset)
            .map(str::to_owned)
            .ok_or_else(|| Error::at("invalid JSON number range", start))
    }

    fn digits(&mut self) {
        while self.byte().is_some_and(|byte| byte.is_ascii_digit()) {
            self.offset = self.offset.saturating_add(1);
        }
    }

    fn whitespace(&mut self) {
        while self
            .byte()
            .is_some_and(|byte| matches!(byte, b' ' | b'\n' | b'\r' | b'\t'))
        {
            self.offset = self.offset.saturating_add(1);
        }
    }

    fn expect(&mut self, expected: u8) -> Result<(), Error> {
        if self.consume(expected) {
            Ok(())
        } else {
            Err(Error::at(
                format!("expected JSON byte 0x{expected:02x}"),
                self.offset,
            ))
        }
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.byte() == Some(expected) {
            self.offset = self.offset.saturating_add(1);
            true
        } else {
            false
        }
    }

    fn byte(&self) -> Option<u8> {
        self.input.as_bytes().get(self.offset).copied()
    }
}
