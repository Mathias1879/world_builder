use core::f64::consts::FRAC_PI_2;
use wb_grid::{CellId, Face, MAX_LEVEL, cells_per_edge};

fn edge_cells(level: u8) -> Vec<CellId> {
    let m = cells_per_edge(level);
    let mut out = Vec::new();
    for face in Face::ALL {
        for k in 0..m {
            for (i, j) in [(k, 0), (k, m - 1), (0, k), (m - 1, k)] {
                out.push(CellId::new(face, level, i, j).unwrap());
            }
        }
    }
    out
}

#[test]
fn neighbours4_are_symmetric_across_every_face_edge() {
    for c in edge_cells(0) {
        for n in c.neighbors4() {
            assert_ne!(n, c);
            assert!(n.neighbors4().contains(&c), "{c:?} -> {n:?} is not mutual");
        }
    }
}

#[test]
fn neighbours4_are_close() {
    let m = cells_per_edge(0) as f64;
    let max_angle = 1.5 * FRAC_PI_2 / m;
    for c in edge_cells(0) {
        for n in c.neighbors4() {
            assert!(
                c.center().dot(n.center()) > libm::cos(max_angle),
                "{c:?} -> {n:?}"
            );
        }
    }
}

#[test]
fn neighbours4_cross_to_the_expected_face() {
    let m = cells_per_edge(0);
    let c = CellId::new(Face::PosX, 0, m - 1, 100).unwrap();
    // +u on PosX points toward +Y.
    assert_eq!(c.offset(1, 0).face, Face::PosY);
}

#[test]
fn neighbours8_counts() {
    let m = cells_per_edge(0);
    for c in edge_cells(0) {
        let n8 = c.neighbors8();
        let corner = (c.i == 0 || c.i == m - 1) && (c.j == 0 || c.j == m - 1);
        assert_eq!(n8.len(), if corner { 7 } else { 8 }, "{c:?}");
        assert!(!n8.contains(&c));
    }
    let interior = CellId::new(Face::NegZ, 0, 10, 10).unwrap();
    assert_eq!(interior.neighbors8().len(), 8);
}

/// Edge cells at `level`: every cell within `near` of a cube corner along each
/// face edge, plus `spread` evenly spaced cells along each face edge.
fn sampled_edge_cells(level: u8, near: u32, spread: u32) -> Vec<CellId> {
    let m = cells_per_edge(level);
    let mut ks: Vec<u32> = (0..near).chain((0..near).map(|d| m - 1 - d)).collect();
    ks.extend((0..spread).map(|n| ((n as u64 * (m as u64 - 1)) / (spread as u64 - 1)) as u32));
    ks.sort_unstable();
    ks.dedup();
    let mut out = Vec::new();
    for face in Face::ALL {
        for &k in &ks {
            for (i, j) in [(k, 0), (k, m - 1), (0, k), (m - 1, k)] {
                out.push(CellId::new(face, level, i, j).unwrap());
            }
        }
    }
    out
}

fn assert_mutual4(cells: &[CellId]) {
    for &c in cells {
        for n in c.neighbors4() {
            assert_ne!(n, c);
            assert!(n.neighbors4().contains(&c), "{c:?} -> {n:?} is not mutual");
        }
    }
}

#[test]
fn neighbours4_are_mutual_on_every_edge_cell_at_level_3() {
    assert_mutual4(&edge_cells(3));
}

#[test]
fn neighbours4_are_mutual_at_max_level() {
    assert_mutual4(&sampled_edge_cells(MAX_LEVEL, 64, 2000));
}

#[test]
fn neighbours4_are_close_at_max_level() {
    let m = cells_per_edge(MAX_LEVEL) as f64;
    let max_angle = 1.5 * FRAC_PI_2 / m;
    for c in sampled_edge_cells(MAX_LEVEL, 64, 2000) {
        for n in c.neighbors4() {
            let angle = libm::acos(c.center().dot(n.center()).min(1.0));
            assert!(angle < max_angle, "{c:?} -> {n:?}: {angle}");
        }
    }
}

#[test]
fn neighbours8_near_corners_at_max_level() {
    let m = cells_per_edge(MAX_LEVEL);
    for c in sampled_edge_cells(MAX_LEVEL, 64, 2) {
        let n8 = c.neighbors8();
        let corner = (c.i == 0 || c.i == m - 1) && (c.j == 0 || c.j == m - 1);
        assert_eq!(n8.len(), if corner { 7 } else { 8 }, "{c:?}");
        assert!(!n8.contains(&c));
        for n in &n8 {
            assert!(n.neighbors8().contains(&c), "{c:?} -> {n:?} is not mutual");
        }
    }
}
