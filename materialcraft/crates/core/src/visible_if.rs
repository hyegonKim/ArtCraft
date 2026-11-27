use crate::Value;

const MAX_TOKENS: usize = 256;
const MAX_DEPTH: usize = 32;

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Ident(String),
    Number(f64),
    Str(String),
    Bool(bool),
    LBracket,
    RBracket,
    LParen,
    RParen,
    Dot,
    Not,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Clone, Debug)]
enum Expr {
    Bool(bool),
    Num(f64),
    Str(String),
    Input(String),
    Index(Box<Expr>, usize),
    UnaryNot(Box<Expr>),
    UnaryNeg(Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BinOp {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Clone, Debug)]
enum EvalVal {
    Bool(bool),
    Num(f64),
    Str(String),
    Vec4([f64; 4], usize),
}

impl EvalVal {
    fn from_value(value: &Value) -> Self {
        match value {
            Value::Bool(b) => Self::Bool(*b),
            Value::Int(v) => Self::Num(f64::from(*v)),
            Value::Enum(v) => Self::Num(f64::from(*v)),
            Value::Float(v) => Self::Num(f64::from(*v)),
            Value::Float2(v) => Self::Vec4([f64::from(v[0]), f64::from(v[1]), 0.0, 0.0], 2),
            Value::Float3(v) => Self::Vec4([f64::from(v[0]), f64::from(v[1]), f64::from(v[2]), 0.0], 3),
            Value::Float4(v) | Value::Color(v) => Self::Vec4([f64::from(v[0]), f64::from(v[1]), f64::from(v[2]), f64::from(v[3])], 4),
            Value::Int2(v) => Self::Vec4([f64::from(v[0]), f64::from(v[1]), 0.0, 0.0], 2),
            Value::Int3(v) => Self::Vec4([f64::from(v[0]), f64::from(v[1]), f64::from(v[2]), 0.0], 3),
            Value::Int4(v) => Self::Vec4([f64::from(v[0]), f64::from(v[1]), f64::from(v[2]), f64::from(v[3])], 4),
            Value::Text(s) => Self::Str(s.clone()),
            _ => Self::Bool(false),
        }
    }

    fn to_bool(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::Num(n) => n.is_finite() && n.abs() > 1e-9,
            Self::Str(s) => !s.is_empty() && s != "0" && !s.eq_ignore_ascii_case("false"),
            Self::Vec4(v, len) => v.iter().take(*len).any(|x| x.is_finite() && x.abs() > 1e-9),
        }
    }

    fn to_num(&self) -> Option<f64> {
        match self {
            Self::Num(n) if n.is_finite() => Some(*n),
            Self::Num(_) => Some(0.0),
            Self::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Self::Vec4(v, _) => Some(if v[0].is_finite() { v[0] } else { 0.0 }),
            Self::Str(s) => s.trim().parse::<f64>().ok().filter(|n| n.is_finite()),
        }
    }

    fn component(&self, idx: usize) -> Self {
        match self {
            Self::Vec4(v, len) if idx < *len => Self::Num(v.get(idx).copied().unwrap_or(0.0)),
            Self::Num(n) if idx == 0 => Self::Num(*n),
            Self::Bool(b) if idx == 0 => Self::Bool(*b),
            _ => Self::Num(0.0),
        }
    }

    fn equals(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Str(a), Self::Str(b)) => a == b,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            _ => match (self.to_num(), other.to_num()) {
                (Some(a), Some(b)) => (a - b).abs() <= 1e-6,
                _ => self.to_bool() == other.to_bool(),
            },
        }
    }
}

/// Validate a conditional visibility expression (`visibleIf`). Empty expressions are always valid.
pub fn validate_visible_if(expr: &str) -> std::result::Result<(), String> {
    if expr.trim().is_empty() {
        return Ok(());
    }
    let tokens = tokenize(expr)?;
    let mut parser = Parser { tokens: &tokens, pos: 0 };
    let _ = parser.parse_expr(0)?;
    if parser.pos < parser.tokens.len() {
        return Err("unexpected trailing tokens in visibility expression".into());
    }
    Ok(())
}

