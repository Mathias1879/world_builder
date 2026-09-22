use crate::vec3::Vec3;
use core::f64::consts::FRAC_PI_4;

const X: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const Y: Vec3 = Vec3::new(0.0, 1.0, 0.0);
const Z: Vec3 = Vec3::new(0.0, 0.0, 1.0);
const NX: Vec3 = Vec3::new(-1.0, 0.0, 0.0);
const NY: Vec3 = Vec3::new(0.0, -1.0, 0.0);
const NZ: Vec3 = Vec3::new(0.0, 0.0, -1.0);

/// One of the six cube faces. Declaration order defines `Ord` and must not change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Face {
    PosX = 0,
    NegX = 1,
    PosY = 2,
    NegY = 3,
    PosZ = 4,
    NegZ = 5,
}

impl Face {
    pub const ALL: [Face; 6] = [
        Face::PosX,
        Face::NegX,
        Face::PosY,
        Face::NegY,
        Face::PosZ,
        Face::NegZ,
    ];

    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn from_index(i: u8) -> Option<Face> {
        Face::ALL.get(i as usize).copied()
    }

    /// `(normal, u_axis, v_axis)`; cube point = normal + a·u + b·v.
    pub fn basis(self) -> (Vec3, Vec3, Vec3) {
        match self {
            Face::PosX => (X, Y, Z),
            Face::NegX => (NX, NY, Z),
            Face::PosY => (Y, NX, Z),
            Face::NegY => (NY, X, Z),
            Face::PosZ => (Z, Y, NX),
            Face::NegZ => (NZ, Y, X),
        }
    }
}

/// Equi-angular face coordinates → unit vector. `s`/`t` beyond ±1 (up to < 2)
/// land on the neighbouring face, which neighbour lookup relies on.
pub fn face_to_sphere(face: Face, s: f64, t: f64) -> Vec3 {
    let (n, u, v) = face.basis();
    let a = libm::tan(FRAC_PI_4 * s);
    let b = libm::tan(FRAC_PI_4 * t);
    n.plus(u.scaled(a)).plus(v.scaled(b)).normalize()
}

/// Any non-zero vector → (face, s, t) with s, t clamped to [-1, 1].
pub fn sphere_to_face(p: Vec3) -> (Face, f64, f64) {
    let (ax, ay, az) = (p.x.abs(), p.y.abs(), p.z.abs());
    let face = if ax >= ay && ax >= az {
        if p.x >= 0.0 { Face::PosX } else { Face::NegX }
    } else if ay >= az {
        if p.y >= 0.0 { Face::PosY } else { Face::NegY }
    } else if p.z >= 0.0 {
        Face::PosZ
    } else {
        Face::NegZ
    };
    let (n, u, v) = face.basis();
    let d = p.dot(n);
    let s = libm::atan(p.dot(u) / d) / FRAC_PI_4;
    let t = libm::atan(p.dot(v) / d) / FRAC_PI_4;
    (face, s.clamp(-1.0, 1.0), t.clamp(-1.0, 1.0))
}
