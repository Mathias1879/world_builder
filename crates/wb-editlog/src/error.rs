use crate::ids::{AssetRef, EntityId, OpId, VersionId};
use core::fmt;

/// Every failure the edit log can report. The crate never panics on user input.
#[derive(Clone, Debug, PartialEq)]
pub enum EditError {
    InvalidValue {
        field: String,
        reason: String,
    },
    InvalidKey(String),
    InvalidName(String),
    ValidationFailed {
        entity: EntityId,
        kind: String,
        reason: String,
    },
    UnknownEntity(EntityId),
    EmptyTransaction,
    UnknownParent(OpId),
    DuplicateOp(OpId),
    NothingToUndo,
    NothingToRedo,
    BranchExists(String),
    UnknownBranch(String),
    UnknownVersion(VersionId),
    UnknownAsset(AssetRef),
    CorruptFile {
        section: String,
    },
    UnsupportedFormat {
        found: u16,
    },
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditError::InvalidValue { field, reason } => {
                write!(f, "invalid value for '{field}': {reason}")
            }
            EditError::InvalidKey(k) => write!(f, "invalid key '{k}' (1–64 bytes of [a-z0-9_.])"),
            EditError::InvalidName(n) => {
                write!(f, "invalid name '{n}' (1–64 bytes, no control characters)")
            }
            EditError::ValidationFailed {
                entity,
                kind,
                reason,
            } => {
                write!(f, "{kind} {entity:?} failed validation: {reason}")
            }
            EditError::UnknownEntity(e) => write!(f, "unknown entity {e:?}"),
            EditError::EmptyTransaction => f.write_str("transaction has no changes"),
            EditError::UnknownParent(p) => write!(f, "unknown parent op {p:?}"),
            EditError::DuplicateOp(o) => write!(f, "duplicate op {o:?}"),
            EditError::NothingToUndo => f.write_str("nothing to undo"),
            EditError::NothingToRedo => f.write_str("nothing to redo"),
            EditError::BranchExists(b) => write!(f, "branch '{b}' already exists"),
            EditError::UnknownBranch(b) => write!(f, "unknown branch '{b}'"),
            EditError::UnknownVersion(v) => write!(f, "unknown version {v:?}"),
            EditError::UnknownAsset(a) => write!(f, "unknown asset {a:?}"),
            EditError::CorruptFile { section } => {
                write!(f, "project file is corrupt in section {section}")
            }
            EditError::UnsupportedFormat { found } => {
                write!(
                    f,
                    "project file format {found} is newer than this app supports"
                )
            }
        }
    }
}

impl std::error::Error for EditError {}
