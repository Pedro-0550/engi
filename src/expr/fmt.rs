use std::fmt::{self, Debug, Display, Formatter, Pointer, Write};

use super::dag::Leaf;
use crate::{
    core::value::ComplexExt,
    expr::{Expr, OperationId, Order, fmt::Joinabability::WithParens},
    symbol::constants,
    units::Unit::Unitless,
};

#[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
enum Joinabability {
    Disjoint,
    AtStart,
    WithParens,
    Joinable,
}

impl Display for Expr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl Debug for Expr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

#[cfg(test)]
mod test {
    use ordered_float::Pow;

    use crate as engi;
    use crate::symbols;

    #[test]
    fn mul() {
        symbols!(x, y, z);
        assert_eq!(((x * y * z * 10 * 2) + 5).to_string(), "xyz·10·2 + 5");
        assert_eq!(((x * y * 10 * 2 * z) + 5).to_string(), "xy·10·2z + 5");
        assert_eq!((x * (10 + z)).to_string(), "x·(10 + z)");
        assert_eq!(((10 + x) * (10 + z)).to_string(), "(10 + x)·(10 + z)");
    }

    #[test]
    fn div() {
        symbols!(x, y, z);
        assert_eq!((x / y).to_string(), "x / y");
        assert_eq!((x / (y * z)).to_string(), "x / yz");

        assert_eq!((x / (y + 2)).to_string(), "x / (y + 2)");
        assert_eq!(((x / (y + 2)) * z).to_string(), "xz / (y + 2)");

        assert_eq!((x / (y + 2).pow(-1)).to_string(), "x·(y + 2)");
    }

    #[test]
    fn sub() {
        symbols!(x, y);

        assert_eq!((x * -1).to_string(), "-x");
        assert_eq!((x * -2).to_string(), "-2x");
        assert_eq!((-x * -1).to_string(), "-(-x)");
        assert_eq!((1 + (-x)).to_string(), "1 - x");
        assert_eq!((-x + 1).to_string(), "-x + 1");
        assert_eq!((-(x * y)).to_string(), "-xy");
        assert_eq!((-(x / y)).to_string(), "-x / y");
        assert_eq!((-(-x / -y)).to_string(), "-(-x) / -y");
    }

    #[test]
    fn pow() {
        symbols!(x, y, z);

        assert_eq!((10 * x.pow(2) * y).to_string(), "10x²y");
        assert_eq!((10 * x.pow(2 * y)).to_string(), "10x^(2 + y)");
        assert_eq!(((10 * x).pow(2 * y)).to_string(), "(10x)^(2 + y)");
        assert_eq!((10 * x.pow(2 * y) * z).to_string(), "(10x^(2 + y))·z");
        assert_eq!((10.pow(x) * y.pow(x)).to_string(), "(10^x)·(y^x)");
    }
}
