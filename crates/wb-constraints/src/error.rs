use core::fmt;
use wb_editlog::{EditError, EntityId};

#[derive(Clone, Debug, PartialEq)]
pub enum ConstraintError {
    DuplicateChecker(String),
    UnknownEntity(EntityId),
    /// The entity has no kind at all. Before `KindCatalog`, this meant "not one of the
    /// fixed `CONSTRAINT_KINDS`"; now that the gate is a registry a subsystem can extend,
    /// a kind outside the catalog is simply not visited by `evaluate` — it is never
    /// rejected here. This variant fires only for the one case a catalog cannot help
    /// with: an entity with no kind whatsoever.
    NotAConstraint(EntityId),
    Edit(EditError),
}

impl From<EditError> for ConstraintError {
    fn from(e: EditError) -> Self {
        ConstraintError::Edit(e)
    }
}

impl fmt::Display for ConstraintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConstraintError::DuplicateChecker(id) => {
                write!(f, "checker '{id}' is already registered")
            }
            ConstraintError::UnknownEntity(e) => write!(f, "unknown entity {e:?}"),
            ConstraintError::NotAConstraint(e) => write!(f, "entity {e:?} is not a constraint"),
            ConstraintError::Edit(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ConstraintError {}
