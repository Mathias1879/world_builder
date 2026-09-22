use crate::layer::LodPolicy;
use crate::tile::{CellValue, Tile};
use crate::world::LayerStore;
use core::fmt;
use wb_grid::{CellId, TILE_SIZE, TileId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LodError {
    MissingTile(TileId),
    NoParentLevel,
    NoChildLevel,
    PolicyNotSupported,
}

impl fmt::Display for LodError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LodError::MissingTile(t) => write!(f, "missing tile {t:?}"),
            LodError::NoParentLevel => f.write_str("level 0 has no parent level"),
            LodError::NoChildLevel => f.write_str("MAX_LEVEL has no child level"),
            LodError::PolicyNotSupported => {
                f.write_str("LOD policy not supported for this cell type")
            }
        }
    }
}

impl std::error::Error for LodError {}

/// Cell types that can be aggregated from 4 children to 1 parent.
pub trait Aggregate: CellValue {
    /// Area-weighted mean, or `None` if the type is categorical.
    fn area_mean(v: [Self; 4], w: [f64; 4]) -> Option<Self>;
    /// Total-order key used by Mode and Max tie-breaks.
    fn order_key(self) -> u64;
}

impl Aggregate for f32 {
    fn area_mean(v: [f32; 4], w: [f64; 4]) -> Option<f32> {
        let wsum: f64 = w.iter().sum();
        let s: f64 = v.iter().zip(w.iter()).map(|(x, wk)| *x as f64 * wk).sum();
        Some((s / wsum) as f32)
    }
    fn order_key(self) -> u64 {
        let b = self.to_bits();
        (if b >> 31 == 1 { !b } else { b | 0x8000_0000 }) as u64
    }
}

impl Aggregate for u8 {
    fn area_mean(_: [u8; 4], _: [f64; 4]) -> Option<u8> {
        None
    }
    fn order_key(self) -> u64 {
        self as u64
    }
}

impl Aggregate for u16 {
    fn area_mean(_: [u16; 4], _: [f64; 4]) -> Option<u16> {
        None
    }
    fn order_key(self) -> u64 {
        self as u64
    }
}

impl Aggregate for i16 {
    fn area_mean(_: [i16; 4], _: [f64; 4]) -> Option<i16> {
        None
    }
    fn order_key(self) -> u64 {
        (self as i32 + 32_768) as u64
    }
}

fn pick_mode<T: Aggregate>(v: [T; 4], w: [f64; 4]) -> T {
    let stats = |key: u64| -> (u32, f64) {
        v.iter()
            .zip(w.iter())
            .filter(|(x, _)| x.order_key() == key)
            .fold((0, 0.0), |(c, a), (_, wk)| (c + 1, a + wk))
    };
    let mut best = v[0];
    let mut best_stats = stats(best.order_key());
    for &cand in &v[1..] {
        let s = stats(cand.order_key());
        let better = s.0 > best_stats.0
            || (s.0 == best_stats.0
                && (s.1 > best_stats.1
                    || (s.1 == best_stats.1 && cand.order_key() < best.order_key())));
        if better {
            best = cand;
            best_stats = s;
        }
    }
    best
}

fn pick_max<T: Aggregate>(v: [T; 4]) -> T {
    v.into_iter().fold(
        v[0],
        |m, x| if x.order_key() > m.order_key() { x } else { m },
    )
}

/// Parent tile from its 4 child tiles.
pub fn downsample<T: Aggregate>(
    store: &LayerStore<T>,
    parent: TileId,
    policy: LodPolicy,
) -> Result<Tile<T>, LodError> {
    let kid_ids = parent.children().ok_or(LodError::NoChildLevel)?;
    let kids: Vec<&Tile<T>> = kid_ids
        .iter()
        .map(|k| store.get(*k).ok_or(LodError::MissingTile(*k)))
        .collect::<Result<_, _>>()?;
    let mut out = Tile::new(parent);
    for j in 0..TILE_SIZE {
        for i in 0..TILE_SIZE {
            let cells = parent.cell(i, j).children().ok_or(LodError::NoChildLevel)?;
            let mut v = [T::default(); 4];
            let mut w = [0.0; 4];
            for (k, c) in cells.iter().enumerate() {
                let (t, ci, cj) = c.tile();
                let tile = kids
                    .iter()
                    .find(|x| x.id() == t)
                    .expect("child cell lies in a child tile");
                v[k] = tile.get(ci, cj);
                w[k] = c.area_unit();
            }
            let value = match policy {
                LodPolicy::AreaMean => T::area_mean(v, w).ok_or(LodError::PolicyNotSupported)?,
                LodPolicy::Mode => pick_mode(v, w),
                LodPolicy::Max => pick_max(v),
            };
            out.set(i, j, value);
        }
    }
    Ok(out)
}

/// Children of one parent: parent + detail − weighted-mean(detail), so the
/// area-weighted mean of the children equals the parent (to f32 rounding).
pub fn refine_cell(parent: f32, detail: [f32; 4], weights: [f64; 4]) -> [f32; 4] {
    let wsum: f64 = weights.iter().sum();
    let dmean: f64 = detail
        .iter()
        .zip(weights.iter())
        .map(|(d, w)| *d as f64 * w)
        .sum::<f64>()
        / wsum;
    detail.map(|d| (parent as f64 + d as f64 - dmean) as f32)
}

/// One child tile from the parent layer plus a detail function.
pub fn refine_tile(
    parent_store: &LayerStore<f32>,
    child: TileId,
    detail: impl Fn(CellId) -> f32,
) -> Result<Tile<f32>, LodError> {
    let parent_id = child.parent().ok_or(LodError::NoParentLevel)?;
    let parent = parent_store
        .get(parent_id)
        .ok_or(LodError::MissingTile(parent_id))?;
    let mut out = Tile::new(child);
    for j in 0..TILE_SIZE {
        for i in 0..TILE_SIZE {
            let c = child.cell(i, j);
            let p = c.parent().ok_or(LodError::NoParentLevel)?;
            let (_, pi, pj) = p.tile();
            let sibs = p
                .children()
                .expect("a parent of an existing cell has children");
            let w = sibs.map(|s| s.area_unit());
            let d = sibs.map(&detail);
            let vals = refine_cell(parent.get(pi, pj), d, w);
            let k = sibs
                .iter()
                .position(|s| *s == c)
                .expect("cell is one of its parent's children");
            out.set(i, j, vals[k]);
        }
    }
    Ok(out)
}
