//! Edit Log: the operation graph that is the only durable truth about a world.

mod error;
mod ids;
mod value;

pub use error::EditError;
pub use ids::{
    ActorId, AssetRef, AuthorId, DELETED, EntityId, EntityKind, FieldKey, KIND, OpId, VersionId,
};
pub use value::{MAX_LIST_DEPTH, MAX_TEXT_BYTES, Value, wrap_lon};
