use crate::branch::check_name;
use crate::error::EditError;
use crate::ids::{ActorId, AssetRef, AuthorId, OpId, VersionId};
use crate::log::{Asset, Branch, Clock, EditLog, Version};
use crate::op::Op;
use crate::state::{State, materialize};
use crate::sync::check_op_shape;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const FORMAT_VERSION: u16 = 1;
const MAGIC: &[u8; 4] = b"WBW1";
const SECTION_HEADER: usize = 4 + 8 + 32;

#[derive(Clone, Copy, Debug)]
pub struct SaveOptions {
    /// Include the materialized state of the current branch for instant loading.
    pub snapshot: bool,
}

impl Default for SaveOptions {
    fn default() -> Self {
        SaveOptions { snapshot: true }
    }
}

#[derive(Serialize, Deserialize)]
struct Meta {
    title: String,
    authors: BTreeMap<AuthorId, String>,
    branches: BTreeMap<String, Branch>,
    versions: BTreeMap<VersionId, Version>,
    current_branch: String,
    created_ms: u64,
    modified_ms: u64,
}

#[derive(Serialize, Deserialize)]
struct Snapshot {
    heads: BTreeSet<OpId>,
    state: State,
}

fn corrupt(section: &str) -> EditError {
    EditError::CorruptFile {
        section: section.to_string(),
    }
}

fn encode<T: Serialize>(value: &T) -> Vec<u8> {
    postcard::to_allocvec(value).expect("in-memory values always serialize")
}

fn decode<T: DeserializeOwned>(payload: &[u8], section: &str) -> Result<T, EditError> {
    let (value, rest) = postcard::take_from_bytes::<T>(payload).map_err(|_| corrupt(section))?;
    if !rest.is_empty() {
        return Err(corrupt(section));
    }
    Ok(value)
}

fn write_section(out: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(tag);
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(blake3::hash(payload).as_bytes());
    out.extend_from_slice(payload);
}

fn read_section<'a>(
    buf: &'a [u8],
    pos: &mut usize,
    tag: &[u8; 4],
    name: &str,
) -> Result<&'a [u8], EditError> {
    let head_end = pos
        .checked_add(SECTION_HEADER)
        .ok_or_else(|| corrupt(name))?;
    let head = buf.get(*pos..head_end).ok_or_else(|| corrupt(name))?;
    if &head[..4] != tag {
        return Err(corrupt(name));
    }
    let len = u64::from_le_bytes(head[4..12].try_into().expect("slice of 8 bytes"));
    let len = usize::try_from(len).map_err(|_| corrupt(name))?;
    let start = pos
        .checked_add(SECTION_HEADER)
        .ok_or_else(|| corrupt(name))?;
    let end = start.checked_add(len).ok_or_else(|| corrupt(name))?;
    let payload = buf.get(start..end).ok_or_else(|| corrupt(name))?;
    if blake3::hash(payload).as_bytes()[..] != head[12..44] {
        return Err(corrupt(name));
    }
    *pos = end;
    Ok(payload)
}

impl EditLog {
    pub fn to_bytes(&self) -> Vec<u8> {
        self.to_bytes_with(SaveOptions::default())
    }

    pub fn to_bytes_with(&self, options: SaveOptions) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        let engine = wb_world::ENGINE_VERSION.as_bytes();
        out.push(u8::try_from(engine.len()).expect("engine version shorter than 256 bytes"));
        out.extend_from_slice(engine);

