use crate::error::EditError;
use crate::ids::{AuthorId, KIND, OpId, VersionId};
use crate::log::{Branch, EditLog, Version, clip_label};
use crate::op::{FieldWrite, TxKind};
use crate::state::{State, materialize, reachable};
use crate::value::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Where a new branch starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ForkFrom {
    Current,
    Version(VersionId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimelineEntry {
    pub op: OpId,
    pub kind: TxKind,
    pub label: String,
    pub author: AuthorId,
    pub time_ms: u64,
    pub undone: bool,
    pub versions: Vec<String>,
    pub forks: Vec<String>,
}

pub(crate) fn check_name(name: &str) -> Result<(), EditError> {
    if name.is_empty() || name.len() > 64 || name.chars().any(char::is_control) {
        return Err(EditError::InvalidName(name.to_string()));
    }
    Ok(())
}

impl EditLog {
    pub fn fork(&mut self, name: &str, from: ForkFrom) -> Result<(), EditError> {
        check_name(name)?;
        if self.branches.contains_key(name) {
            return Err(EditError::BranchExists(name.to_string()));
        }
        let heads = match from {
            ForkFrom::Current => self.current_branch_ref().heads.clone(),
            ForkFrom::Version(v) => self
                .versions
                .get(&v)
                .ok_or(EditError::UnknownVersion(v))?
                .heads
                .clone(),
        };
        self.branches.insert(
            name.to_string(),
            Branch {
                heads: heads.clone(),
                base: heads,
                undo: Vec::new(),
                redo: Vec::new(),
            },
        );
        Ok(())
    }

    pub fn switch(&mut self, name: &str) -> Result<(), EditError> {
        let heads = self
            .branches
            .get(name)
            .ok_or_else(|| EditError::UnknownBranch(name.to_string()))?
            .heads
            .clone();
        self.current = name.to_string();
        self.state = materialize(&self.ops, &heads);
        Ok(())
    }

    pub fn branches(&self) -> impl Iterator<Item = (&str, &Branch)> {
        self.branches.iter().map(|(n, b)| (n.as_str(), b))
    }

    pub fn branch(&self, name: &str) -> Option<&Branch> {
        self.branches.get(name)
    }

    pub fn save_version(&mut self, name: &str) -> Result<VersionId, EditError> {
        check_name(name)?;
        let id = VersionId {
            actor: self.actor,
            seq: self.next_version_seq,
        };
        self.next_version_seq += 1;
        let time_ms = self.clock.now_ms();
        let version = Version {
            name: name.to_string(),
            heads: self.current_branch_ref().heads.clone(),
            branch: self.current.clone(),
            time_ms,
            author: self.author,
        };
        self.versions.insert(id, version);
        self.modified_ms = time_ms;
        Ok(id)
    }

    pub fn versions(&self) -> impl Iterator<Item = (&VersionId, &Version)> {
        self.versions.iter()
    }

    pub fn view_version(&self, id: VersionId) -> Result<State, EditError> {
        let v = self
            .versions
            .get(&id)
            .ok_or(EditError::UnknownVersion(id))?;
        Ok(materialize(&self.ops, &v.heads))
    }

    /// Commits one op that turns the current state into the version's state.
    pub fn restore_version(&mut self, id: VersionId) -> Result<OpId, EditError> {
        let target = self.view_version(id)?;
        let name = self
            .versions
            .get(&id)
            .map(|v| v.name.clone())
            .unwrap_or_default();
        let mut writes: BTreeMap<(crate::ids::EntityId, crate::ids::FieldKey), Value> =
            BTreeMap::new();
        let ids: BTreeSet<_> = target
            .entities()
            .map(|e| e.id)
            .chain(self.state.entities().map(|e| e.id))
            .collect();
        for eid in ids {
            let empty = crate::state::Fields::new();
            let tf = target.entity(eid).map(|e| e.fields).unwrap_or(&empty);
            let cf = self.state.entity(eid).map(|e| e.fields).unwrap_or(&empty);
            for (k, v) in tf {
                if cf.get(k) != Some(v) {
                    writes.insert((eid, k.clone()), v.clone());
                }
            }
            for k in cf.keys() {
                if !tf.contains_key(k) {
                    writes.insert((eid, k.clone()), Value::Null);
                }
            }
        }
        writes.retain(|(eid, k), _| !(*eid == crate::ids::EntityId::PLANET && k.as_str() == KIND));
        let writes = writes
            .into_iter()
            .map(|((entity, field), value)| FieldWrite {
                entity,
                field,
                value,
            })
            .collect();
        let op_id = self.next_id();
        self.commit_op(
            op_id,
            TxKind::Restore(id),
            clip_label(format!("Restore: {name}")),
            writes,
        )
    }

    pub fn timeline(&self, branch: &str) -> Result<Vec<TimelineEntry>, EditError> {
        let b = self
            .branches
            .get(branch)
            .ok_or_else(|| EditError::UnknownBranch(branch.to_string()))?;
        let ids = reachable(&self.ops, &b.heads);
        let mut undone: BTreeMap<OpId, bool> = BTreeMap::new();
        for id in &ids {
            match self.ops[id].kind {
                TxKind::Undo(x) => {
                    undone.insert(x, true);
                }
                TxKind::Redo(x) => {
                    undone.insert(x, false);
                }
                _ => {}
            }
        }
        Ok(ids
            .iter()
            .map(|id| {
                let op = &self.ops[id];
                TimelineEntry {
                    op: *id,
                    kind: op.kind.clone(),
                    label: op.label.clone(),
                    author: op.author,
                    time_ms: op.time_ms,
                    undone: undone.get(id).copied().unwrap_or(false),
                    versions: self
                        .versions
                        .values()
                        .filter(|v| v.heads.contains(id))
                        .map(|v| v.name.clone())
                        .collect(),
                    forks: self
                        .branches
                        .iter()
                        .filter(|(_, br)| br.base.contains(id))
                        .map(|(n, _)| n.clone())
                        .collect(),
                }
            })
            .collect())
    }
}
