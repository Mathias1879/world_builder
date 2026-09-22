use core::f64::consts::PI;
use wb_grid::{CellId, Face, cells_per_edge};

#[test]
fn level0_cells_tile_the_sphere() {
    let m = cells_per_edge(0);
    let mut total = 0.0;
    for face in Face::ALL {
        for j in 0..m {
            for i in 0..m {
                total += CellId::new(face, 0, i, j).unwrap().area_unit();
            }
        }
    }
    assert!(
        (total - 4.0 * PI).abs() < 1e-9 * 4.0 * PI,
        "total = {total}"
    );
}

#[test]
fn children_sum_to_parent() {
    for (i, j) in [(0, 0), (17, 200), (128, 128), (255, 3)] {
        let p = CellId::new(Face::PosZ, 1, i, j).unwrap();
        let sum: f64 = p.children().unwrap().iter().map(|c| c.area_unit()).sum();
        assert!((sum - p.area_unit()).abs() < 1e-15, "{p:?}");
    }
}

#[test]
fn equi_angular_area_ratio_is_bounded() {
    let m = cells_per_edge(0);
    let corner = CellId::new(Face::PosX, 0, 0, 0).unwrap().area_unit();
    let centre = CellId::new(Face::PosX, 0, m / 2, m / 2)
        .unwrap()
        .area_unit();
    let ratio = corner.max(centre) / corner.min(centre);
    assert!(ratio > 1.0 && ratio < 1.5, "ratio = {ratio}");
}
