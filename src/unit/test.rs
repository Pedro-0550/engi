use super::*;
use crate::unit::defs::{
    B, Hz, J, K, Np, V, W, dB, eV, m, mNp, mV, mW, oC, s, µV,
};

/* -------------------------------- FUNCTIONS ------------------------------- */

#[test]
pub fn equivalence() {
    assert_ne!(J * m, m * J);
    assert_eq!(J * m, J * m);

    assert!(!(J * Hz).repr_eq(J / s));
    assert!(!(eV / s).repr_eq(J / s));
    assert!((J * s).repr_eq(s * J));

    assert!((J * Hz).dimensional_eq(J / s));
    assert!(!(eV * s).dimensional_eq(J / s));
}

#[test]
pub fn normalization() {
    assert_eq!((10.0 * eV / J).normalize(), 1.602176634e-18);
    assert_eq!((100.0 * oC).normalize(), 373.15 * K);
    assert_eq!((3 * B).normalize(), 1000);

    assert_eq!((-20 * dB(J)).normalize(), 0.01 * J);
    assert_eq!((30 * dB(mW)).normalize(), 1 * W);
    assert_eq!((-20 * dB).normalize(), 0.01);
    assert_eq!((5 * Np).normalize(), 148.4131591025766);
}

#[test]
pub fn prefixalization() {
    assert_eq!((5e-3 * V).prefixalize(), 5 * mV);
    assert_eq!((5e-6 * V / s).prefixalize(), 5 * µV / s);
}

#[test]
pub fn analysis() {
    assert_eq!((eV / J).dimension().unwrap(), Dimension::DIMENSIONLESS);
}
