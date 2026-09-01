use super::*;
use crate::units::{
    non_si::eV,
    si::{Hz, J, m, s},
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
    assert_eq!((10.0 * eV / J).normalize(), 1.602176634e-18)
}

#[test]
pub fn analysis() {
    assert_eq!((eV / J).analyze().unwrap(), DIMENSIONLESS);
}
