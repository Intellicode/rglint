//! Recursive-descent parser for the selector language (spec-010 / PLAN §4.3).
//!
//! Consumes the [`lexer`]'s token stream and produces a [`SelectorNode`]
//! tree. Whitespace tokens are kept by the lexer and consumed here to
//! detect descendant combinators (`A B`) versus child combinators
//! (`A > B`); explicit `>` overrides surrounding whitespace.
//!
//! ## Compound encoding
//!
//! The spec's [`SelectorNode`] enum has no `And` variant — only `Matches`
//! (OR) and `Not` (negation). A compound like
//! `FieldDefinition[name.value=/^_/]` is "Kind **AND** Attribute", which we
//! encode by De Morgan as `Not([Not(Kind), Not(Attribute)])`. The matcher
//! evaluates that as "not (not-Kind OR not-Attribute)" == "Kind AND
//! Attribute". Top-level `:matches`/`:not` keep their natural shapes.

use crate::selector::ast::{AttrKind, AttrOp, AttrValue, SelectorError, SelectorNode};
use crate::selector::lexer::{lex, Tok, TokKind};

/// Parse a selector string into a [`SelectorNode`] tree.
///
/// This is the shared front end of [`crate::selector::compile`]; the
/// [`crate::selector::matcher`] module then walks the tree to produce a
/// [`Matcher`](crate::selector::Matcher).
pub fn parse(src: &str) -> Result<SelectorNode, SelectorError> {
    let toks = lex(src)?;
    let mut p = Parser {
        toks,
        pos: 0,
        src_len: src.len(),
    };
    let node = p.parse_selector()?;
    // Only trailing whitespace may remain; anything else is unexpected.
    p.skip_ws();
    if let Some(tok) = p.peek() {
        return Err(SelectorError::Parse {
            span: tok.start,
            message: "unexpected token after selector".to_owned(),
        });
    }
    Ok(node)
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    src_len: usize,
}

