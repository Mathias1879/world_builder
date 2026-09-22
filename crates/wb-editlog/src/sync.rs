use crate::error::EditError;
use crate::ids::OpId;
use crate::log::EditLog;
use crate::op::{MAX_LABEL_BYTES, Op};
use crate::state::{materialize, reachable};
use std::collections::{BTreeMap, BTreeSet};

fn corrupt() -> EditError {
    EditError::CorruptFile {
        section: "OPS".to_string(),
    }
}

/// Structural checks every received or loaded op must pass.
pub(crate) fn check_op_shape(op: &Op) -> Result<(), EditError> {
    if op.writes.is_empty() || op.label.len() > MAX_LABEL_BYTES {
        return Err(corrupt());
    }
    for pair in op.writes.windows(2) {
        if (pair[0].entity, &pair[0].field) >= (pair[1].entity, &pair[1].field) {
            return Err(corrupt());
        }
    }
    for w in &op.writes {
        if w.value.clone().canonical(w.field.as_str()).as_ref() != Ok(&w.value) {
            return Err(corrupt());
        }
    }
    Ok(())
}

impl EditLog {
    /// Every op not reachable from `heads`, ascending by `OpId`.
    pub fn ops_since(&self, heads: &BTreeSet<OpId>) -> Vec<Op> {
        let known = reachable(&self.ops, heads);
        self.ops
            .values()
            .filter(|op| !known.contains(&op.id))
            .cloned()
            .collect()
    }

    /// Adds ops received from storage or another actor. Atomic: all or nothing.
    /// Received ops extend the current branch: its heads become the newest ops of (current heads ∪ incoming leaves).
    pub fn apply_ops(&mut self, ops: Vec<Op>) -> Result<(), EditError> {
        let mut incoming: BTreeMap<OpId, Op> = BTreeMap::new();
        for op in ops {
            if self.ops.contains_key(&op.id) || incoming.contains_key(&op.id) {
                return Err(EditError::DuplicateOp(op.id));
            }
            incoming.insert(op.id, op);
        }
        for op in incoming.values() {
            check_op_shape(op)?;
            for p in &op.parents {
                if !self.ops.contains_key(p) && !incoming.contains_key(p) {
                    return Err(EditError::UnknownParent(*p));
                }
                if p.lamport >= op.id.lamport {
                    return Err(corrupt());
                }
            }
        }
        if let Some(max) = incoming.keys().map(|id| id.lamport).max() {
            self.max_lamport = self.max_lamport.max(max);
        }

        // Compute incoming leaves before extending ops
        let incoming_parents: BTreeSet<OpId> = incoming
            .values()
            .flat_map(|op| op.parents.iter().copied())
            .collect();
        let incoming_leaves: BTreeSet<OpId> = incoming
            .keys()
            .filter(|id| !incoming_parents.contains(id))
            .copied()
            .collect();

        self.ops.extend(incoming);
        let cur = self.current.clone();
        let branch = self
            .branches
            .get_mut(&cur)
            .expect("current branch always exists");
        let mut heads: BTreeSet<OpId> = branch
            .heads
            .iter()
            .copied()
            .chain(incoming_leaves)
            .collect();
        let parents: BTreeSet<OpId> = heads
            .iter()
            .flat_map(|h| self.ops[h].parents.iter().copied())
            .collect();
        let ancestors = reachable(&self.ops, &parents);
        heads.retain(|h| !ancestors.contains(h));
        branch.heads = heads.clone();
        self.state = materialize(&self.ops, &heads);
        Ok(())
    }
}
