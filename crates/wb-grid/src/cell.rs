use crate::face::{Face, face_to_sphere, sphere_to_face};
use crate::vec3::Vec3;

pub const TILE_SIZE: u32 = 256;
pub const MAX_LEVEL: u8 = 12;

pub const fn tiles_per_edge(level: u8) -> u32 {
    1u32 << level
}

pub const fn cells_per_edge(level: u8) -> u32 {
    TILE_SIZE << level
}

/// A 256×256 tile. `x` runs along the face's u axis, `y` along v.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileId {
    pub face: Face,
    pub level: u8,
    pub x: u32,
    pub y: u32,
}

/// A single cell, addressed by global per-face indices at a tile level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellId {
    pub face: Face,
    pub level: u8,
    pub i: u32,
    pub j: u32,
}

impl TileId {
    pub fn new(face: Face, level: u8, x: u32, y: u32) -> Option<TileId> {
        let ok = level <= MAX_LEVEL && x < tiles_per_edge(level) && y < tiles_per_edge(level);
        ok.then_some(TileId { face, level, x, y })
    }

    /// Cell at local coordinates `(i, j)`, each in `0..TILE_SIZE`.
    pub fn cell(self, i: u32, j: u32) -> CellId {
        assert!(
            i < TILE_SIZE && j < TILE_SIZE,
            "local cell index out of range: ({}, {})",
            i,
            j
        );
        CellId {
            face: self.face,
            level: self.level,
            i: self.x * TILE_SIZE + i,
            j: self.y * TILE_SIZE + j,
        }
    }

    pub fn parent(self) -> Option<TileId> {
        (self.level > 0).then(|| TileId {
            face: self.face,
            level: self.level - 1,
            x: self.x / 2,
            y: self.y / 2,
        })
    }

    pub fn children(self) -> Option<[TileId; 4]> {
        (self.level < MAX_LEVEL).then(|| {
            let (face, level, x, y) = (self.face, self.level + 1, self.x * 2, self.y * 2);
            [
                TileId { face, level, x, y },
                TileId {
                    face,
                    level,
                    x: x + 1,
                    y,
                },
                TileId {
                    face,
                    level,
                    x,
                    y: y + 1,
                },
                TileId {
                    face,
                    level,
                    x: x + 1,
                    y: y + 1,
                },
            ]
        })
    }

    /// Every tile at `level`, in `Ord` order (face, level, x, y).
    pub fn all(level: u8) -> Vec<TileId> {
        let n = tiles_per_edge(level);
        let mut out = Vec::with_capacity(6 * (n * n) as usize);
        for face in Face::ALL {
            for x in 0..n {
                for y in 0..n {
                    out.push(TileId { face, level, x, y });
                }
            }
        }
        out
    }
}

impl CellId {
    pub fn new(face: Face, level: u8, i: u32, j: u32) -> Option<CellId> {
        let ok = level <= MAX_LEVEL && i < cells_per_edge(level) && j < cells_per_edge(level);
        ok.then_some(CellId { face, level, i, j })
    }

    /// Equi-angular coordinates of the cell centre.
    pub fn center_st(self) -> (f64, f64) {
        let m = cells_per_edge(self.level) as f64;
        (
            -1.0 + (self.i as f64 + 0.5) * 2.0 / m,
            -1.0 + (self.j as f64 + 0.5) * 2.0 / m,
        )
    }

    pub fn center(self) -> Vec3 {
        let (s, t) = self.center_st();
        face_to_sphere(self.face, s, t)
    }

    /// The cell containing direction `p` at `level`.
    pub fn locate(p: Vec3, level: u8) -> CellId {
        let (face, s, t) = sphere_to_face(p);
        let m = cells_per_edge(level);
        let index = |c: f64| -> u32 {
            let k = ((c + 1.0) * 0.5 * m as f64) as i64;
            k.clamp(0, m as i64 - 1) as u32
        };
        CellId {
            face,
            level,
            i: index(s),
            j: index(t),
        }
    }

    /// `(tile, local_i, local_j)`.
    pub fn tile(self) -> (TileId, u32, u32) {
        let tile = TileId {
            face: self.face,
            level: self.level,
            x: self.i / TILE_SIZE,
            y: self.j / TILE_SIZE,
        };
        (tile, self.i % TILE_SIZE, self.j % TILE_SIZE)
    }

    pub fn parent(self) -> Option<CellId> {
        (self.level > 0).then(|| CellId {
            face: self.face,
            level: self.level - 1,
            i: self.i / 2,
            j: self.j / 2,
        })
    }

    pub fn children(self) -> Option<[CellId; 4]> {
        (self.level < MAX_LEVEL).then(|| {
            let (face, level, i, j) = (self.face, self.level + 1, self.i * 2, self.j * 2);
            [
                CellId { face, level, i, j },
                CellId {
                    face,
                    level,
                    i: i + 1,
                    j,
                },
                CellId {
                    face,
                    level,
                    i,
                    j: j + 1,
                },
                CellId {
                    face,
                    level,
                    i: i + 1,
                    j: j + 1,
                },
            ]
        })
    }
}
