//! Edit Log: the operation graph that is the only durable truth about a world.

mod error;
mod ids;
mod op;
mod state;
mod value;

pub use error::EditError;
pub use ids::{
    ActorId, AssetRef, AuthorId, DELETED, EntityId, EntityKind, FieldKey, KIND, OpId, VersionId,
};
pub use op::{FieldWrite, MAX_LABEL_BYTES, Op, TxKind};
pub use state::{EntityView, Fields, State, materialize, reachable};
pub use value::{MAX_LIST_DEPTH, MAX_TEXT_BYTES, Value, wrap_lon};
