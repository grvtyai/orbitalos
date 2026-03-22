use std::fmt;

pub const NOTE_ENTITY_TYPE: &str = "note";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NoteId(String);

impl NoteId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for NoteId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for NoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewNote {
    pub id: NoteId,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
}

impl NewNote {
    pub fn new(id: NoteId, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            body: body.into(),
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteSummary {
    pub id: NoteId,
    pub title: String,
    pub excerpt: String,
    pub tags: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteDocument {
    pub summary: NoteSummary,
    pub body: String,
}

pub fn note_excerpt(body: &str) -> String {
    const EXCERPT_LIMIT: usize = 160;

    let compact = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let compact_len = compact.chars().count();

    if compact_len <= EXCERPT_LIMIT {
        compact
    } else {
        let excerpt = compact.chars().take(EXCERPT_LIMIT - 3).collect::<String>();
        format!("{excerpt}...")
    }
}
