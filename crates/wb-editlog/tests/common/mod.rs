#![allow(dead_code)]
use wb_editlog::{ActorId, AuthorId, EditLog, FixedClock};

pub const ACTOR_A: ActorId = ActorId([1; 16]);
pub const ACTOR_B: ActorId = ActorId([2; 16]);
pub const AUTHOR: AuthorId = AuthorId([9; 16]);
pub const T0: u64 = 1_758_000_000_000;

pub fn new_log() -> EditLog {
    new_log_as(ACTOR_A)
}

pub fn new_log_as(actor: ActorId) -> EditLog {
    EditLog::new(actor, AUTHOR, Box::new(FixedClock(T0)))
}
