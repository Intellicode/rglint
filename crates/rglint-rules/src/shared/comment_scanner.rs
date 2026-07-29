use rglint_core::{SourceFile, Span};

pub struct Comment {
    pub span: Span,
    pub text: String,
}

/// Return every line comment in a GraphQL source file.
///
/// This intentionally accepts only comments whose `#` is the first
/// non-whitespace character on the line, matching GraphQL's line-comment
/// grammar. Rules that need document-wide directives (for example
/// `require-import-fragment`) can inspect all comments, while
/// [`preceding_comments`] retains its contiguous-comment semantics.
pub fn all_comments(source: &SourceFile) -> Vec<Comment> {
    let mut comments = Vec::new();
    let mut offset = 0;

    for line in source.source().split_inclusive('\n') {
        let line_without_newline = line.strip_suffix('\n').unwrap_or(line);
        if let Some(hash_byte) = line_without_newline.find('#') {
            if line_without_newline[..hash_byte].trim().is_empty() {
                comments.push(Comment {
                    span: Span::new(offset + hash_byte, line_without_newline.len() - hash_byte),
                    text: line_without_newline[hash_byte..].to_owned(),
                });
            }
        }
        offset += line.len();
    }

    comments
}

pub fn preceding_comments(source: &SourceFile, node_span: Span) -> Vec<Comment> {
    let text = source.source();
    let node_start = node_span.offset;

    let node_line_start = match text[..node_start].rfind('\n') {
        Some(i) => i + 1,
        None => 0,
    };

    let mut comments: Vec<Comment> = Vec::new();
    let mut cursor = node_line_start;

    loop {
        if cursor == 0 {
            break;
        }

        let prev_newline_plus1 = match text[..cursor - 1].rfind('\n') {
            Some(i) => i + 1,
            None => 0,
        };

        let line = &text[prev_newline_plus1..cursor - 1];
        let trimmed = line.trim();

        if trimmed.is_empty() {
            break;
        }

        if let Some(hash_byte) = line.find('#') {
            if line[..hash_byte].trim().is_empty() {
                let after_hash = &line[hash_byte + 1..];
                let after_hash_trimmed = after_hash.trim();

                if !after_hash_trimmed.starts_with("import")
                    && !after_hash_trimmed.starts_with("eslint")
                {
                    let span = Span::new(prev_newline_plus1 + hash_byte, line.len() - hash_byte);
                    comments.push(Comment {
                        span,
                        text: line[hash_byte..].to_owned(),
                    });
                }
                cursor = prev_newline_plus1;
                continue;
            }
        }

        break;
    }

    comments.reverse();
    comments
}
