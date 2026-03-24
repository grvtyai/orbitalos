use std::fmt;

use crate::error::{OrbitalError, OrbitalResult};

pub const SNAPSHOT_ENTITY_TYPE: &str = "snapshot";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SnapshotId(String);

impl SnapshotId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for SnapshotId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotKind {
    Link,
    Image,
    File,
    Text,
}

impl SnapshotKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Link => "link",
            Self::Image => "image",
            Self::File => "file",
            Self::Text => "text",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Link => "Link",
            Self::Image => "Image",
            Self::File => "File",
            Self::Text => "Text",
        }
    }

    pub fn from_stored(value: &str) -> OrbitalResult<Self> {
        match value {
            "link" => Ok(Self::Link),
            "image" => Ok(Self::Image),
            "file" => Ok(Self::File),
            "text" => Ok(Self::Text),
            _ => Err(OrbitalError::DataInvariant(
                "snapshot kind stored in database is invalid",
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSnapshot {
    pub id: SnapshotId,
    pub title: String,
    pub kind: SnapshotKind,
    pub source: Option<String>,
    pub file_path: Option<String>,
    pub mime_type: Option<String>,
    pub tags: Vec<String>,
}

impl NewSnapshot {
    pub fn new(id: SnapshotId, title: impl Into<String>, kind: SnapshotKind) -> Self {
        Self {
            id,
            title: title.into(),
            kind,
            source: None,
            file_path: None,
            mime_type: None,
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotSummary {
    pub id: SnapshotId,
    pub title: String,
    pub kind: SnapshotKind,
    pub source: Option<String>,
    pub file_path: Option<String>,
    pub mime_type: Option<String>,
    pub tags: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}
