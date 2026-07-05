//! Tokenizer for the selector language (spec-010 / PLAN §4.3).
//!
//! Produces a flat [`Vec<Tok>`] of `(kind, payload, span)` tuples the
//! [`parser`](super::parser) consumes. Whitespace is **retained** as
//! [`TokKind::Whitespace`] tokens — the parser needs it to detect descendant
//! combinators (a run of whitespace between two compounds that isn't
//! "absorbed" by a `>`).
//!
//! ## Lexical grammar
//!
//! ```text
//! IDENT      = [A-Za-z_][A-Za-z0-9_.]*   (kind names; "name.value"; "kind")
//! STRING     = '"' ... '"' | "'" ... "'"
//! REGEX      = '/' ... '/'
//! OP         = '=' | '=~'
//! PUNCT      = '[' ']' '(' ')' '>' ':'
//! WS         = [ \t\r\n]+
//! ```
//!
//! Regex literals use `/` as the delimiter and `\\` as the escape character;
//! an unescaped `/` ends the literal. The lexer does **not** validate the
//! regex — the parser compiles it via [`regex::Regex::new`] and surfaces a
//! [`SelectorError::Regex`] on failure.

/// A kind of token produced by the lexer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokKind {
    /// An identifier — a kind name (`ObjectTypeDefinition`), an attribute
    /// key (`name.value`, `kind`), or a pseudo-class name (`matches`,
    /// `not`).
    Ident(String),
    /// A quoted string literal, with the surrounding quotes stripped.
    Str(String),
    /// A regex literal, with the surrounding slashes stripped (and backslash
    /// escapes resolved except for the closing delimiter).
    Regex(String),
    /// `=`
    Eq,
    /// `=~`
    RegexEq,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `>` (child combinator)
    Gt,
    /// `:` (pseudo-class introducer)
    Colon,
    /// `,` (pseudo-class argument separator)
    Comma,
    /// A run of ASCII whitespace; kept so the parser can detect descendant
    /// combinators.
    Whitespace,
}

/// A single token: its kind, the byte offset where it started, and the byte
/// offset where the *next* token starts (== start + length).
#[derive(Debug, Clone)]
pub struct Tok {
    pub kind: TokKind,
    pub start: usize,
    pub end: usize,
}

