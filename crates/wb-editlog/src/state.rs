use crate::ids::{DELETED, EntityId, FieldKey, KIND, OpId};
use crate::op::Op;
use crate::value::Value;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use wb_world::SourceHash;

pub type Fields = BTreeMap<FieldKey, Value>;

/// Materialized world state: entity → fields, all sorted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    entities: BTreeMap<EntityId, Fields>,
}

/// A read-only view of one entity.
#[derive(Clone, Copy, Debug)]
pub struct EntityView<'a> {
    pub id: EntityId,
    pub fields: &'a Fields,
}

impl<'a> EntityView<'a> {
    pub fn kind(&self) -> Option<&'a str> {
        match self.fields.get(KIND) {
            Some(Value::Text(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn is_deleted(&self) -> bool {
        self.fields.get(DELETED) == Some(&Value::Bool(true))
    }

    pub fn get(&self, field: &str) -> Option<&'a Value> {
        self.fields.get(field)
    }
}

impl Default for State {
    fn default() -> Self {
        State::new()
    }
}

impl State {
    /// A state containing only the implicit planet.
    pub fn new() -> Self {
        let mut planet = Fields::new();
        planet.insert(FieldKey::reserved(KIND), Value::Text("planet".to_string()));
        let mut entities = BTreeMap::new();
        entities.insert(EntityId::PLANET, planet);
        State { entities }
    }

    /// Applies an op's writes. Callers apply ops in ascending `OpId` order (last writer wins).
    pub fn apply(&mut self, op: &Op) {
        for w in &op.writes {
            self.write(w.entity, &w.field, &w.value);
        }
    }

    pub(crate) fn write(&mut self, entity: EntityId, field: &FieldKey, value: &Value) {
        if entity == EntityId::PLANET && field.as_str() == KIND {
            return;
        }
        if *value == Value::Null {
            if let Some(fields) = self.entities.get_mut(&entity) {
                fields.remove(field.as_str());
                if fields.is_empty() && entity != EntityId::PLANET {
                    self.entities.remove(&entity);
                }
            }
        } else {
            self.entities
                .entry(entity)
                .or_default()
                .insert(field.clone(), value.clone());
        }
    }

    pub fn entity(&self, id: EntityId) -> Option<EntityView<'_>> {
        self.entities
            .get(&id)
            .map(|fields| EntityView { id, fields })
    }

    pub fn field(&self, id: EntityId, field: &str) -> Option<&Value> {
        self.entities.get(&id).and_then(|f| f.get(field))
    }

    /// Every entity, including deleted ones, in `EntityId` order.
    pub fn entities(&self) -> impl Iterator<Item = EntityView<'_>> {
        self.entities
            .iter()
            .map(|(id, fields)| EntityView { id: *id, fields })
    }

    /// Non-deleted entities, in `EntityId` order.
    pub fn live(&self) -> impl Iterator<Item = EntityView<'_>> {
        self.entities().filter(|e| !e.is_deleted())
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Always false: the planet always exists.
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// `blake3("wb-source-v1" ‖ postcard(state))` — state, not history.
    pub fn source_hash(&self) -> SourceHash {
        let bytes = postcard::to_allocvec(self).expect("state is always serializable");
        let mut h = blake3::Hasher::new();
        h.update(b"wb-source-v1");
        h.update(&bytes);
        SourceHash(h.finalize().into())
    }
}

/// `heads` and all their ancestors that exist in `ops`.
pub fn reachable(ops: &BTreeMap<OpId, Op>, heads: &BTreeSet<OpId>) -> BTreeSet<OpId> {
    let mut seen = BTreeSet::new();
    let mut stack: Vec<OpId> = heads.iter().copied().collect();
    while let Some(id) = stack.pop() {
        if let Some(op) = ops.get(&id)
            && seen.insert(id)
        {
            stack.extend(op.parents.iter().copied());
        }
    }
    seen
}

/// Last-writer-wins state over the ops reachable from `heads`.
pub fn materialize(ops: &BTreeMap<OpId, Op>, heads: &BTreeSet<OpId>) -> State {
    let mut state = State::new();
    for id in reachable(ops, heads) {
        if let Some(op) = ops.get(&id) {
            state.apply(op);
        }
    }
    state
}
