mod common;

use common::{ACTOR_A, AUTHOR, T0, new_log};
use wb_editlog::{EditLog, FixedClock, SaveOptions, Value};

#[test]
#[cfg_attr(debug_assertions, ignore = "release-only performance guard")]
fn loading_100k_ops_is_within_budget() {
    let mut log = new_log();
    let mut tx = log.transact("seed");
    let e = tx.create("test.counter", [("n", Value::Int(0))]);
    tx.commit().unwrap();
    for i in 1..100_000i64 {
        let mut tx = log.transact("bump");
        tx.set(e, "n", i);
        tx.commit().unwrap();
    }
    let bytes = log.to_bytes_with(SaveOptions { snapshot: false });

    let start = std::time::Instant::now();
    let loaded = EditLog::from_bytes(&bytes, ACTOR_A, AUTHOR, Box::new(FixedClock(T0))).unwrap();
    let elapsed_ms = start.elapsed().as_millis();

    assert_eq!(loaded.source_hash(), log.source_hash());
    let budget_ms = if cfg!(target_family = "wasm") {
        3000
    } else {
        1000
    };
    assert!(
        elapsed_ms <= budget_ms,
        "loaded 100k ops in {elapsed_ms} ms (budget {budget_ms} ms)"
    );
}
