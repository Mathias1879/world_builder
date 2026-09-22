use crate::error::EditError;
use core::borrow::Borrow;
use core::fmt;
use serde::{Deserialize, Serialize};

/// Reserved field holding an entity's kind.
pub const KIND: &str = "kind";
/// Reserved field marking an entity deleted (`Bool(true)`).
pub const DELETED: &str = "deleted";

/// One device/session. Supplied by the caller; the log never generates randomness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ActorId(pub [u8; 16]);

/// The human author of an op.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AuthorId(pub [u8; 16]);

/// Totally ordered by `lamport`, then `actor` (field order defines `Ord`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct OpId {
    pub lamport: u64,
    pub actor: ActorId,
}

/// The `n`-th entity created by transaction `op`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityId {
    pub op: OpId,
    pub n: u32,
}

impl EntityId {
    /// The implicit, undeletable planet entity.
    pub const PLANET: EntityId = EntityId {
        op: OpId {
            lamport: 0,
            actor: ActorId([0; 16]),
        },
        n: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VersionId {
    pub actor: ActorId,
    pub seq: u32,
}

/// blake3 of an asset's bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AssetRef(pub [u8; 32]);

fn check_key(s: &str) -> Result<(), EditError> {
    let ok = !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.');
    if ok {
        Ok(())
    } else {
        Err(EditError::InvalidKey(s.to_string()))
    }
}

macro_rules! key_type {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub fn new(s: &str) -> Result<Self, EditError> {
                check_key(s)?;
                Ok(Self(s.to_string()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Crate-internal constructor for reserved, known-valid keys.
            #[allow(dead_code)]
            pub(crate) fn reserved(s: &'static str) -> Self {
                Self(s.to_string())
            }
        }

        impl TryFrom<String> for $name {
            type Error = EditError;
            fn try_from(s: String) -> Result<Self, EditError> {
                check_key(&s)?;
                Ok(Self(s))
            }
        }

        impl From<$name> for String {
            fn from(k: $name) -> String {
                k.0
            }
        }

        impl Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

key_type!(FieldKey, "A field name: 1–64 bytes of `[a-z0-9_.]`.");
key_type!(
    EntityKind,
    "An entity kind, dotted namespace: 1–64 bytes of `[a-z0-9_.]`."
);
