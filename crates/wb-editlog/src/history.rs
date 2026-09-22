use crate::error::EditError;
use crate::log::{EditLog, clip_label};
use crate::op::{FieldWrite, TxKind};
use crate::state::materialize;
use crate::value::Value;

impl EditLog {
    pub fn can_undo(&self) -> bool {
        !self.current_branch_ref().undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.current_branch_ref().redo.is_empty()
    }

    /// Reverts the latest undoable transaction on the current branch by committing
    /// a new op that writes back each changed field's value from before it.
    pub fn undo(&mut self) -> Result<crate::ids::OpId, EditError> {
        let x = *self
            .current_branch_ref()
            .undo
            .last()
            .ok_or(EditError::NothingToUndo)?;
        let target = self
            .ops
            .get(&x)
            .expect("undo stack only holds known ops")
            .clone();
        let before = materialize(&self.ops, &target.parents);
        let writes = target
            .writes
            .iter()
            .map(|w| FieldWrite {
                entity: w.entity,
                field: w.field.clone(),
                value: before
                    .field(w.entity, w.field.as_str())
                    .cloned()
                    .unwrap_or(Value::Null),
            })
            .collect();
        let id = self.next_id();
        self.commit_op(
            id,
            TxKind::Undo(x),
            clip_label(format!("Undo: {}", target.label)),
            writes,
        )?;
        let cur = self.current.clone();
        let branch = self
            .branches
            .get_mut(&cur)
            .expect("current branch always exists");
        branch.undo.pop();
        branch.redo.push(x);
        Ok(id)
    }

    /// Re-applies the most recently undone transaction on the current branch.
    pub fn redo(&mut self) -> Result<crate::ids::OpId, EditError> {
        let x = *self
            .current_branch_ref()
            .redo
            .last()
            .ok_or(EditError::NothingToRedo)?;
        let target = self
            .ops
            .get(&x)
            .expect("redo stack only holds known ops")
            .clone();
        let id = self.next_id();
        self.commit_op(
            id,
            TxKind::Redo(x),
            clip_label(format!("Redo: {}", target.label)),
            target.writes,
        )?;
        let cur = self.current.clone();
        let branch = self
            .branches
            .get_mut(&cur)
            .expect("current branch always exists");
        branch.redo.pop();
        branch.undo.push(x);
        Ok(id)
    }
}
