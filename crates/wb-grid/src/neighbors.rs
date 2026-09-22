use crate::cell::{CellId, cells_per_edge};
use crate::face::Face;

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

/// A face edge: which face coordinate is pinned, and to which end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edge {
    /// `s = +1` (`i = m − 1`).
    PosU,
    /// `s = −1` (`i = 0`).
    NegU,
    /// `t = +1` (`j = m − 1`).
    PosV,
    /// `t = −1` (`j = 0`).
    NegV,
}

#[cfg(test)]
impl Edge {
    const ALL: [Edge; 4] = [Edge::PosU, Edge::NegU, Edge::PosV, Edge::NegV];
}

/// Across `(face, edge)`: the neighbouring face, the edge of it that is shared,
/// and whether the along-edge index runs in the opposite direction there.
/// Indexed `[face.index()][edge as usize]`. Derived from `Face::basis` (the
/// face across the `±u`/`±v` edge is the one whose normal is `±u`/`±v`); the
/// unit test `adjacency_matches_face_bases` re-derives it.
const ADJACENCY: [[(Face, Edge, bool); 4]; 6] = {
    use Edge::*;
    use Face::*;
    [
        // PosX
        [
            (PosY, NegU, false),
            (NegY, PosU, false),
            (PosZ, NegV, false),
            (NegZ, PosV, false),
        ],
        // NegX
        [
            (NegY, NegU, false),
            (PosY, PosU, false),
            (PosZ, PosV, true),
            (NegZ, NegV, true),
        ],
        // PosY
        [
            (NegX, NegU, false),
            (PosX, PosU, false),
            (PosZ, PosU, false),
            (NegZ, PosU, true),
        ],
        // NegY
        [
            (PosX, NegU, false),
            (NegX, PosU, false),
            (PosZ, NegU, true),
            (NegZ, NegU, false),
        ],
        // PosZ
        [
            (PosY, PosV, false),
            (NegY, PosV, true),
            (NegX, PosV, true),
            (PosX, PosV, false),
        ],
        // NegZ
        [
            (PosY, NegV, true),
            (NegY, NegV, false),
            (PosX, NegV, false),
            (NegX, NegV, true),
        ],
    ]
};

/// The cell just across `edge` of `face`, where `k` is the along-edge index on
/// `face` (`j` for a u edge, `i` for a v edge). Exact integer mapping.
fn cross(face: Face, level: u8, edge: Edge, k: u32) -> CellId {
    let m = cells_per_edge(level);
    let (g, g_edge, reversed) = ADJACENCY[face.index() as usize][edge as usize];
    let a = if reversed { m - 1 - k } else { k };
    let (i, j) = match g_edge {
        Edge::PosU => (m - 1, a),
        Edge::NegU => (0, a),
        Edge::PosV => (a, m - 1),
        Edge::NegV => (a, 0),
    };
    CellId {
        face: g,
        level,
        i,
        j,
    }
}

impl CellId {
    /// Neighbouring cell one step away. Off-face steps use an exact integer
    /// edge-adjacency table. A diagonal step is one on-face step followed by one
    /// edge crossing; at a cube corner, the diagonal that would enter the
    /// missing fourth cell resolves to the 4-neighbour across the `i` edge.
    pub fn offset(self, di: i32, dj: i32) -> CellId {
        debug_assert!((-1..=1).contains(&di) && (-1..=1).contains(&dj));
        let m = cells_per_edge(self.level) as i64;
        let ni = self.i as i64 + di as i64;
        let nj = self.j as i64 + dj as i64;
        let i_in = (0..m).contains(&ni);
        let j_in = (0..m).contains(&nj);
        match (i_in, j_in) {
            (true, true) => CellId {
                i: ni as u32,
                j: nj as u32,
                ..self
            },
            (false, true) => {
                let edge = if ni < 0 { Edge::NegU } else { Edge::PosU };
                cross(self.face, self.level, edge, nj as u32)
            }
            (true, false) => {
                let edge = if nj < 0 { Edge::NegV } else { Edge::PosV };
                cross(self.face, self.level, edge, ni as u32)
            }
            (false, false) => {
                let edge = if ni < 0 { Edge::NegU } else { Edge::PosU };
                cross(self.face, self.level, edge, self.j)
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec3::Vec3;

    fn neg(v: Vec3) -> Vec3 {
        Vec3::new(-v.x, -v.y, -v.z)
    }

    /// `(outward direction, along-edge direction)` of an edge in face space.
    fn edge_dirs(face: Face, edge: Edge) -> (Vec3, Vec3) {
        let (_, u, v) = face.basis();
        match edge {
            Edge::PosU => (u, v),
            Edge::NegU => (neg(u), v),
            Edge::PosV => (v, u),
            Edge::NegV => (neg(v), u),
        }
    }

    #[test]
    fn adjacency_matches_face_bases() {
        for f in Face::ALL {
            let (n, _, _) = f.basis();
            for e in Edge::ALL {
                let (out, along) = edge_dirs(f, e);
                let (g, ge, rev) = ADJACENCY[f.index() as usize][e as usize];
                // The face across the edge has the outward direction as its normal.
                assert_eq!(g.basis().0, out, "{f:?} {e:?}");
                // Seen from g, f lies outward across the shared edge.
                let (g_out, g_along) = edge_dirs(g, ge);
                assert_eq!(g_out, n, "{f:?} {e:?}");
                // Along-edge directions agree or are reversed.
                let expected = if rev { neg(g_along) } else { g_along };
                assert_eq!(along, expected, "{f:?} {e:?}");
                // The table is its own inverse.
                let back = ADJACENCY[g.index() as usize][ge as usize];
                assert_eq!(back, (f, e, rev), "{f:?} {e:?}");
            }
        }
    }
}
