use crate::error::ConstraintError;
use crate::kinds::{is_constraint_kind, text};
use crate::model::{IssueCode, Suggestion};
use crate::templates::render_suggestion;
use std::collections::BTreeSet;
use wb_editlog::{EditLog, EntityId, OpId, Transaction, Value, clip_label};

/// Commits a suggestion's writes as one transaction.
///
/// Fails with [`ConstraintError::UnknownEntity`] — writing nothing — if any target
/// entity is missing. Deletion in the Edit Log is a tombstone, so a deleted entity is
/// still reachable through `State::entity`; it counts as missing here.
pub fn apply_suggestion(log: &mut EditLog, s: &Suggestion) -> Result<OpId, ConstraintError> {
    if let Some((e, _, _)) = s
        .writes
        .iter()
        .find(|(e, _, _)| log.state().entity(*e).filter(|v| !v.is_deleted()).is_none())
    {
        return Err(ConstraintError::UnknownEntity(*e));
    }
    let label = clip_label(format!("Suggestion: {}", render_suggestion(s)));
    let mut tx = log.transact(&label);
    for (e, field, value) in &s.writes {
        tx.set(*e, field, value.clone());
    }
    Ok(tx.commit()?)
}

/// The constraint's current kept-code set and display name.
///
/// Deletion is a tombstone, so a deleted entity is still reachable through
/// `State::entity`; it is treated as missing here.
fn kept(
    log: &EditLog,
    constraint: EntityId,
) -> Result<(BTreeSet<String>, String), ConstraintError> {
    let view = log
        .state()
        .entity(constraint)
        .filter(|v| !v.is_deleted())
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

/// Writes the kept-code set in its canonical encoding.
///
/// "Nothing kept" is always `Value::Null` for both `keep_codes` and `keep_reason` —
/// never `Value::List([])`, so the two actions cannot disagree about the empty set.
fn write_codes(tx: &mut Transaction<'_>, constraint: EntityId, codes: &BTreeSet<String>) {
    if codes.is_empty() {
        tx.set(constraint, "keep_codes", Value::Null);
        tx.set(constraint, "keep_reason", Value::Null);
    } else {
        tx.set(constraint, "keep_codes", codes_value(codes));
    }
}

/// Unions `codes` into the constraint's `keep_codes`, optionally recording `reason`.
///
/// Writes nothing when the code set is unchanged and no reason is given — the empty
/// transaction then surfaces as [`ConstraintError::Edit`] with `EditError::EmptyTransaction`.
/// A reason is written whenever it is given, even if the code set did not change.
pub fn keep_anyway(
    log: &mut EditLog,
    constraint: EntityId,
    codes: &[IssueCode],
    reason: Option<&str>,
) -> Result<OpId, ConstraintError> {
    let (mut current, display) = kept(log, constraint)?;
    let before = current.len();
    current.extend(codes.iter().map(|c| c.0.clone()));
    let changed = current.len() != before;
    let mut tx = log.transact(&clip_label(format!("Kept anyway: {display}")));
    if changed {
        write_codes(&mut tx, constraint, &current);
    }
    if let Some(r) = reason {
        tx.set(constraint, "keep_reason", r);
    }
    Ok(tx.commit()?)
}

/// Removes `codes` from the constraint's `keep_codes`, clearing `keep_reason` when
/// none remain. Writes nothing when no code was actually kept.
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
    let mut tx = log.transact(&clip_label(format!("Un-kept: {display}")));
    if current.len() != before {
        write_codes(&mut tx, constraint, &current);
    }
    Ok(tx.commit()?)
}

pub fn set_realism(log: &mut EditLog, r: f64) -> Result<OpId, ConstraintError> {
    let value = if r.is_nan() { r } else { r.clamp(0.0, 1.0) };
    let mut tx = log.transact(&format!("Realism: {value:.2}"));
    tx.set(EntityId::PLANET, "realism", value);
    Ok(tx.commit()?)
}
