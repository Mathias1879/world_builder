use crate::error::ConstraintError;
use crate::kinds::{is_constraint_kind, text};
use crate::model::{IssueCode, Suggestion};
use crate::templates::render_suggestion;
use std::collections::BTreeSet;
use wb_editlog::{EditLog, EntityId, MAX_LABEL_BYTES, OpId, Value};

fn clip(s: String) -> String {
    if s.len() <= MAX_LABEL_BYTES {
        return s;
    }
    let mut end = MAX_LABEL_BYTES;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

/// Commits a suggestion's writes as one transaction.
pub fn apply_suggestion(log: &mut EditLog, s: &Suggestion) -> Result<OpId, ConstraintError> {
    if let Some((e, _, _)) = s
        .writes
        .iter()
        .find(|(e, _, _)| log.state().entity(*e).is_none())
    {
        return Err(ConstraintError::UnknownEntity(*e));
    }
    let label = clip(format!("Suggestion: {}", render_suggestion(s)));
    let mut tx = log.transact(&label);
    for (e, field, value) in &s.writes {
        tx.set(*e, field, value.clone());
    }
    Ok(tx.commit()?)
}

fn kept(
    log: &EditLog,
    constraint: EntityId,
) -> Result<(BTreeSet<String>, String), ConstraintError> {
    let view = log
        .state()
        .entity(constraint)
        .ok_or(ConstraintError::UnknownEntity(constraint))?;
    let kind = view.kind().unwrap_or_default();
    if !is_constraint_kind(kind) {
        return Err(ConstraintError::NotAConstraint(constraint));
    }
    let codes = match view.get("keep_codes") {
        Some(Value::List(items)) => items
            .iter()
            .filter_map(|v| match v {
                Value::Text(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        _ => BTreeSet::new(),
    };
    let display = text(&view, "name").unwrap_or(kind).to_string();
    Ok((codes, display))
}

fn codes_value(codes: &BTreeSet<String>) -> Value {
    Value::List(codes.iter().map(|c| Value::Text(c.clone())).collect())
}

pub fn keep_anyway(
    log: &mut EditLog,
    constraint: EntityId,
    codes: &[IssueCode],
    reason: Option<&str>,
) -> Result<OpId, ConstraintError> {
    let (mut current, display) = kept(log, constraint)?;
    current.extend(codes.iter().map(|c| c.0.clone()));
    let mut tx = log.transact(&clip(format!("Kept anyway: {display}")));
    tx.set(constraint, "keep_codes", codes_value(&current));
    if let Some(r) = reason {
        tx.set(constraint, "keep_reason", r);
    }
    Ok(tx.commit()?)
}

pub fn unkeep(
    log: &mut EditLog,
    constraint: EntityId,
    codes: &[IssueCode],
) -> Result<OpId, ConstraintError> {
    let (mut current, display) = kept(log, constraint)?;
    let before = current.len();
    for c in codes {
        current.remove(c.as_str());
    }
    let mut tx = log.transact(&clip(format!("Un-kept: {display}")));
    if current.len() != before {
        if current.is_empty() {
            tx.set(constraint, "keep_codes", Value::Null);
            tx.set(constraint, "keep_reason", Value::Null);
        } else {
            tx.set(constraint, "keep_codes", codes_value(&current));
        }
    }
    Ok(tx.commit()?)
}

pub fn set_realism(log: &mut EditLog, r: f64) -> Result<OpId, ConstraintError> {
    let value = if r.is_nan() { r } else { r.clamp(0.0, 1.0) };
    let mut tx = log.transact(&format!("Realism: {value:.2}"));
    tx.set(EntityId::PLANET, "realism", value);
    Ok(tx.commit()?)
}