        let ops: Vec<&Op> = self.ops.values().collect();
        write_section(&mut out, b"OPS\0", &encode(&ops));
        let assets: Vec<(&AssetRef, &Asset)> = self.assets.iter().collect();
        write_section(&mut out, b"ASST", &encode(&assets));
        let meta = Meta {
            title: self.title.clone(),
            authors: self.authors.clone(),
            branches: self.branches.clone(),
            versions: self.versions.clone(),
            current_branch: self.current.clone(),
            created_ms: self.created_ms,
            modified_ms: self.modified_ms,
        };
        write_section(&mut out, b"META", &encode(&meta));
        if options.snapshot {
            let snap = Snapshot {
                heads: self.current_branch_ref().heads.clone(),
                state: self.state.clone(),
            };
            write_section(&mut out, b"SNAP", &encode(&snap));
        }
        out
    }

    pub fn from_bytes(
        bytes: &[u8],
        actor: ActorId,
        author: AuthorId,
        clock: Box<dyn Clock>,
    ) -> Result<EditLog, EditError> {
        let header = bytes.get(..9).ok_or_else(|| corrupt("header"))?;
        if &header[..4] != MAGIC {
            return Err(corrupt("header"));
        }
        let format = u16::from_le_bytes([header[4], header[5]]);
        if format == 0 {
            return Err(corrupt("header"));
        }
        if format > FORMAT_VERSION {
            return Err(EditError::UnsupportedFormat { found: format });
        }
        let engine_len = header[8] as usize;
        let mut pos = 9 + engine_len;
        if bytes.len() < pos {
            return Err(corrupt("header"));
        }

        let ops: Vec<Op> = decode(read_section(bytes, &mut pos, b"OPS\0", "OPS")?, "OPS")?;
        let assets: Vec<(AssetRef, Asset)> =
            decode(read_section(bytes, &mut pos, b"ASST", "ASST")?, "ASST")?;
        let meta: Meta = decode(read_section(bytes, &mut pos, b"META", "META")?, "META")?;
        let snapshot: Option<Snapshot> = if pos < bytes.len() && bytes[pos..].starts_with(b"SNAP") {
            Some(decode(
                read_section(bytes, &mut pos, b"SNAP", "SNAP")?,
                "SNAP",
            )?)
        } else {
            None
        };
        if pos != bytes.len() {
            return Err(corrupt("trailer"));
        }

        let mut log = EditLog::new(actor, author, clock);
        let mut graph: BTreeMap<OpId, Op> = BTreeMap::new();
        for op in ops {
            check_op_shape(&op).map_err(|_| corrupt("OPS"))?;
            let id = op.id;
            if graph.insert(id, op).is_some() {
                return Err(EditError::DuplicateOp(id));
            }
        }
        for op in graph.values() {
            for p in &op.parents {
                if !graph.contains_key(p) {
                    return Err(EditError::UnknownParent(*p));
                }
                if p.lamport >= op.id.lamport {
                    return Err(corrupt("OPS"));
                }
            }
        }
        for (r, asset) in &assets {
            if r.0 != *blake3::hash(&asset.bytes).as_bytes() {
                return Err(corrupt("ASST"));
            }
        }
        let known = |heads: &BTreeSet<OpId>| heads.iter().all(|h| graph.contains_key(h));
        if !meta.branches.contains_key(&meta.current_branch)
            || !meta
                .branches
                .values()
                .all(|b| known(&b.heads) && known(&b.base))
            || !meta.versions.values().all(|v| known(&v.heads))
        {
            return Err(corrupt("META"));
        }
        for name in meta.branches.keys() {
            check_name(name).map_err(|_| corrupt("META"))?;
        }
        check_name(&meta.current_branch).map_err(|_| corrupt("META"))?;
        for v in meta.versions.values() {
            check_name(&v.name).map_err(|_| corrupt("META"))?;
        }

        let mut next_version_seq = 0u32;
        for v in meta.versions.keys().filter(|v| v.actor == actor) {
            let candidate = v.seq.checked_add(1).ok_or_else(|| corrupt("META"))?;
            next_version_seq = next_version_seq.max(candidate);
        }

        log.max_lamport = graph.keys().map(|id| id.lamport).max().unwrap_or(0);
        log.ops = graph;
        log.assets = assets.into_iter().collect();
        log.next_version_seq = next_version_seq;
        log.title = meta.title;
        log.authors = meta.authors;
        log.branches = meta.branches;
        log.versions = meta.versions;
        log.current = meta.current_branch;
        log.created_ms = meta.created_ms;
        log.modified_ms = meta.modified_ms;

        let heads = log.current_branch_ref().heads.clone();
        log.state = match snapshot {
            Some(s) if s.heads == heads => {
                if cfg!(debug_assertions) {
                    let fresh = materialize(&log.ops, &heads);
                    // A mismatch here means the snapshot was stale or tampered with in a
                    // way that still passes its own checksum; per spec §5.2 we recover
                    // silently from the op graph rather than refusing to load the file.
                    if fresh == s.state { s.state } else { fresh }
                } else {
                    s.state
                }
            }
            _ => materialize(&log.ops, &heads),
        };
        Ok(log)
    }
}
