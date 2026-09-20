use crate::expr::Expr;

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum Domain {
    Real,
    Imag,
    Complex,
}

impl Expr {
    fn domain(&self) -> Domain {}
    fn realize(self) -> [Expr; 2] {}
}
