use crate::cell::{CellId, cells_per_edge};
use core::f64::consts::FRAC_PI_4;

/// Solid angle of the gnomonic rectangle [0, a] × [0, b] on a unit-distance plane.
fn rect_solid_angle(a: f64, b: f64) -> f64 {
    libm::atan(a * b / (1.0 + a * a + b * b).sqrt())
}

impl CellId {
    /// Exact area of this cell on the unit sphere, in steradians.
    pub fn area_unit(self) -> f64 {
        let m = cells_per_edge(self.level) as f64;
        let edge = |k: u32| libm::tan(FRAC_PI_4 * (-1.0 + k as f64 * 2.0 / m));
        let (a0, a1) = (edge(self.i), edge(self.i + 1));
        let (b0, b1) = (edge(self.j), edge(self.j + 1));
        rect_solid_angle(a1, b1) - rect_solid_angle(a0, b1) - rect_solid_angle(a1, b0)
            + rect_solid_angle(a0, b0)
    }
}
