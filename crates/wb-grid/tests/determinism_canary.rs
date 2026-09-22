//! If these fail on any target, libm is not bit-exact there and the
//! determinism invariant is broken.

#[test]
fn libm_sin_is_correctly_rounded() {
    assert_eq!(
        libm::sin(1.0).to_bits(),
        0.841_470_984_807_896_5_f64.to_bits()
    );
}

#[test]
fn libm_atan_of_one_is_quarter_pi() {
    assert_eq!(
        libm::atan(1.0).to_bits(),
        core::f64::consts::FRAC_PI_4.to_bits()
    );
}
