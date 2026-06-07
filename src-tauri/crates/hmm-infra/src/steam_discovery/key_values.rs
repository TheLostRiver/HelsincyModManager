use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyValueNode {
    Text(String),
    Object(BTreeMap<String, KeyValueNode>),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum KeyValuesError {
    #[error("unexpected end of input")]
    UnexpectedEnd,
    #[error("expected quoted string")]
    ExpectedQuotedString,
    #[error("expected object")]
    ExpectedObject,
    #[error("unexpected token: {0}")]
    UnexpectedToken(String),
}

pub fn parse_key_values(input: &str) -> Result<KeyValueNode, KeyValuesError> {
    KeyValuesParser::new(input).parse()
}

struct KeyValuesParser<'a> {
    input: &'a str,
    position: usize,
}

impl<'a> KeyValuesParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, position: 0 }
    }

    fn parse(mut self) -> Result<KeyValueNode, KeyValuesError> {
        let object = self.parse_pairs_until(None)?;
        self.skip_whitespace();

        if self.is_at_end() {
            Ok(KeyValueNode::Object(object))
        } else {
            Err(KeyValuesError::UnexpectedToken(
                self.remaining_token().to_owned(),
            ))
        }
    }

    fn parse_pairs_until(
        &mut self,
        terminator: Option<char>,
    ) -> Result<BTreeMap<String, KeyValueNode>, KeyValuesError> {
        let mut object = BTreeMap::new();

        loop {
            self.skip_whitespace();

            if self.is_at_end() {
                return if terminator.is_some() {
                    Err(KeyValuesError::UnexpectedEnd)
                } else {
                    Ok(object)
                };
            }

            if let Some(expected) = terminator {
                if self.peek_char() == Some(expected) {
                    self.advance_char();
                    return Ok(object);
                }
            }

            let key = self.parse_quoted_string()?;
            self.skip_whitespace();
            let value = self.parse_value()?;
            object.insert(key, value);
        }
    }

    fn parse_value(&mut self) -> Result<KeyValueNode, KeyValuesError> {
        match self.peek_char() {
            Some('"') => self.parse_quoted_string().map(KeyValueNode::Text),
            Some('{') => {
                self.advance_char();
                self.parse_pairs_until(Some('}')).map(KeyValueNode::Object)
            }
            Some('}') => Err(KeyValuesError::UnexpectedToken("}".to_owned())),
            Some(_) => Err(KeyValuesError::ExpectedObject),
            None => Err(KeyValuesError::UnexpectedEnd),
        }
    }

    fn parse_quoted_string(&mut self) -> Result<String, KeyValuesError> {
        if self.peek_char() != Some('"') {
            return Err(KeyValuesError::ExpectedQuotedString);
        }

        self.advance_char();
        let mut value = String::new();

        while let Some(next) = self.peek_char() {
            self.advance_char();

            if next == '"' {
                return Ok(value);
            }

            if next == '\\' {
                let escaped = self.advance_char().ok_or(KeyValuesError::UnexpectedEnd)?;
                match escaped {
                    '"' => value.push('"'),
                    '\\' => value.push('\\'),
                    'n' => value.push('\n'),
                    'r' => value.push('\r'),
                    't' => value.push('\t'),
                    other => {
                        value.push('\\');
                        value.push(other);
                    }
                }
                continue;
            }

            value.push(next);
        }

        Err(KeyValuesError::UnexpectedEnd)
    }

    fn skip_whitespace(&mut self) {
        while self.peek_char().is_some_and(char::is_whitespace) {
            self.advance_char();
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.position..].chars().next()
    }

    fn advance_char(&mut self) -> Option<char> {
        let next = self.peek_char()?;
        self.position += next.len_utf8();
        Some(next)
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.input.len()
    }

    fn remaining_token(&self) -> &str {
        self.input[self.position..]
            .split_whitespace()
            .next()
            .unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_key_values_parses_nested_objects() {
        let parsed = parse_key_values(
            r#"
            "libraryfolders"
            {
                "0"
                {
                    "path" "D:\\SteamLibrary"
                    "apps"
                    {
                        "582010" "123"
                    }
                }
            }
            "#,
        )
        .expect("valid vdf");

        assert!(matches!(parsed, KeyValueNode::Object(_)));
    }

    #[test]
    fn steam_key_values_rejects_unclosed_quote() {
        let error = parse_key_values(r#""libraryfolders" { "0"#).expect_err("invalid vdf");
        assert_eq!(error, KeyValuesError::UnexpectedEnd);
    }

    #[test]
    fn steam_key_values_parses_escaped_characters() {
        let parsed = parse_key_values(
            r#"
            "AppState"
            {
                "installdir" "Monster \"Hunter\" World"
                "path" "D:\\SteamLibrary"
                "note" "line\nnext"
            }
            "#,
        )
        .expect("valid vdf");

        let KeyValueNode::Object(root) = parsed else {
            panic!("root should be object");
        };
        let KeyValueNode::Object(app_state) = root.get("AppState").expect("app state") else {
            panic!("app state should be object");
        };

        assert_eq!(
            app_state.get("installdir"),
            Some(&KeyValueNode::Text("Monster \"Hunter\" World".to_owned()))
        );
        assert_eq!(
            app_state.get("path"),
            Some(&KeyValueNode::Text("D:\\SteamLibrary".to_owned()))
        );
        assert_eq!(
            app_state.get("note"),
            Some(&KeyValueNode::Text("line\nnext".to_owned()))
        );
    }

    #[test]
    fn steam_key_values_rejects_trailing_escape() {
        let error =
            parse_key_values(r#""AppState" { "installdir" "Monster \"#).expect_err("invalid vdf");

        assert_eq!(error, KeyValuesError::UnexpectedEnd);
    }
}