/// Evaluate a `visibleIf` expression against graph/instance input values.
/// An empty expression evaluates to `Ok(true)`.
pub fn evaluate_visible_if<'a>(expr: &str, lookup: &dyn Fn(&str) -> Option<&'a Value>) -> std::result::Result<bool, String> {
    if expr.trim().is_empty() {
        return Ok(true);
    }
    let tokens = tokenize(expr)?;
    let mut parser = Parser { tokens: &tokens, pos: 0 };
    let ast = parser.parse_expr(0)?;
    if parser.pos < parser.tokens.len() {
        return Err("unexpected trailing tokens in visibility expression".into());
    }
    Ok(eval_ast(&ast, lookup).to_bool())
}

/// Convenience helper for UI panels: empty or malformed expressions keep the input visible.
pub fn is_parameter_visible<'a>(expr: &str, lookup: &dyn Fn(&str) -> Option<&'a Value>) -> bool {
    evaluate_visible_if(expr, lookup).unwrap_or(true)
}

fn tokenize(src: &str) -> std::result::Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = src.chars().peekable();
    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }
        if tokens.len() >= MAX_TOKENS {
            return Err("expression is too long".into());
        }
        match ch {
            '[' => {
                chars.next();
                tokens.push(Token::LBracket);
            }
            ']' => {
                chars.next();
                tokens.push(Token::RBracket);
            }
            '(' => {
                chars.next();
                tokens.push(Token::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RParen);
            }
            '.' => {
                chars.next();
                tokens.push(Token::Dot);
            }
            '+' => {
                chars.next();
                tokens.push(Token::Plus);
            }
            '-' => {
                chars.next();
                tokens.push(Token::Minus);
            }
            '*' => {
                chars.next();
                tokens.push(Token::Star);
            }
            '/' => {
                chars.next();
                tokens.push(Token::Slash);
            }
            '%' => {
                chars.next();
                tokens.push(Token::Percent);
            }
            '!' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Ne);
                } else {
                    tokens.push(Token::Not);
                }
            }
            '=' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Eq);
                } else {
                    return Err("expected `==`".into());
                }
            }
            '<' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Le);
                } else {
                    tokens.push(Token::Lt);
                }
            }
            '>' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Ge);
                } else {
                    tokens.push(Token::Gt);
                }
            }
            '&' => {
                chars.next();
                if chars.peek() == Some(&'&') {
                    chars.next();
                    tokens.push(Token::And);
                } else {
                    return Err("expected `&&`".into());
                }
            }
            '|' => {
                chars.next();
                if chars.peek() == Some(&'|') {
                    chars.next();
                    tokens.push(Token::Or);
                } else {
                    return Err("expected `||`".into());
                }
            }
            '"' | '\'' => {
                let quote = ch;
                chars.next();
                let mut text = String::new();
                let mut closed = false;
                while let Some(c) = chars.next() {
                    if c == quote {
                        closed = true;
                        break;
                    }
                    if c == '\\' {
                        if let Some(escaped) = chars.next() {
                            text.push(escaped);
                        }
                    } else {
                        text.push(c);
                    }
                }
                if !closed {
                    return Err("unterminated string literal".into());
                }
                tokens.push(Token::Str(text));
            }
            c if c.is_ascii_digit() => {
                let mut num = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() || d == '.' || d == 'e' || d == 'E' {
                        num.push(d);
                        chars.next();
                        if (d == 'e' || d == 'E')
                            && matches!(chars.peek(), Some('+' | '-'))
                            && let Some(sign) = chars.next()
                        {
                            num.push(sign);
                        }
                    } else {
                        break;
                    }
                }
                let parsed = num.parse::<f64>().ok().filter(|v| v.is_finite()).ok_or_else(|| format!("invalid number `{num}`"))?;
                tokens.push(Token::Number(parsed));
            }
            '$' => {
                chars.next();
                let mut ident = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        ident.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if ident.is_empty() {
                    return Err("expected identifier after `$`".into());
                }
                tokens.push(Token::Ident(ident));
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut ident = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        ident.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                match ident.as_str() {
                    "true" | "True" | "TRUE" => tokens.push(Token::Bool(true)),
                    "false" | "False" | "FALSE" => tokens.push(Token::Bool(false)),
                    "and" | "AND" => tokens.push(Token::And),
                    "or" | "OR" => tokens.push(Token::Or),
                    "not" | "NOT" => tokens.push(Token::Not),
                    _ => tokens.push(Token::Ident(ident)),
                }
            }
            other => return Err(format!("unexpected character `{other}`")),
        }
    }
    Ok(tokens)
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&'a Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<&'a Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn parse_expr(&mut self, depth: usize) -> std::result::Result<Expr, String> {
        if depth > MAX_DEPTH {
            return Err("expression nesting is too deep".into());
        }
        self.parse_or(depth)
    }

    fn parse_or(&mut self, depth: usize) -> std::result::Result<Expr, String> {
        let mut left = self.parse_and(depth + 1)?;
        while self.peek() == Some(&Token::Or) {
            self.next();
            let right = self.parse_and(depth + 1)?;
            left = Expr::Binary(BinOp::Or, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self, depth: usize) -> std::result::Result<Expr, String> {
        let mut left = self.parse_equality(depth + 1)?;
        while self.peek() == Some(&Token::And) {
            self.next();
            let right = self.parse_equality(depth + 1)?;
            left = Expr::Binary(BinOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_equality(&mut self, depth: usize) -> std::result::Result<Expr, String> {
        let mut left = self.parse_relational(depth + 1)?;
        while let Some(op) = match self.peek() {
            Some(Token::Eq) => Some(BinOp::Eq),
            Some(Token::Ne) => Some(BinOp::Ne),
            _ => None,
        } {
            self.next();
            let right = self.parse_relational(depth + 1)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_relational(&mut self, depth: usize) -> std::result::Result<Expr, String> {
        let mut left = self.parse_additive(depth + 1)?;
        while let Some(op) = match self.peek() {
            Some(Token::Lt) => Some(BinOp::Lt),
            Some(Token::Le) => Some(BinOp::Le),
            Some(Token::Gt) => Some(BinOp::Gt),
            Some(Token::Ge) => Some(BinOp::Ge),
            _ => None,
        } {
            self.next();
            let right = self.parse_additive(depth + 1)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_additive(&mut self, depth: usize) -> std::result::Result<Expr, String> {
        let mut left = self.parse_multiplicative(depth + 1)?;
        while let Some(op) = match self.peek() {
            Some(Token::Plus) => Some(BinOp::Add),
            Some(Token::Minus) => Some(BinOp::Sub),
            _ => None,
        } {
            self.next();
            let right = self.parse_multiplicative(depth + 1)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self, depth: usize) -> std::result::Result<Expr, String> {
        let mut left = self.parse_unary(depth + 1)?;
        while let Some(op) = match self.peek() {
            Some(Token::Star) => Some(BinOp::Mul),
            Some(Token::Slash) => Some(BinOp::Div),
            Some(Token::Percent) => Some(BinOp::Rem),
            _ => None,
        } {
            self.next();
            let right = self.parse_unary(depth + 1)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self, depth: usize) -> std::result::Result<Expr, String> {
        if depth > MAX_DEPTH {
            return Err("expression nesting is too deep".into());
        }
        match self.peek() {
            Some(Token::Not) => {
                self.next();
                Ok(Expr::UnaryNot(Box::new(self.parse_unary(depth + 1)?)))
            }
            Some(Token::Minus) => {
                self.next();
                Ok(Expr::UnaryNeg(Box::new(self.parse_unary(depth + 1)?)))
            }
            Some(Token::Plus) => {
                self.next();
                self.parse_unary(depth + 1)
            }
            _ => self.parse_postfix(depth + 1),
        }
    }

    fn parse_postfix(&mut self, depth: usize) -> std::result::Result<Expr, String> {
        let mut expr = self.parse_primary(depth + 1)?;
        loop {
            match self.peek() {
                Some(Token::LBracket) => {
                    self.next();
                    let Some(Token::Number(idx_f)) = self.next() else {
                        return Err("expected numeric component index inside `[...]`".into());
                    };
                    if *idx_f < 0.0 || *idx_f > 3.0 || (*idx_f - idx_f.round()).abs() > 1e-6 {
                        return Err("vector component index must be 0, 1, 2 or 3".into());
                    }
                    if self.next() != Some(&Token::RBracket) {
                        return Err("expected `]` after component index".into());
                    }
                    expr = Expr::Index(Box::new(expr), *idx_f as usize);
                }
                Some(Token::Dot) => {
                    self.next();
                    let Some(Token::Ident(swizzle)) = self.next() else {
                        return Err("expected component name (`x`, `y`, `z`, `w`) after `.`".into());
                    };
                    let idx = match swizzle.as_str() {
                        "x" | "r" => 0,
                        "y" | "g" => 1,
                        "z" | "b" => 2,
                        "w" | "a" => 3,
                        _ => return Err(format!("unknown vector component `.{swizzle}`")),
                    };
                    expr = Expr::Index(Box::new(expr), idx);
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self, depth: usize) -> std::result::Result<Expr, String> {
        if depth > MAX_DEPTH {
            return Err("expression nesting is too deep".into());
        }
        match self.next() {
            Some(Token::Bool(b)) => Ok(Expr::Bool(*b)),
            Some(Token::Number(n)) => Ok(Expr::Num(*n)),
            Some(Token::Str(s)) => Ok(Expr::Str(s.clone())),
            Some(Token::LParen) => {
                let inner = self.parse_expr(depth + 1)?;
                if self.next() != Some(&Token::RParen) {
                    return Err("expected closing `)`".into());
                }
                Ok(inner)
            }
            Some(Token::Ident(name)) if (name == "input" || name == "inputs") && self.peek() == Some(&Token::LBracket) => {
                self.next(); // consume `[`
                let key = match self.next() {
                    Some(Token::Str(s)) if !s.trim().is_empty() => s.clone(),
                    Some(Token::Ident(s)) => s.clone(),
                    _ => return Err("expected input identifier inside `input[...]`".into()),
                };
                if self.next() != Some(&Token::RBracket) {
                    return Err("expected `]` after `input[...]`".into());
                }
                Ok(Expr::Input(key))
            }
            Some(Token::Ident(name)) => Ok(Expr::Input(name.clone())),
            Some(other) => Err(format!("unexpected token `{other:?}`")),
            None => Err("unexpected end of expression".into()),
        }
    }
}

fn eval_ast<'a>(expr: &Expr, lookup: &dyn Fn(&str) -> Option<&'a Value>) -> EvalVal {
    match expr {
        Expr::Bool(b) => EvalVal::Bool(*b),
        Expr::Num(n) => EvalVal::Num(*n),
        Expr::Str(s) => EvalVal::Str(s.clone()),
        Expr::Input(id) => lookup(id).map_or(EvalVal::Bool(false), EvalVal::from_value),
        Expr::Index(inner, idx) => eval_ast(inner, lookup).component(*idx),
        Expr::UnaryNot(inner) => EvalVal::Bool(!eval_ast(inner, lookup).to_bool()),
        Expr::UnaryNeg(inner) => EvalVal::Num(-eval_ast(inner, lookup).to_num().unwrap_or(0.0)),
        Expr::Binary(BinOp::Or, lhs, rhs) => EvalVal::Bool(eval_ast(lhs, lookup).to_bool() || eval_ast(rhs, lookup).to_bool()),
        Expr::Binary(BinOp::And, lhs, rhs) => EvalVal::Bool(eval_ast(lhs, lookup).to_bool() && eval_ast(rhs, lookup).to_bool()),
        Expr::Binary(BinOp::Eq, lhs, rhs) => EvalVal::Bool(eval_ast(lhs, lookup).equals(&eval_ast(rhs, lookup))),
        Expr::Binary(BinOp::Ne, lhs, rhs) => EvalVal::Bool(!eval_ast(lhs, lookup).equals(&eval_ast(rhs, lookup))),
        Expr::Binary(op @ (BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge), lhs, rhs) => {
            let a = eval_ast(lhs, lookup).to_num().unwrap_or(0.0);
            let b = eval_ast(rhs, lookup).to_num().unwrap_or(0.0);
            let res = match op {
                BinOp::Lt => a < b,
                BinOp::Le => a <= b,
                BinOp::Gt => a > b,
                BinOp::Ge => a >= b,
                _ => false,
            };
            EvalVal::Bool(res)
        }
        Expr::Binary(op @ (BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem), lhs, rhs) => {
            let a = eval_ast(lhs, lookup).to_num().unwrap_or(0.0);
            let b = eval_ast(rhs, lookup).to_num().unwrap_or(0.0);
            let res = match op {
                BinOp::Add => a + b,
                BinOp::Sub => a - b,
                BinOp::Mul => a * b,
                BinOp::Div => {
                    if b.abs() <= 1e-12 {
                        0.0
                    } else {
                        a / b
                    }
                }
                BinOp::Rem => {
                    if b.abs() <= 1e-12 {
                        0.0
                    } else {
                        a % b
                    }
                }
                _ => 0.0,
            };
            EvalVal::Num(if res.is_finite() { res } else { 0.0 })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn evaluates_boolean_and_comparison_visibility_expressions() {
        let map = BTreeMap::from([
            ("mode".to_string(), Value::Enum(1)),
            ("use_detail".to_string(), Value::Bool(true)),
            ("amount".to_string(), Value::Float(0.75)),
            ("tiling".to_string(), Value::Int2([2, 4])),
            ("tag".to_string(), Value::Text("metal".into())),
        ]);
        let get = |id: &str| map.get(id);

        assert_eq!(evaluate_visible_if("", &get), Ok(true));
        assert_eq!(evaluate_visible_if(r#"input["mode"] == 1 && input["use_detail"]"#, &get), Ok(true));
        assert_eq!(evaluate_visible_if(r#"input["mode"] == 2 || !input["use_detail"]"#, &get), Ok(false));
        assert_eq!(evaluate_visible_if(r#"input['amount'] >= 0.5 && input['amount'] < 1.0"#, &get), Ok(true));
        assert_eq!(evaluate_visible_if(r#"$mode != 0 && (amount > 0.8 || tiling[1] == 4)"#, &get), Ok(true));
        assert_eq!(evaluate_visible_if(r#"tiling.x == 2 && tiling.y > 3"#, &get), Ok(true));
        assert_eq!(evaluate_visible_if(r#"input["use_detail"] == 1"#, &get), Ok(true));
        assert_eq!(evaluate_visible_if(r#"input["tag"] == "metal""#, &get), Ok(true));
        assert_eq!(evaluate_visible_if(r#"input["missing"]"#, &get), Ok(false));

        assert!(validate_visible_if(r#"input["mode"] == 1 && !input["advanced"]"#).is_ok());
        assert!(validate_visible_if(r#"input["mode"] =="#).is_err());
        assert!(validate_visible_if(r#"input[] == 1"#).is_err());
        assert!(is_parameter_visible(r#"broken == "#, &get));
    }
}
