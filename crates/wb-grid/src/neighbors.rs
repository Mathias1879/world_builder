use crate::cell::{CellId, cells_per_edge};
use crate::face::face_to_sphere;

pub const DIRS4: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
pub const DIRS8: [(i32, i32); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];

impl CellId {
    /// Neighbouring cell one step away. Off-face steps are resolved by projecting the
    /// would-be cell centre (just past the edge) onto the sphere and locating it.
    pub fn offset(self, di: i32, dj: i32) -> CellId {
        debug_assert!((-1..=1).contains(&di) && (-1..=1).contains(&dj));
        let m = cells_per_edge(self.level) as i64;
        let ni = self.i as i64 + di as i64;
        let nj = self.j as i64 + dj as i64;
        if (0..m).contains(&ni) && (0..m).contains(&nj) {
            return CellId {
                i: ni as u32,
                j: nj as u32,
                ..self
            };
        }
        let s = -1.0 + (ni as f64 + 0.5) * 2.0 / m as f64;
        let t = -1.0 + (nj as f64 + 0.5) * 2.0 / m as f64;
        CellId::locate(face_to_sphere(self.face, s, t), self.level)
    }

    pub fn neighbors4(self) -> [CellId; 4] {
        DIRS4.map(|(di, dj)| self.offset(di, dj))
    }

    /// Distinct 8-neighbourhood; at cube corners only 7 cells exist.
    pub fn neighbors8(self) -> Vec<CellId> {
        let mut out = Vec::with_capacity(8);
        for (di, dj) in DIRS8 {
            let n = self.offset(di, dj);
            if n != self && !out.contains(&n) {
                out.push(n);
            }
        }
        out
    }
}
