use crate::error::EditError;
use crate::ids::{
    ActorId, AssetRef, AuthorId, DELETED, EntityId, EntityKind, FieldKey, KIND, OpId, VersionId,
};
use crate::op::{FieldWrite, MAX_LABEL_BYTES, Op, TxKind};
use crate::state::{EntityView, Fields, State, materialize};
use crate::value::Value;
use core::fmt;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use wb_world::SourceHash;

/// Wall-clock source (display only; never used for ordering).
pub trait Clock {
    fn now_ms(&self) -> u64;
}

/// A clock that always returns the same time (tests, deterministic tooling).
#[derive(Clone, Copy, Debug)]
pub struct FixedClock(pub u64);

impl Clock for FixedClock {
    fn now_ms(&self) -> u64 {
        self.0
    }
}

/// Checks an entity of one kind after a transaction.
pub trait Validator {
    fn validate(&self, entity: EntityView<'_>) -> Result<(), String>;
}

impl<F: Fn(EntityView<'_>) -> Result<(), String>> Validator for F {
    fn validate(&self, entity: EntityView<'_>) -> Result<(), String> {
        self(entity)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    pub heads: BTreeSet<OpId>,
    /// Heads at the moment the branch was forked (fork point).
    pub base: BTreeSet<OpId>,
    pub undo: Vec<OpId>,
    pub redo: Vec<OpId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Version {
    pub name: String,
    pub heads: BTreeSet<OpId>,
    pub branch: String,
    pub time_ms: u64,
    pub author: AuthorId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub media_type: String,
    pub bytes: Vec<u8>,
}

/// The edit log for one world.
pub struct EditLog {
    pub(crate) actor: ActorId,
    pub(crate) author: AuthorId,
    pub(crate) clock: Box<dyn Clock>,
    pub(crate) ops: BTreeMap<OpId, Op>,
    pub(crate) max_lamport: u64,
    pub(crate) branches: BTreeMap<String, Branch>,
    pub(crate) current: String,
    pub(crate) versions: BTreeMap<VersionId, Version>,
    pub(crate) next_version_seq: u32,
    pub(crate) assets: BTreeMap<AssetRef, Asset>,
    pub(crate) validators: BTreeMap<EntityKind, Box<dyn Validator>>,
    pub(crate) state: State,
    pub(crate) title: String,
    pub(crate) authors: BTreeMap<AuthorId, String>,
    #[allow(dead_code)]
    pub(crate) created_ms: u64,
    pub(crate) modified_ms: u64,
}

impl fmt::Debug for EditLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EditLog")
            .field("title", &self.title)
            .field("current", &self.current)
            .field("ops", &self.ops.len())
            .field("branches", &self.branches.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Truncates a label to `MAX_LABEL_BYTES` at a char boundary (for generated labels).
pub(crate) fn clip_label(s: String) -> String {
    if s.len() <= MAX_LABEL_BYTES {
        return s;
    }
    let mut end = MAX_LABEL_BYTES;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

impl EditLog {
    pub fn new(actor: ActorId, author: AuthorId, clock: Box<dyn Clock>) -> Self {
        let now = clock.now_ms();
        let mut branches = BTreeMap::new();
        branches.insert("main".to_string(), Branch::default());
        EditLog {
            actor,
            author,
            clock,
            ops: BTreeMap::new(),
            max_lamport: 0,
            branches,
            current: "main".to_string(),
            versions: BTreeMap::new(),
            next_version_seq: 0,
            assets: BTreeMap::new(),
            validators: BTreeMap::new(),
            state: State::new(),
            title: String::new(),
            authors: BTreeMap::new(),
            created_ms: now,
            modified_ms: now,
        }
    }

    pub fn register_validator(&mut self, kind: EntityKind, validator: Box<dyn Validator>) {
        self.validators.insert(kind, validator);
    }

    pub fn transact(&mut self, label: &str) -> Transaction<'_> {
        Transaction::new(self, label, TxKind::Edit)
    }

    pub fn import(&mut self, label: &str) -> Transaction<'_> {
        Transaction::new(self, label, TxKind::Import)
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn source_hash(&self) -> SourceHash {
        self.state.source_hash()
    }

    pub fn heads(&self) -> &BTreeSet<OpId> {
        &self.current_branch_ref().heads
    }

    pub fn current_branch(&self) -> &str {
        &self.current
    }

    pub fn op(&self, id: OpId) -> Option<&Op> {
        self.ops.get(&id)
    }

    /// Materializes the state at arbitrary heads (read-only).
    pub fn state_at(&self, heads: &BTreeSet<OpId>) -> Result<State, EditError> {
        if let Some(missing) = heads.iter().find(|h| !self.ops.contains_key(h)) {
            return Err(EditError::UnknownParent(*missing));
        }
        Ok(materialize(&self.ops, heads))
    }

    pub fn add_asset(&mut self, media_type: &str, bytes: Vec<u8>) -> AssetRef {
        let r = AssetRef(*blake3::hash(&bytes).as_bytes());
        self.assets.entry(r).or_insert(Asset {
            media_type: media_type.to_string(),
            bytes,
        });
        r
    }

    pub fn asset(&self, r: AssetRef) -> Option<&[u8]> {
        self.assets.get(&r).map(|a| a.bytes.as_slice())
    }

    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn set_author_name(&mut self, name: &str) {
        self.authors.insert(self.author, name.to_string());
    }

    pub(crate) fn current_branch_ref(&self) -> &Branch {
        self.branches
            .get(&self.current)
            .expect("current branch always exists")
    }

    pub(crate) fn next_id(&self) -> OpId {
        OpId {
            lamport: self.max_lamport + 1,
            actor: self.actor,
        }
    }

    fn validate(&self, writes: &[FieldWrite]) -> Result<(), EditError> {
        let mut touched: BTreeMap<EntityId, Fields> = BTreeMap::new();
        for w in writes {
            let fields = touched.entry(w.entity).or_insert_with(|| {
                self.state
                    .entity(w.entity)
                    .map(|v| v.fields.clone())
                    .unwrap_or_default()
            });
            if w.value == Value::Null {
                fields.remove(w.field.as_str());
            } else {
                fields.insert(w.field.clone(), w.value.clone());
            }
        }
        for (id, fields) in &touched {
            let view = EntityView { id: *id, fields };
            if fields.is_empty() || view.is_deleted() {
                continue;
            }
            let Some(kind) = view.kind() else { continue };
            if let Some(v) = self.validators.get(kind) {
                v.validate(view)
                    .map_err(|reason| EditError::ValidationFailed {
                        entity: *id,
                        kind: kind.to_string(),
                        reason,
                    })?;
            }
        }
        Ok(())
    }

    /// Validates and appends one op on the current branch.
    pub(crate) fn commit_op(
        &mut self,
        id: OpId,
        kind: TxKind,
        label: String,
        writes: Vec<FieldWrite>,
    ) -> Result<OpId, EditError> {
        if writes.is_empty() {
            return Err(EditError::EmptyTransaction);
        }
        if label.len() > MAX_LABEL_BYTES {
            return Err(EditError::InvalidValue {
                field: "label".into(),
                reason: "label exceeds 200 bytes".into(),
            });
        }
        let user_edit = matches!(kind, TxKind::Edit | TxKind::Import);
        for w in &writes {
            if w.entity == EntityId::PLANET && w.field.as_str() == DELETED {
                return Err(EditError::InvalidValue {
                    field: DELETED.into(),
                    reason: "the planet cannot be deleted".into(),
                });
            }
            let known = w.entity == EntityId::PLANET
                || w.entity.op == id
                || self.state.entity(w.entity).is_some();
            if user_edit && !known {
                return Err(EditError::UnknownEntity(w.entity));
            }
        }
        self.validate(&writes)?;

        let time_ms = self.clock.now_ms();
        let cur = self.current.clone();
        let branch = self
            .branches
            .get_mut(&cur)
            .expect("current branch always exists");
        let op = Op {
            id,
            parents: branch.heads.clone(),
            author: self.author,
            time_ms,
            kind,
            label,
            writes,
        };
        branch.heads = BTreeSet::from([id]);
        if matches!(op.kind, TxKind::Edit | TxKind::Import | TxKind::Restore(_)) {
            branch.undo.push(id);
            branch.redo.clear();
        }
        self.state.apply(&op);
        self.max_lamport = id.lamport;
        self.modified_ms = time_ms;
        self.ops.insert(id, op);
        Ok(id)
    }
}

/// Builder for one transaction (one undo step).
pub struct Transaction<'a> {
    log: &'a mut EditLog,
    id: OpId,
    label: String,
    kind: TxKind,
    writes: BTreeMap<(EntityId, FieldKey), Value>,
    created: u32,
    error: Option<EditError>,
}

impl<'a> Transaction<'a> {
    fn new(log: &'a mut EditLog, label: &str, kind: TxKind) -> Self {
        let id = log.next_id();
        Transaction {
            log,
            id,
            label: label.to_string(),
            kind,
            writes: BTreeMap::new(),
            created: 0,
            error: None,
        }
    }

    fn fail(&mut self, e: EditError) {
        if self.error.is_none() {
            self.error = Some(e);
        }
    }

    fn put(&mut self, entity: EntityId, field: FieldKey, value: Value) {
        match value.canonical(field.as_str()) {
            Ok(v) => {
                self.writes.insert((entity, field), v);
            }
            Err(e) => self.fail(e),
        }
    }

    /// Creates a new entity; its id is known before commit.
    pub fn create<'k>(
        &mut self,
        kind: &str,
        fields: impl IntoIterator<Item = (&'k str, Value)>,
    ) -> EntityId {
        let entity = EntityId {
            op: self.id,
            n: self.created,
        };
        self.created += 1;
        match EntityKind::new(kind) {
            Ok(k) => self.put(
                entity,
                FieldKey::reserved(KIND),
                Value::Text(k.as_str().to_string()),
            ),
            Err(e) => self.fail(e),
        }
        for (field, value) in fields {
            self.set(entity, field, value);
        }
        entity
    }

    pub fn set(&mut self, entity: EntityId, field: &str, value: impl Into<Value>) -> &mut Self {
        if field == KIND || field == DELETED {
            self.fail(EditError::InvalidKey(field.to_string()));
            return self;
        }
        match FieldKey::new(field) {
            Ok(k) => self.put(entity, k, value.into()),
            Err(e) => self.fail(e),
        }
        self
    }

    pub fn delete(&mut self, entity: EntityId) -> &mut Self {
        self.put(entity, FieldKey::reserved(DELETED), Value::Bool(true));
        self
    }

    pub fn undelete(&mut self, entity: EntityId) -> &mut Self {
        self.put(entity, FieldKey::reserved(DELETED), Value::Null);
        self
    }

    pub fn commit(self) -> Result<OpId, EditError> {
        if let Some(e) = self.error {
            return Err(e);
        }
        let writes = self
            .writes
            .into_iter()
            .map(|((entity, field), value)| FieldWrite {
                entity,
                field,
                value,
            })
            .collect();
        self.log.commit_op(self.id, self.kind, self.label, writes)
    }
}
