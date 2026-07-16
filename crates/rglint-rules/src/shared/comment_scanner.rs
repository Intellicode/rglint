use rglint_core::{SourceFile, Span};

pub struct Comment {
    pub span: Span,
    pub text: String,
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
                    let span = Span::new(
                        prev_newline_plus1 + hash_byte,
                        line.len() - hash_byte,
                    );
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