/// Lex a selector string into tokens.
///
/// Returns `Err` only for unterminated string / regex literals; everything
/// else (unknown characters) is reported by the parser. The lexer keeps
/// [`Whitespace`](TokKind::Whitespace) tokens so the parser can distinguish
/// `A > B` (child) from `A B` (descendant).
pub fn lex(src: &str) -> Result<Vec<Tok>, crate::selector::ast::SelectorError> {
    use crate::selector::ast::SelectorError;

    let chars: Vec<char> = src.chars().collect();
    let mut byte = 0usize; // byte offset in `src`
    let mut idx = 0usize; // char index into `chars`
    let mut out: Vec<Tok> = Vec::new();

    while idx < chars.len() {
        let c = chars[idx];
        let start = byte;

        if c.is_ascii_whitespace() {
            let end_idx = consume_ws(&chars, idx);
            let end = byte_for(src, end_idx);
            out.push(Tok {
                kind: TokKind::Whitespace,
                start,
                end,
            });
            idx = end_idx;
            byte = end;
            continue;
        }

        match c {
            '[' => push_simple(&mut out, TokKind::LBracket, start, byte + c.len_utf8()),
            ']' => push_simple(&mut out, TokKind::RBracket, start, byte + c.len_utf8()),
            '(' => push_simple(&mut out, TokKind::LParen, start, byte + c.len_utf8()),
            ')' => push_simple(&mut out, TokKind::RParen, start, byte + c.len_utf8()),
            '>' => push_simple(&mut out, TokKind::Gt, start, byte + c.len_utf8()),
            ':' => push_simple(&mut out, TokKind::Colon, start, byte + c.len_utf8()),
            ',' => push_simple(&mut out, TokKind::Comma, start, byte + c.len_utf8()),
            '=' => {
                if chars.get(idx + 1) == Some(&'~') {
                    push_simple(
                        &mut out,
                        TokKind::RegexEq,
                        start,
                        byte + "=~".chars().count(),
                    );
                } else {
                    push_simple(&mut out, TokKind::Eq, start, byte + c.len_utf8());
                }
            }
            '"' | '\'' => {
                let (s, end_idx, end_byte) =
                    lex_string(src, &chars, idx, byte, c).ok_or_else(|| SelectorError::Lex {
                        span: start,
                        message: "unterminated string literal".to_owned(),
                    })?;
                out.push(Tok {
                    kind: TokKind::Str(s),
                    start,
                    end: end_byte,
                });
                idx = end_idx;
                byte = end_byte;
                continue;
            }
            '/' => {
                let (s, end_idx, end_byte) =
                    lex_regex(src, &chars, idx, byte).ok_or_else(|| SelectorError::Lex {
                        span: start,
                        message: "unterminated regex literal".to_owned(),
                    })?;
                out.push(Tok {
                    kind: TokKind::Regex(s),
                    start,
                    end: end_byte,
                });
                idx = end_idx;
                byte = end_byte;
                continue;
            }
            _ if is_ident_start(c) => {
                let (s, end_idx, end_byte) = lex_ident(src, &chars, idx, byte);
                out.push(Tok {
                    kind: TokKind::Ident(s),
                    start,
                    end: end_byte,
                });
                idx = end_idx;
                byte = end_byte;
                continue;
            }
            _ => {
                return Err(SelectorError::Lex {
                    span: start,
                    message: format!("unexpected character `{c}`"),
                });
            }
        }

        // `push_simple` consumers advance manually here.
        let adv = out.last().unwrap().end - out.last().unwrap().start;
        // Char delta — only valid because all "simple" punctuation above is
        // ASCII (1 byte / 1 char).
        idx += adv;
        byte += adv;
    }

    Ok(out)
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_cont(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.'
}

/// Consume a run of ASCII whitespace starting at `idx`; return the index of
/// the first non-whitespace char (== `chars.len()` at EOF).
fn consume_ws(chars: &[char], mut idx: usize) -> usize {
    while idx < chars.len() && chars[idx].is_ascii_whitespace() {
        idx += 1;
    }
    idx
}

/// Lex an identifier. The byte offset of the first non-ident char is
/// returned alongside the sliced string.
fn lex_ident(src: &str, chars: &[char], mut idx: usize, mut byte: usize) -> (String, usize, usize) {
    let start_byte = byte;
    while idx < chars.len() && is_ident_cont(chars[idx]) {
        byte += chars[idx].len_utf8();
        idx += 1;
    }
    let s = src[start_byte..byte].to_owned();
    (s, idx, byte)
}

/// Lex a quoted string literal with delimiter `quote`. Handles `\\` escapes
/// (any char following a backslash is taken literally, including the quote
/// itself). Returns `(content, next_idx, end_byte)` or `None` if EOF
/// reached before the closing quote.
fn lex_string(
    _src: &str,
    chars: &[char],
    mut idx: usize,
    mut byte: usize,
    quote: char,
) -> Option<(String, usize, usize)> {
    let start_byte = byte;
    // skip opening quote
    byte += quote.len_utf8();
    idx += 1;
    let content_start = byte;
    let mut content = String::new();
    while idx < chars.len() {
        let c = chars[idx];
        if c == '\\' {
            let next = chars.get(idx + 1)?;
            // Keep the escaped char verbatim (so `\"` -> `"`), drop the slash.
            content.push(*next);
            byte += c.len_utf8() + next.len_utf8();
            idx += 2;
            continue;
        }
        if c == quote {
            byte += c.len_utf8();
            idx += 1;
            let _ = start_byte; // unused but kept for clarity
            let _ = content_start; // (the slice would double-count escapes)
            return Some((content, idx, byte));
        }
        content.push(c);
        byte += c.len_utf8();
        idx += 1;
    }
    None
}

/// Lex a `/.../ ` regex literal. `\\` escapes the next char (so `\/` is a
/// literal slash within the pattern, not the closing delimiter). Returns
/// `(pattern, next_idx, end_byte)` or `None` if EOF before the closing `/`.
fn lex_regex(
    _src: &str,
    chars: &[char],
    mut idx: usize,
    mut byte: usize,
) -> Option<(String, usize, usize)> {
    // skip opening slash
    byte += '/'.len_utf8();
    idx += 1;
    let mut pattern = String::new();
    while idx < chars.len() {
        let c = chars[idx];
        if c == '\\' {
            let next = chars.get(idx + 1)?;
            // Keep both the backslash and the escaped char — the `regex`
            // crate wants the raw pattern (e.g. `\d`, `\/`).
            pattern.push('\\');
            pattern.push(*next);
            byte += c.len_utf8() + next.len_utf8();
            idx += 2;
            continue;
        }
        if c == '/' {
            byte += c.len_utf8();
            idx += 1;
            return Some((pattern, idx, byte));
        }
        pattern.push(c);
        byte += c.len_utf8();
        idx += 1;
    }
    None
}

/// Get the byte offset in `src` corresponding to char index `idx`.
fn byte_for(src: &str, idx: usize) -> usize {
    src.char_indices()
        .nth(idx)
        .map(|(b, _)| b)
        .unwrap_or(src.len())
}

fn push_simple(out: &mut Vec<Tok>, kind: TokKind, start: usize, end: usize) {
    out.push(Tok { kind, start, end });
}
