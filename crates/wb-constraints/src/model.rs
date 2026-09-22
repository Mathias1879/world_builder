use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use wb_editlog::{EntityId, Value};

/// Dotted issue identifier, e.g. `lore.area_mismatch`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct IssueCode(pub String);

impl IssueCode {
    pub fn new(s: &str) -> Self {
        IssueCode(s.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for IssueCode {
    fn from(s: &str) -> Self {
        IssueCode::new(s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FindingKind {
    Support,
    Issue,
    Consequence,
}

/// Explanation parameter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Param {
    Int(i64),
    Float(f64),
    Text(String),
    Entity(EntityId),
}

/// A one-click fix: field writes committed as one Edit Log transaction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Suggestion {
    pub code: IssueCode,
    pub params: BTreeMap<String, Param>,
    pub writes: Vec<(EntityId, String, Value)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub checker: String,
    pub kind: FindingKind,
    pub code: IssueCode,
    /// Issue badness in [0, 1]; 0 for supports and consequences.
    pub badness: f64,
    pub params: BTreeMap<String, Param>,
    pub related: Vec<EntityId>,
    pub suggestions: Vec<Suggestion>,
}

impl Finding {
    fn new(checker: &str, kind: FindingKind, code: &str, badness: f64) -> Self {
        Finding {
            checker: checker.to_string(),
            kind,
            code: IssueCode::new(code),
            badness,
            params: BTreeMap::new(),
            related: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn issue(checker: &str, code: &str, badness: f64) -> Self {
        Finding::new(checker, FindingKind::Issue, code, badness)
    }

    pub fn support(checker: &str, code: &str) -> Self {
        Finding::new(checker, FindingKind::Support, code, 0.0)
    }

    pub fn consequence(checker: &str, code: &str) -> Self {
        Finding::new(checker, FindingKind::Consequence, code, 0.0)
    }

    pub fn with_param(mut self, key: &str, value: Param) -> Self {
        self.params.insert(key.to_string(), value);
        self
    }

    pub fn with_related(mut self, e: EntityId) -> Self {
        self.related.push(e);
        self
    }

    pub fn with_suggestion(mut self, s: Suggestion) -> Self {
        self.suggestions.push(s);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Grade {
    Plausible,
    Stretch,
    Implausible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Open,
    Intentional,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    EarthStrict,
    PlausibleFantasy,
    HighFantasy,
}

impl Preset {
    pub fn value(self) -> f64 {
        match self {
            Preset::EarthStrict => 0.0,
            Preset::PlausibleFantasy => 0.5,
            Preset::HighFantasy => 0.85,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    pub constraint: EntityId,
    pub kind: String,
    pub grade: Grade,
    pub score: f64,
    pub status: Status,
    pub issues: Vec<Finding>,
    pub intentional: Vec<Finding>,
    pub supports: Vec<Finding>,
    pub consequences: Vec<Finding>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    pub plausible: u32,
    pub stretch: u32,
    pub implausible: u32,
    pub intentional: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub verdicts: Vec<Verdict>,
    pub counts: Counts,
    pub hash: [u8; 32],
}

impl Report {
    pub fn new(verdicts: Vec<Verdict>) -> Report {
        let mut counts = Counts::default();
        for v in &verdicts {
            match (v.status, v.grade) {
                (Status::Intentional, _) => counts.intentional += 1,
                (Status::Open, Grade::Plausible) => counts.plausible += 1,
                (Status::Open, Grade::Stretch) => counts.stretch += 1,
                (Status::Open, Grade::Implausible) => counts.implausible += 1,
            }
        }
        let mut h = blake3::Hasher::new();
        h.update(b"wb-report-v1");
        h.update(&postcard::to_allocvec(&verdicts).expect("verdicts always serialize"));
        Report {
            verdicts,
            counts,
            hash: h.finalize().into(),
        }
    }
}
