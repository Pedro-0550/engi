use ordered_float::Pow;

use crate as engi;
use crate::{
    expr::{cos, cosh, ln, log, sin, sinh, tan},
    symbols,
};

#[test]
fn diff() {
    symbols!(x, y);

    let f_of_xy = (x.pow(3) + 2.0 * x * y + y.pow(2) + 1.0)
        * sin(x * y + x.pow(2))
        * cos(y.pow(2) + x)
        * ln((x.pow(2) + y.pow(2) + 1.0) / (x + y))
        + ((x + 1.0).pow(y)) * sinh(x * y) * cosh(x.pow(2) - y)
        + (x.pow(2) * y + x * y.pow(2) + 1.0)
            * log(x + y, x.pow(2) + y + 1.0)
            * tan(x * y);

    panic!("{}", f_of_xy.diff(x));
}
