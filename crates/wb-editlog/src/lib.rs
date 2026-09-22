//! Edit Log: the operation graph that is the only durable truth about a world.

mod error;
mod ids;

pub use error::EditError;
pub use ids::{
    ActorId, AssetRef, AuthorId, DELETED, EntityId, EntityKind, FieldKey, KIND, OpId, VersionId,
};
