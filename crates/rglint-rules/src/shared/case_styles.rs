use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseStyle {
    Camel,
    Pascal,
    StrictPascal,
    Snake,
    ScreamingSnake,
    Kebab,
    ScreamingKebab,
}

impl CaseStyle {
    pub fn all() -> &'static [CaseStyle] {
        &[
            CaseStyle::Camel,
            CaseStyle::Pascal,
            CaseStyle::StrictPascal,
            CaseStyle::Snake,
            CaseStyle::ScreamingSnake,
            CaseStyle::Kebab,
            CaseStyle::ScreamingKebab,
        ]
    }
}

impl fmt::Display for CaseStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CaseStyle::Camel => write!(f, "camelCase"),
            CaseStyle::Pascal => write!(f, "PascalCase"),
            CaseStyle::StrictPascal => write!(f, "StrictPascalCase"),
            CaseStyle::Snake => write!(f, "snake_case"),
            CaseStyle::ScreamingSnake => write!(f, "SCREAMING_SNAKE_CASE"),
            CaseStyle::Kebab => write!(f, "kebab-case"),
            CaseStyle::ScreamingKebab => write!(f, "SCREAMING-KEBAB-CASE"),
        }
    }
}
