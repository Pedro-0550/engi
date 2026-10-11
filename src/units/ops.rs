use std::ops::{Add, BitXor, Div, DivAssign, Mul, MulAssign, Sub};

use num::{
    complex::{Complex32, Complex64},
    pow::Pow,
};

use crate::{
    core::{
        util::impl_op_permutations,
        value::{ComplexExt, Value},
    },
    units::{
        COMPOSITIONS, DIMENSIONLESS, Dimension, Dimensioned, Quantity, Unit,
        si::rad,
    },
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

impl Pow<i64> for Unit {
    type Output = Unit;

    fn pow(self, exp: i64) -> Self::Output {
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

impl_qty_from_scalar!(i64, f64, Complex64, Value);

macro_rules! impl_qty_trig_fn {
    ($ident:ident) => {
        pub fn $ident(self) -> Quantity {
            assert_eq!(self.1.dimension().unwrap(), DIMENSIONLESS, "Trig functions are transcedental, so they can only take dimensionless arguments");

            self.0.sin() * self.1
        }
    };
}

impl Quantity {
    impl_qty_trig_fn!(sin);
    impl_qty_trig_fn!(cos);
    impl_qty_trig_fn!(tan);
    impl_qty_trig_fn!(asin);
    impl_qty_trig_fn!(acos);
    impl_qty_trig_fn!(atan);
    impl_qty_trig_fn!(sinh);
    impl_qty_trig_fn!(cosh);
    impl_qty_trig_fn!(tanh);
    impl_qty_trig_fn!(asinh);
    impl_qty_trig_fn!(acosh);
    impl_qty_trig_fn!(atanh);

    pub fn real(self) -> Quantity {
        let [re, _] = self.0.realize();
        re * self.1
    }

    pub fn imag(self) -> Quantity {
        let [_, im] = self.0.realize();
        im * self.1
    }

    pub fn log(self, base: Quantity) -> Quantity {
        assert_eq!(base.1, Unit::Unitless, "Log base must be unitless");
        assert_eq!(self.1, Unit::Unitless, "Log arg must be unitless");

        (self.0.ln() / base.0.ln()) * Unit::Unitless
    }

    pub fn atan2(&self, y: &Quantity) -> Quantity {
        assert!(
            self.1 == y.1 || self.0.is_zero() || y.0.is_zero(),
            "atan2(x, y)'s arguments must have the same unit: x = {self}, y = {y}"
        );

        self.0.atan2(&y.0) * rad
    }

    pub fn max(&self, b: &Quantity) -> Quantity {
        assert_eq!(
            self.1, b.1,
            "max(a, b)'s arguments must have the same unit"
        );

        self.0.max(&b.0) * self.1
    }

    pub fn min(&self, b: &Quantity) -> Quantity {
        assert_eq!(
            self.1, b.1,
            "min(a, b)'s arguments must have the same unit"
        );

        self.0.min(&b.0) * self.1
    }

    pub fn sign(self) -> Quantity {
        self.0.sign() * self.1
    }

    pub fn norm(self) -> Quantity {
        self.0.norm() * self.1
    }

    pub fn conj(self) -> Quantity {
        self.0.conj() * self.1
    }

    pub fn arg(self) -> Quantity {
        self.0.arg() * rad
    }
}

impl From<Unit> for Quantity {
    fn from(unit: Unit) -> Self {
        Quantity(1.0.into(), unit)
    }
}

impl From<&Quantity> for Quantity {
    fn from(qty: &Quantity) -> Self {
        qty.clone()
    }
}

impl From<&Value> for Quantity {
    fn from(val: &Value) -> Self {
        val.clone().into()
    }
}

impl_op_permutations! {
    types = [i64, f64, Complex64, Value, &Value, Quantity, &Quantity, Unit],
    exclude_permutations = [i64, f64, Complex64, Value, &Value],
    exclude_specific = [(Unit, Unit)],
    out = Quantity,
    into = Quantity,

    exclude = {
        pow = {
            lhs = [Unit],
            rhs = [Unit, f64, Complex64, Value],
        },
    },

    add = {
        if lhs.value().is_zero() {
            rhs
        } else if rhs.value().is_zero() {
            lhs
        } else {
            assert!(lhs.unit().repr_eq(rhs.unit()), "cannot add two quantities with different units: {} + {}", lhs, rhs);
            Quantity(lhs.value().clone() + rhs.value().clone(), lhs.unit())
        }
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
        if lhs.1 != Unit::Unitless {
            let exp = try { rhs.value().as_scalar()?.as_integer()? }.expect("Non unitless quantities can only be raised to integer powers for now");
            Quantity(lhs.0.pow(exp), lhs.1.pow(exp))
        } else {
            Quantity(lhs.0.pow(rhs.0), Unit::Unitless)
        }
    },

    partial_eq = {
        lhs == rhs
    }
}
