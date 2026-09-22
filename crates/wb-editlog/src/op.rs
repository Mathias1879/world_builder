use crate::ids::{AuthorId, EntityId, FieldKey, OpId, VersionId};
use crate::value::Value;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_LABEL_BYTES: usize = 200;

/// Why an op exists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TxKind {
    Edit,
    Undo(OpId),
    Redo(OpId),
    Restore(VersionId),
    Import,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FieldWrite {
    pub entity: EntityId,
    pub field: FieldKey,
    pub value: Value,
}

/// One transaction = one undo step. `writes` is sorted by `(entity, field)` with no duplicates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Op {
    pub id: OpId,
    pub parents: BTreeSet<OpId>,
    pub author: AuthorId,
    pub time_ms: u64,
    pub kind: TxKind,
    pub label: String,
    pub writes: Vec<FieldWrite>,
}
