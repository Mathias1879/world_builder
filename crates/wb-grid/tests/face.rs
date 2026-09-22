use proptest::prelude::*;
use wb_grid::{Face, face_to_sphere, sphere_to_face};

#[test]
fn face_centres_are_normals() {
    for face in Face::ALL {
        let (n, _, _) = face.basis();
        let p = face_to_sphere(face, 0.0, 0.0);
        assert!((p.dot(n) - 1.0).abs() < 1e-15, "{face:?}");
    }
}

#[test]
fn bases_are_orthonormal() {
    for face in Face::ALL {
        let (n, u, v) = face.basis();
        assert_eq!(n.dot(u), 0.0);
        assert_eq!(n.dot(v), 0.0);
        assert_eq!(u.dot(v), 0.0);
    }
}

#[test]
fn index_roundtrip() {
    for face in Face::ALL {
        assert_eq!(Face::from_index(face.index()), Some(face));
    }
    assert_eq!(Face::from_index(6), None);
}

#[test]
fn stepping_past_an_edge_changes_face() {
    // +u on PosX points toward +Y.
    let (face, _, _) = sphere_to_face(face_to_sphere(Face::PosX, 1.01, 0.0));
    assert_eq!(face, Face::PosY);
}

fn face_strategy() -> impl Strategy<Value = Face> {
    (0u8..6).prop_map(|i| Face::from_index(i).unwrap())
}

fn cfg() -> ProptestConfig {
    ProptestConfig {
        cases: 512,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(cfg())]
    #[test]
    fn projection_roundtrip(face in face_strategy(), s in -0.999f64..0.999, t in -0.999f64..0.999) {
        let p = face_to_sphere(face, s, t);
        prop_assert!((p.length() - 1.0).abs() < 1e-14);
        let (f2, s2, t2) = sphere_to_face(p);
        prop_assert_eq!(f2, face);
        prop_assert!((s - s2).abs() < 1e-12);
        prop_assert!((t - t2).abs() < 1e-12);
    }
}
