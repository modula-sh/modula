//! Tiny boolean expression evaluator for agent rules.
//!
//! Grammar:
//!   expr   := or
//!   or     := and ('or' and)*
//!   and    := cmp ('and' cmp)*
//!   cmp    := value (('==' | '!=') value)?
//!   value  := path | string | bool | '(' expr ')'
//!   path   := IDENT ('.' IDENT)*
//!   string := '"…"' | "'…'"
//!   bool   := 'true' | 'false'
//!
//! Paths are looked up in a JSON object (typically the event row); a missing
//! field resolves to JSON null. `==`/`!=` compare structurally. A bool field can
//! be matched as `approved`, `approved == true`, or `approved == 'true'` (the
//! quoted form is coerced to bool in `json_eq`).

use serde_json::Value as Json;

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Str(String),
    Bool(bool),
    Dot,
    LParen,
    RParen,
    EqEq,
    BangEq,
    And,
    Or,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Path(Vec<String>),
    Str(String),
    Bool(bool),
    Eq(Box<Expr>, Box<Expr>),
    Ne(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

#[derive(Debug)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

pub fn parse(src: &str) -> Result<Expr, ParseError> {
    let toks = tokenize(src)?;
    let mut p = Parser { toks, pos: 0 };
    let e = p.parse_or()?;
    if p.pos != p.toks.len() {
        return Err(ParseError(format!(
            "unexpected token after expression: {:?}",
            p.toks[p.pos]
        )));
    }
    Ok(e)
}

pub fn eval(expr: &Expr, env: &Json) -> bool {
    match expr {
        Expr::And(a, b) => eval(a, env) && eval(b, env),
        Expr::Or(a, b) => eval(a, env) || eval(b, env),
        Expr::Eq(a, b) => json_eq(&value_of(a, env), &value_of(b, env)),
        Expr::Ne(a, b) => !json_eq(&value_of(a, env), &value_of(b, env)),
        // A bare path/string/bool used as a boolean: truthy iff non-null/non-empty.
        Expr::Path(_) | Expr::Str(_) | Expr::Bool(_) => is_truthy(&value_of(expr, env)),
    }
}

fn value_of(expr: &Expr, env: &Json) -> Json {
    match expr {
        Expr::Path(p) => lookup(env, p),
        Expr::Str(s) => Json::String(s.clone()),
        Expr::Bool(b) => Json::Bool(*b),
        _ => Json::Null,
    }
}

fn lookup(env: &Json, path: &[String]) -> Json {
    let mut cur = env;
    for key in path {
        match cur {
            Json::Object(map) => match map.get(key) {
                Some(v) => cur = v,
                None => return Json::Null,
            },
            _ => return Json::Null,
        }
    }
    cur.clone()
}

fn json_eq(a: &Json, b: &Json) -> bool {
    match (a, b) {
        // Treat string vs string-number equality intuitively: "1" == 1.
        (Json::String(s), Json::Number(n)) | (Json::Number(n), Json::String(s)) => {
            s == &n.to_string()
        }
        // Likewise string vs bool: 'true' == true, 'false' == false. Lets a
        // boolean field be matched with either `== true` or `== 'true'`.
        (Json::String(s), Json::Bool(b)) | (Json::Bool(b), Json::String(s)) => s == &b.to_string(),
        _ => a == b,
    }
}

fn is_truthy(v: &Json) -> bool {
    match v {
        Json::Null => false,
        Json::Bool(b) => *b,
        Json::String(s) => !s.is_empty(),
        Json::Number(n) => n.as_f64().is_none_or(|f| f != 0.0),
        Json::Array(a) => !a.is_empty(),
        Json::Object(o) => !o.is_empty(),
    }
}

fn tokenize(src: &str) -> Result<Vec<Tok>, ParseError> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '.' => {
                out.push(Tok::Dot);
                i += 1;
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            '=' if i + 1 < bytes.len() && bytes[i + 1] == b'=' => {
                out.push(Tok::EqEq);
                i += 2;
            }
            '!' if i + 1 < bytes.len() && bytes[i + 1] == b'=' => {
                out.push(Tok::BangEq);
                i += 2;
            }
            '"' | '\'' => {
                let quote = c;
                let start = i + 1;
                let mut j = start;
                while j < bytes.len() && bytes[j] as char != quote {
                    j += 1;
                }
                if j >= bytes.len() {
                    return Err(ParseError("unterminated string literal".into()));
                }
                out.push(Tok::Str(src[start..j].to_string()));
                i = j + 1;
            }
            _ if c.is_ascii_alphabetic() || c == '_' => {
                let start = i;
                while i < bytes.len() {
                    let c2 = bytes[i] as char;
                    if c2.is_ascii_alphanumeric() || c2 == '_' {
                        i += 1;
                    } else {
                        break;
                    }
                }
                let word = &src[start..i];
                out.push(match word {
                    "and" => Tok::And,
                    "or" => Tok::Or,
                    "true" => Tok::Bool(true),
                    "false" => Tok::Bool(false),
                    _ => Tok::Ident(word.to_string()),
                });
            }
            _ => {
                return Err(ParseError(format!(
                    "unexpected character {c:?} at byte {i}"
                )))
            }
        }
    }
    Ok(out)
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn bump(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned()?;
        self.pos += 1;
        Some(t)
    }
    fn eat(&mut self, want: &Tok) -> bool {
        if self.peek() == Some(want) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and()?;
        while self.eat(&Tok::Or) {
            let right = self.parse_and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_cmp()?;
        while self.eat(&Tok::And) {
            let right = self.parse_cmp()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn parse_cmp(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_value()?;
        match self.peek() {
            Some(Tok::EqEq) => {
                self.pos += 1;
                let right = self.parse_value()?;
                Ok(Expr::Eq(Box::new(left), Box::new(right)))
            }
            Some(Tok::BangEq) => {
                self.pos += 1;
                let right = self.parse_value()?;
                Ok(Expr::Ne(Box::new(left), Box::new(right)))
            }
            _ => Ok(left),
        }
    }
    fn parse_value(&mut self) -> Result<Expr, ParseError> {
        match self.bump() {
            Some(Tok::LParen) => {
                let e = self.parse_or()?;
                if !self.eat(&Tok::RParen) {
                    return Err(ParseError("missing closing paren".into()));
                }
                Ok(e)
            }
            Some(Tok::Str(s)) => Ok(Expr::Str(s)),
            Some(Tok::Bool(b)) => Ok(Expr::Bool(b)),
            Some(Tok::Ident(head)) => {
                let mut path = vec![head];
                while self.eat(&Tok::Dot) {
                    match self.bump() {
                        Some(Tok::Ident(n)) => path.push(n),
                        other => {
                            return Err(ParseError(format!(
                                "expected identifier after '.', got {other:?}"
                            )))
                        }
                    }
                }
                Ok(Expr::Path(path))
            }
            other => Err(ParseError(format!("expected value, got {other:?}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn env() -> Json {
        json!({
            "type": "task.create",
            "data": {
                "task_id": "FAC-1",
                "approved": true,
                "status": "ready_for_research",
            }
        })
    }

    #[test]
    fn eq_path_and_string() {
        let e = parse("type == 'task.create'").unwrap();
        assert!(eval(&e, &env()));
    }

    #[test]
    fn ne_path() {
        let e = parse("type != 'task.update'").unwrap();
        assert!(eval(&e, &env()));
    }

    #[test]
    fn dotted_path() {
        let e = parse("data.task_id == 'FAC-1'").unwrap();
        assert!(eval(&e, &env()));
    }

    #[test]
    fn missing_path_is_false_eq_str() {
        let e = parse("data.missing == 'x'").unwrap();
        assert!(!eval(&e, &env()));
    }

    #[test]
    fn and_or_precedence() {
        let e =
            parse("type == 'task.create' and data.task_id == 'FAC-1' or type == 'other'").unwrap();
        assert!(eval(&e, &env()));
    }

    #[test]
    fn parens_group() {
        let e =
            parse("(type == 'a' or type == 'task.create') and data.task_id == 'FAC-1'").unwrap();
        assert!(eval(&e, &env()));
    }

    #[test]
    fn bool_literal_and_string_coerce() {
        // approved is JSON bool true; all three spellings should match.
        assert!(eval(&parse("data.approved").unwrap(), &env()));
        assert!(eval(&parse("data.approved == true").unwrap(), &env()));
        assert!(eval(&parse("data.approved == 'true'").unwrap(), &env()));
        // …and the negatives should not.
        assert!(!eval(&parse("data.approved == false").unwrap(), &env()));
        assert!(!eval(&parse("data.approved == 'false'").unwrap(), &env()));
        assert!(eval(&parse("data.approved != false").unwrap(), &env()));
    }

    #[test]
    fn bool_literal_against_missing_path() {
        // A missing field is null, which is neither true nor false.
        assert!(!eval(&parse("data.missing == true").unwrap(), &env()));
        assert!(!eval(&parse("data.missing == false").unwrap(), &env()));
    }

    #[test]
    fn double_quoted_string() {
        let e = parse("data.status == \"ready_for_research\"").unwrap();
        assert!(eval(&e, &env()));
    }

    #[test]
    fn parse_error_bubbles() {
        assert!(parse("foo ==").is_err());
        assert!(parse("(a == 'b'").is_err());
        assert!(parse("'unterminated").is_err());
    }
}
