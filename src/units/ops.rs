use std::ops::{Add, BitXor, Div, DivAssign, Mul, MulAssign, Sub};

use num::{
    complex::{Complex32, Complex64},
    pow::Pow,
};

use crate::{
    core::{
        util::impl_op_permutations,
        value::{Scalar, Value},
    },
    units::{COMPOSITIONS, Dimension, Quantity, Unit},
};

impl Dimension {
    pub const fn pow(self, exponent: i8) -> Self {
        Self {
            T: self.T * exponent,
            L: self.L * exponent,
            M: self.M * exponent,
            I: self.I * exponent,
            Θ: self.Θ * exponent,
            J: self.J * exponent,
            N: self.N * exponent,
        }
    }
}

impl Mul for Dimension {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            T: self.T + rhs.T,
            L: self.L + rhs.L,
            M: self.M + rhs.M,
            I: self.I + rhs.I,
            N: self.N + rhs.N,
            Θ: self.Θ + rhs.Θ,
            J: self.J + rhs.J,
        }
    }
}

impl MulAssign for Dimension {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl DivAssign for Dimension {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}

impl Pow<i32> for Dimension {
    type Output = Dimension;

    fn pow(self, rhs: i32) -> Self::Output {
        Self {
            T: self.T * rhs as i8,
            L: self.L * rhs as i8,
            M: self.M * rhs as i8,
            I: self.I * rhs as i8,
            N: self.N * rhs as i8,
            Θ: self.Θ * rhs as i8,
            J: self.J * rhs as i8,
        }
    }
}

impl Div for Dimension {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self {
            T: self.T - rhs.T,
            L: self.L - rhs.L,
            M: self.M - rhs.M,
            I: self.I - rhs.I,
            N: self.N - rhs.N,
            Θ: self.Θ - rhs.Θ,
            J: self.J - rhs.J,
        }
    }
}

impl Mul for Unit {
    type Output = Unit;

    fn mul(self, rhs: Self) -> Self::Output {
        let result = {
            match (self, rhs) {
                (Unit::Composed(id), rhs) if rhs.is_atomic() => {
                    let mut lhs_comp = COMPOSITIONS.get_cloned(id).unwrap();

                    lhs_comp.push((rhs, 1));
                    lhs_comp
                }

                (_, Unit::Unitless) => return self,

                (lhs, Unit::Composed(id)) if lhs.is_atomic() => {
                    let mut rhs_comp = COMPOSITIONS.get_cloned(id).unwrap();
                    let mut new_comp = vec![(self, 1)];

                    new_comp.append(&mut rhs_comp);
                    new_comp
                }
                (Unit::Unitless, _) => return rhs,

                (Unit::Composed(lhs_id), Unit::Composed(rhs_id)) => {
                    let mut lhs_comp = COMPOSITIONS.get_cloned(lhs_id).unwrap();
                    let mut rhs_comp = COMPOSITIONS.get_cloned(rhs_id).unwrap();

                    lhs_comp.append(&mut rhs_comp);
                    lhs_comp
                }

                (lhs, rhs) if lhs.is_atomic() && rhs.is_atomic() => {
                    vec![(lhs, 1), (rhs, 1)]
                }
                _ => unreachable!(),
            }
        };

        Unit::new_composition(result)
    }
}

impl Div for Unit {
    type Output = Unit;

    fn div(self, rhs: Self) -> Self::Output {
        self * (rhs.pow(-1))
    }
}

impl Pow<i32> for Unit {
    type Output = Unit;

    fn pow(self, exp: i32) -> Self::Output {
        match self {
            Self::Unitless => self,
            _ if self.is_atomic() => {
                Unit::new_composition(vec![(self, exp as i8)])
            }
            Self::Composed(id) => {
                let comp = COMPOSITIONS
                    .get_cloned(id)
                    .unwrap()
                    .iter()
                    .map(|(unit, e)| (*unit, e * exp as i8))
                    .collect();

                Unit::new_composition(comp)
            }
            _ => unreachable!(),
        }
    }
}

macro_rules! impl_qty_from_scalar {
    ($($t:ty),*) => {
        $(
            impl From<$t> for Quantity {
                fn from(value: $t) -> Self {
                    Quantity(value.into(), Unit::Unitless)
                }
            }
        )*
    };
}

impl_qty_from_scalar!(
    u8, i8, u16, i16, u32, i32, u64, i64, f32, f64, Complex32, Complex64,
    Scalar, Value
);

impl From<Unit> for Quantity {
    fn from(unit: Unit) -> Self {
        Quantity(1.0.into(), unit)
    }
}

impl_op_permutations! {
    types = [i64, f64, Scalar, Value, Quantity, Unit],
    exclude_permutations = [i64, f64, Scalar, Value],
    exclude_specific = [(Unit, Unit)],
    out = Quantity,

    add = {
        assert!(lhs.unit().repr_eq(rhs.unit()), "cannot add two quantities with different units");
        Quantity(lhs.value().clone() + rhs.value().clone(), lhs.unit())
    },

    sub = {
        assert!(lhs.unit().repr_eq(rhs.unit()), "cannot subtract two quantities with different units");
        Quantity(lhs.value().clone() - rhs.value().clone(), lhs.unit())
    },

    mul = {
        Quantity(lhs.value().clone() * rhs.value().clone(), lhs.unit() * rhs.unit())
    },

    div = {
        Quantity(lhs.value().clone() / rhs.value().clone(), lhs.unit() / rhs.unit())
    },

    pow = {
        todo!()
    },

    partial_eq = {
        lhs == rhs
    }
}