impl Parser {
    /// `selector := compound (combinator compound)*`
    fn parse_selector(&mut self) -> Result<SelectorNode, SelectorError> {
        let mut left = self.parse_compound()?;
        loop {
            let saw_ws = self.skip_ws();
            match self.peek() {
                Some(Tok {
                    kind: TokKind::Gt, ..
                }) => {
                    self.advance();
                    self.skip_ws();
                    let right = self.parse_compound()?;
                    left = SelectorNode::Child(Box::new(left), Box::new(right));
                }
                Some(_) if saw_ws => {
                    let right = self.parse_compound()?;
                    left = SelectorNode::Descendant(Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    /// `compound := primary (filter)*`
    /// `primary := IDENT | '[' attr ']' | ':' pseudo`
    /// `filter := '[' attr ']' | ':' pseudo`
    ///
    /// Successive filters conjoin with the primary via De Morgan
    /// (see the module-level note).
    fn parse_compound(&mut self) -> Result<SelectorNode, SelectorError> {
        self.skip_ws();
        let primary = match self.peek() {
            Some(Tok {
                kind: TokKind::Ident(_),
                ..
            }) => {
                let tok = self.peek().unwrap().clone();
                self.advance();
                validate_kind_name(&ident_value(&tok), tok.start)?;
                SelectorNode::Kind(ident_value(&tok))
            }
            Some(Tok {
                kind: TokKind::LBracket,
                ..
            }) => {
                self.advance();
                self.parse_attribute()?
            }
            Some(Tok {
                kind: TokKind::Colon,
                ..
            }) => {
                self.advance();
                self.parse_pseudo()?
            }
            Some(tok) => {
                return Err(SelectorError::Parse {
                    span: tok.start,
                    message: format!("expected a selector, found `{}`", tok_desc(&tok.kind)),
                });
            }
            None => {
                return Err(SelectorError::Parse {
                    span: self.src_len,
                    message: "expected a selector, found end of input".to_owned(),
                });
            }
        };

        let mut filters: Vec<SelectorNode> = Vec::new();
        loop {
            // Look ahead past optional whitespace. If the next non-ws
            // token is a filter (`[` or `:`), consume the ws + token and
            // parse it. Otherwise break **without** consuming the ws —
            // that whitespace may be a descendant combinator that
            // [`parse_selector`](Self::parse_selector) needs to see.
            match self.peek_non_ws().map(|t| &t.kind) {
                Some(TokKind::LBracket) => {
                    self.skip_ws_inline();
                    self.advance();
                    filters.push(self.parse_attribute()?);
                }
                Some(TokKind::Colon) => {
                    self.skip_ws_inline();
                    self.advance();
                    filters.push(self.parse_pseudo()?);
                }
                _ => break,
            }
        }

        if filters.is_empty() {
            return Ok(primary);
        }
        // De Morgan: primary AND f1 AND ... AND fn
        //        == NOT(NOT(primary), NOT(f1), ..., NOT(fn))
        let mut inner: Vec<SelectorNode> = Vec::with_capacity(filters.len() + 1);
        inner.push(SelectorNode::Not(vec![primary]));
        for f in filters {
            inner.push(SelectorNode::Not(vec![f]));
        }
        Ok(SelectorNode::Not(inner))
    }

    /// `pseudo := IDENT '(' arg_list ')'` — caller has already consumed the
    /// leading `:`. Supports `:matches` and `:not`.
    fn parse_pseudo(&mut self) -> Result<SelectorNode, SelectorError> {
        let tok = match self.peek() {
            Some(t) => t.clone(),
            None => {
                return Err(SelectorError::Parse {
                    span: self.src_len,
                    message: "expected pseudo-class name after `:`".to_owned(),
                });
            }
        };
        let name = match &tok.kind {
            TokKind::Ident(s) => s.clone(),
            _ => {
                return Err(SelectorError::Parse {
                    span: tok.start,
                    message: format!(
                        "expected pseudo-class name after `:`, found `{}`",
                        tok_desc(&tok.kind)
                    ),
                });
            }
        };
        self.advance();
        if !matches!(name.as_str(), "matches" | "not") {
            return Err(SelectorError::Parse {
                span: tok.start,
                message: format!("unsupported pseudo-class `:{name}`"),
            });
        }

        self.skip_ws_inline();
        match self.peek() {
            Some(Tok {
                kind: TokKind::LParen,
                ..
            }) => self.advance(),
            Some(tok) => {
                return Err(SelectorError::Parse {
                    span: tok.start,
                    message: format!(
                        "expected `(` after `:{name}`, found `{}`",
                        tok_desc(&tok.kind)
                    ),
                });
            }
            None => {
                return Err(SelectorError::Parse {
                    span: self.src_len,
                    message: format!("expected `(` after `:{name}`, found end of input"),
                });
            }
        }

        let mut args: Vec<SelectorNode> = Vec::new();
        loop {
            self.skip_ws();
            if matches!(self.peek().map(|t| &t.kind), Some(TokKind::RParen)) {
                break;
            }
            args.push(self.parse_selector()?);
            self.skip_ws();
            match self.peek() {
                Some(Tok {
                    kind: TokKind::Comma,
                    ..
                }) => self.advance(),
                Some(Tok {
                    kind: TokKind::RParen,
                    ..
                }) => break,
                Some(tok) => {
                    return Err(SelectorError::Parse {
                        span: tok.start,
                        message: format!(
                            "expected `,` or `)` in `:{name}(...)`, found `{}`",
                            tok_desc(&tok.kind)
                        ),
                    });
                }
                None => {
                    return Err(SelectorError::Parse {
                        span: self.src_len,
                        message: format!("unclosed `:{name}(` — missing `)`"),
                    });
                }
            }
        }

        match self.peek() {
            Some(Tok {
                kind: TokKind::RParen,
                ..
            }) => self.advance(),
            Some(tok) => {
                return Err(SelectorError::Parse {
                    span: tok.start,
                    message: format!(
                        "expected `)` to close `:{name}(...)`, found `{}`",
                        tok_desc(&tok.kind)
                    ),
                });
            }
            None => {
                return Err(SelectorError::Parse {
                    span: self.src_len,
                    message: format!("unclosed `:{name}(` — missing `)`"),
                });
            }
        }

        Ok(if name == "matches" {
            SelectorNode::Matches(args)
        } else {
            SelectorNode::Not(args)
        })
    }

    /// `attr := attr_key (op value)? ']'` — caller has already consumed `[`.
    fn parse_attribute(&mut self) -> Result<SelectorNode, SelectorError> {
        self.skip_ws_inline();
        let key_tok = match self.peek() {
            Some(t) => t.clone(),
            None => {
                return Err(SelectorError::Parse {
                    span: self.src_len,
                    message: "unclosed attribute `[`".to_owned(),
                });
            }
        };
        let key = match &key_tok.kind {
            TokKind::Ident(s) => s.clone(),
            _ => {
                return Err(SelectorError::Parse {
                    span: key_tok.start,
                    message: format!(
                        "expected attribute key after `[`, found `{}`",
                        tok_desc(&key_tok.kind)
                    ),
                });
            }
        };
        self.advance();
        let target = parse_attr_kind(&key, key_tok.start)?;

        self.skip_ws_inline();
        let (op, value) = match self.peek() {
            Some(Tok {
                kind: TokKind::Eq, ..
            }) => {
                self.advance();
                self.skip_ws_inline();
                // graphql-eslint/esquery spells regex match as
                // `[k=/.../ ]` — the `=` operator is overloaded by the
                // RHS: a regex literal means regex-match, a string or bare
                // identifier means equality. Inspect the RHS to pick the
                // op so a malformed regex still surfaces as
                // [`SelectorError::Regex`] (with the literal's span)
                // rather than a confusing "expected string" Parse error.
                let op = match self.peek().map(|t| &t.kind) {
                    Some(TokKind::Regex(_)) => AttrOp::RegexMatch,
                    _ => AttrOp::Eq,
                };
                let v = self.parse_attr_value(op, key_tok.start)?;
                (op, v)
            }
            Some(Tok {
                kind: TokKind::RegexEq,
                ..
            }) => {
                // Explicit `=~` form (not used by graphql-eslint but
                // supported for parity); the RHS must be a regex literal.
                self.advance();
                self.skip_ws_inline();
                let v = self.parse_attr_value(AttrOp::RegexMatch, key_tok.start)?;
                (AttrOp::RegexMatch, v)
            }
            Some(Tok {
                kind: TokKind::RBracket,
                ..
            }) => {
                return Err(SelectorError::Parse {
                    span: key_tok.start,
                    message: format!(
                        "attribute `[{}]` has no operator; expected `= value` or `= /regex/`",
                        key
                    ),
                });
            }
            Some(tok) => {
                return Err(SelectorError::Parse {
                    span: tok.start,
                    message: format!(
                        "expected `=`, `=~`, or `]` after attribute key `{key}`, found `{}`",
                        tok_desc(&tok.kind)
                    ),
                });
            }
            None => {
                return Err(SelectorError::Parse {
                    span: self.src_len,
                    message: "unclosed attribute `[`".to_owned(),
                });
            }
        };

        self.skip_ws_inline();
        match self.peek() {
            Some(Tok {
                kind: TokKind::RBracket,
                ..
            }) => self.advance(),
            Some(tok) => {
                return Err(SelectorError::Parse {
                    span: tok.start,
                    message: format!(
                        "expected `]` to close attribute, found `{}`",
                        tok_desc(&tok.kind)
                    ),
                });
            }
            None => {
                return Err(SelectorError::Parse {
                    span: self.src_len,
                    message: "unclosed attribute `[`".to_owned(),
                });
            }
        }

        Ok(SelectorNode::Attribute { target, op, value })
    }

    /// Parse the RHS value of an attribute predicate. For `=`, the value
    /// may be a quoted string OR a bare identifier (graphql-eslint allows
    /// `[name.value=PageInfo]` — bare ident as a string). For `=~`, the
    /// value must be a regex literal.
    fn parse_attr_value(
        &mut self,
        op: AttrOp,
        _key_start: usize,
    ) -> Result<AttrValue, SelectorError> {
        let tok = match self.peek() {
            Some(t) => t.clone(),
            None => {
                return Err(SelectorError::Parse {
                    span: self.src_len,
                    message: "expected a value, found end of input".to_owned(),
                });
            }
        };
        let value = match (&op, &tok.kind) {
            (AttrOp::Eq, TokKind::Str(s)) => AttrValue::Str(s.clone()),
            (AttrOp::Eq, TokKind::Ident(s)) => AttrValue::Str(s.clone()),
            (AttrOp::RegexMatch, TokKind::Regex(s)) => {
                let r = regex::Regex::new(s).map_err(|e| SelectorError::Regex {
                    span: tok.start,
                    message: e.to_string(),
                })?;
                AttrValue::Regex(r)
            }
            (_, kind) => {
                return Err(SelectorError::Parse {
                    span: tok.start,
                    message: format!(
                        "expected {} after `{}`, found `{}`",
                        expected_value_for(op),
                        op_str(op),
                        tok_desc(kind),
                    ),
                });
            }
        };
        self.advance();
        Ok(value)
    }

    // --- token-cursor helpers ------------------------------------------------

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn advance(&mut self) {
        if self.pos < self.toks.len() {
            self.pos += 1;
        }
    }

    /// Consume consecutive whitespace tokens and report whether any were
    /// consumed. Used at combinator boundaries to distinguish `A B`
    /// (descendant) from `A > B` (child).
    fn skip_ws(&mut self) -> bool {
        let mut saw = false;
        while matches!(self.peek().map(|t| &t.kind), Some(TokKind::Whitespace)) {
            self.advance();
            saw = true;
        }
        saw
    }

    /// Whitespace inside a compound (around `(`, `[`, etc) is insignificant;
    /// this is [`skip_ws`](Self::skip_ws) with the boolean discarded.
    fn skip_ws_inline(&mut self) {
        let _ = self.skip_ws();
    }

    /// Peek at the next non-whitespace token without advancing the cursor.
    /// Used by the compound filter loop to decide whether a `[`/`:` filter
    /// follows without consuming a combinator-significant whitespace run.
    fn peek_non_ws(&self) -> Option<&Tok> {
        let mut i = self.pos;
        while i < self.toks.len() && matches!(self.toks[i].kind, TokKind::Whitespace) {
            i += 1;
        }
        self.toks.get(i)
    }
}

fn ident_value(tok: &Tok) -> String {
    match &tok.kind {
        TokKind::Ident(s) => s.clone(),
        _ => unreachable!("ident_value on non-ident token"),
    }
}

/// Resolve a selector attribute-key spelling into an [`AttrKind`].
///
/// `type` is accepted as an alias for `kind` (graphql-eslint spells it
/// `[type=ObjectTypeDefinition]`).
fn parse_attr_kind(key: &str, span: usize) -> Result<AttrKind, SelectorError> {
    match key {
        "name" | "name.value" => Ok(AttrKind::NameValue),
        "kind" | "type" => Ok(AttrKind::Kind),
        "description" | "description.value" => Ok(AttrKind::DescriptionValue),
        "value" | "value.raw" => Ok(AttrKind::ValueRaw),
        _ => Err(SelectorError::Parse {
            span,
            message: format!("unknown attribute key `{key}`"),
        }),
    }
}

/// Validate that a kind-name spelling resolves to a real
/// [`apollo_parser::SyntaxKind`] — typos surface as a
/// [`SelectorError::UnknownKind`] with a span at compile time, not as silent
/// never-match at runtime.
fn validate_kind_name(name: &str, span: usize) -> Result<(), SelectorError> {
    if crate::selector::matcher::kind_from_camel(name).is_some() {
        Ok(())
    } else {
        Err(SelectorError::UnknownKind {
            span,
            kind: name.to_owned(),
        })
    }
}

fn op_str(op: AttrOp) -> &'static str {
    match op {
        AttrOp::Eq => "=",
        AttrOp::RegexMatch => "=~",
    }
}

fn expected_value_for(op: AttrOp) -> &'static str {
    match op {
        AttrOp::Eq => "a string or identifier",
        AttrOp::RegexMatch => "a regex literal `/.../`",
    }
}

fn tok_desc(k: &TokKind) -> String {
    match k {
        TokKind::Ident(s) => format!("identifier `{s}`"),
        TokKind::Str(_) => "string literal".to_owned(),
        TokKind::Regex(_) => "regex literal".to_owned(),
        TokKind::Eq => "`=`".to_owned(),
        TokKind::RegexEq => "`=~`".to_owned(),
        TokKind::LBracket => "`[`".to_owned(),
        TokKind::RBracket => "`]`".to_owned(),
        TokKind::LParen => "`(`".to_owned(),
        TokKind::RParen => "`)`".to_owned(),
        TokKind::Gt => "`>`".to_owned(),
        TokKind::Colon => "`:`".to_owned(),
        TokKind::Comma => "`,`".to_owned(),
        TokKind::Whitespace => "whitespace".to_owned(),
    }
}
