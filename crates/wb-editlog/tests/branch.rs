mod common;

use common::new_log;
use wb_editlog::{EditError, EditLog, EntityId, ForkFrom, TxKind, Value};

fn add(log: &mut EditLog, label: &str, h: i64) -> EntityId {
    let mut tx = log.transact(label);
    let e = tx.create("test.thing", [("h", Value::Int(h))]);
    tx.commit().unwrap();
    e
}

fn set(log: &mut EditLog, e: EntityId, h: i64) {
    let mut tx = log.transact("set");
    tx.set(e, "h", h);
    tx.commit().unwrap();
}

#[test]
fn forks_are_isolated() {
    let mut log = new_log();
    let e = add(&mut log, "base", 1);
    log.fork("what-if", ForkFrom::Current).unwrap();
    assert_eq!(log.current_branch(), "main");
    set(&mut log, e, 2);

    log.switch("what-if").unwrap();
    assert_eq!(log.state().field(e, "h"), Some(&Value::Int(1)));
    assert!(
        !log.can_undo(),
        "a new branch starts with an empty undo stack"
    );
    set(&mut log, e, 3);

    log.switch("main").unwrap();
    assert_eq!(log.state().field(e, "h"), Some(&Value::Int(2)));
    assert_eq!(
        log.branches().map(|(n, _)| n).collect::<Vec<_>>(),
        vec!["main", "what-if"]
    );

    assert_eq!(
        log.fork("what-if", ForkFrom::Current).unwrap_err(),
        EditError::BranchExists("what-if".into())
    );
    assert_eq!(
        log.switch("nope").unwrap_err(),
        EditError::UnknownBranch("nope".into())
    );
    assert!(matches!(
        log.fork("", ForkFrom::Current).unwrap_err(),
        EditError::InvalidName(_)
    ));
    assert!(matches!(
        log.fork("a\nb", ForkFrom::Current).unwrap_err(),
        EditError::InvalidName(_)
    ));
}

#[test]
fn versions_view_restore_and_undo_restore() {
    let mut log = new_log();
    let e = add(&mut log, "base", 1);
    let v = log.save_version("Before the Titan War").unwrap();
    let hv = log.source_hash();
    set(&mut log, e, 7);
    let f = add(&mut log, "extra", 5);
    let before_restore = log.source_hash();

    let viewed = log.view_version(v).unwrap();
    assert_eq!(viewed.source_hash(), hv);
    assert_eq!(log.source_hash(), before_restore, "viewing is read-only");

    let r = log.restore_version(v).unwrap();
    assert_eq!(log.source_hash(), hv);
    assert!(log.state().entity(f).is_none());
    assert_eq!(log.op(r).unwrap().kind, TxKind::Restore(v));
    assert_eq!(log.op(r).unwrap().label, "Restore: Before the Titan War");
    assert_eq!(
        log.restore_version(v).unwrap_err(),
        EditError::EmptyTransaction
    );

    log.undo().unwrap();
    assert_eq!(log.source_hash(), before_restore);

    let (id, meta) = log.versions().next().unwrap();
    assert_eq!(
        (*id, meta.name.as_str(), meta.branch.as_str()),
        (v, "Before the Titan War", "main")
    );
    let missing = wb_editlog::VersionId {
        actor: wb_editlog::ActorId([8; 16]),
        seq: 0,
    };
    assert_eq!(
        log.view_version(missing).unwrap_err(),
        EditError::UnknownVersion(missing)
    );
}

#[test]
fn fork_from_a_version() {
    let mut log = new_log();
    let e = add(&mut log, "base", 1);
    let v = log.save_version("v1").unwrap();
    set(&mut log, e, 9);
    log.fork("from-v1", ForkFrom::Version(v)).unwrap();
    log.switch("from-v1").unwrap();
    assert_eq!(log.state().field(e, "h"), Some(&Value::Int(1)));
}

#[test]
fn timeline_marks_undone_versions_and_forks() {
    let mut log = new_log();
    let e = add(&mut log, "base", 1);
    let base_op = e.op;
    log.save_version("v1").unwrap();
    log.fork("alt", ForkFrom::Current).unwrap();
    set(&mut log, e, 2);
    log.undo().unwrap();

    let t = log.timeline("main").unwrap();
    assert_eq!(t.len(), 3);
    assert_eq!(t[0].op, base_op);
    assert_eq!(t[0].versions, vec!["v1".to_string()]);
    assert_eq!(t[0].forks, vec!["alt".to_string()]);
    assert!(t[1].undone, "the set was undone");
    assert!(matches!(t[2].kind, TxKind::Undo(_)));
    assert!(t.windows(2).all(|w| w[0].op < w[1].op));
    assert_eq!(
        log.timeline("nope").unwrap_err(),
        EditError::UnknownBranch("nope".into())
    );
}
