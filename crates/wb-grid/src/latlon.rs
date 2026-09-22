use crate::vec3::Vec3;

/// Geographic coordinate in radians. `lat` in [-π/2, π/2], `lon` in (-π, π].
/// Convention: z = north pole, x = (0°, 0°), y = (0°, 90°E).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

impl LatLon {
    pub fn from_degrees(lat_deg: f64, lon_deg: f64) -> Self {
        Self {
            lat: lat_deg.to_radians(),
            lon: lon_deg.to_radians(),
        }
    }

    pub fn to_vec3(self) -> Vec3 {
        let cl = libm::cos(self.lat);
        Vec3::new(
            cl * libm::cos(self.lon),
            cl * libm::sin(self.lon),
            libm::sin(self.lat),
        )
    }

    pub fn from_vec3(v: Vec3) -> Self {
        let horizontal = (v.x * v.x + v.y * v.y).sqrt();
        Self {
            lat: libm::atan2(v.z, horizontal),
            lon: libm::atan2(v.y, v.x),
        }
    }
}
