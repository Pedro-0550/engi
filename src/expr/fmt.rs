use std::fmt::{self, Debug, Display, Formatter, Pointer, Write};

use super::dag::{Branch, Leaf, Node};
use crate::{
    core::value::ComplexExt,
    expr::{Expr, NodeId, Order, fmt::Joinabability::WithParens},
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
        fn joinability(expr: &Expr, id: NodeId) -> Joinabability {
            match expr.node(id) {
                Node::Leaf(leaf) => match leaf {
                    Leaf::Symbol(symbol) => Joinabability::Joinable,
                    Leaf::Constant(constant) => Joinabability::Joinable,
                    Leaf::Quantity(quantity) => {
                        if quantity.unit() == Unitless
                            && quantity.value().is_scalar_real()
                        {
                            Joinabability::AtStart
                        } else {
                            Joinabability::Disjoint
                        }
                    }
                },
                Node::Branch(branch) => match branch {
                    Branch::Add(_) => Joinabability::WithParens,
                    Branch::Mul([a, _]) => match joinability(expr, *a) {
                        Joinabability::Joinable | Joinabability::WithParens => {
                            Joinabability::Joinable
                        }
                        _ => Joinabability::Disjoint,
                    },
                    Branch::Min(_)
                    | Branch::Max(_)
                    | Branch::Sin(_)
                    | Branch::Cos(_)
                    | Branch::Tan(_)
                    | Branch::Asin(_)
                    | Branch::Acos(_)
                    | Branch::Atan(_)
                    | Branch::Sinh(_)
                    | Branch::Cosh(_)
                    | Branch::Tanh(_)
                    | Branch::Asinh(_)
                    | Branch::Acosh(_)
                    | Branch::Atanh(_)
                    | Branch::Arg(_)
                    | Branch::Conj(_)
                    | Branch::Norm(_)
                    | Branch::Sign(_)
                    | Branch::Real(_)
                    | Branch::Imag(_) => Joinabability::Disjoint,
                    Branch::Pow { base, exp } => {
                        let exp_z = try {
                            expr.node(*exp)
                                .as_leaf()?
                                .as_quantity()?
                                .value()
                                .as_scalar()?
                                .as_integer()?
                        };

                        if exp_z.is_some() {
                            Joinabability::Joinable
                                .min(joinability(expr, *base))
                        } else {
                            Joinabability::Disjoint
                        }
                    }
                    Branch::Log { base, arg } => Joinabability::Disjoint,
                    Branch::Atan2 { x: a, y: b } => Joinabability::Disjoint,
                    Branch::Matrix(matrix) => todo!(),
                    Branch::Transpose(_) => todo!(),
                    Branch::Det(_) => todo!(),
                    Branch::Rank(_) => todo!(),
                    Branch::Trace(_) => todo!(),
                    Branch::Conditional { cond, pass, fail } => todo!(),
                },
            }
        }

        fn fmt_inner(
            expr: &Expr,
            f: &mut Formatter<'_>,
            id: NodeId,
        ) -> fmt::Result {
            let node = expr.node(id);

            match node {
                Node::Leaf(leaf) => match leaf {
                    Leaf::Symbol(symbol) => f.write_str(symbol.name())?,
                    Leaf::Constant(constant) => f.write_str(constant.name())?,
                    Leaf::Quantity(quantity) => Display::fmt(&quantity, f)?,
                },
                Node::Branch(branch) => match branch {
                    Branch::Add([a, b]) => {
                        fmt_inner(expr, f, *a)?;
                        f.write_str(" + ")?;
                        fmt_inner(expr, f, *b)?;
                    }
                    Branch::Mul([a, b]) => {
                        let (ja, jb) =
                            (joinability(expr, *a), joinability(expr, *b));

                        f.write_char('(')?;
                        fmt_inner(expr, f, *a)?;
                        f.write_char(')')?;

                        f.write_str("·")?;

                        f.write_char('(')?;
                        fmt_inner(expr, f, *b)?;
                        f.write_char(')')?;
                    }
                    Branch::Min(_) => todo!(),
                    Branch::Max(_) => todo!(),
                    Branch::Sin(u) => {
                        f.write_str("sin(")?;
                        fmt_inner(expr, f, *u)?;
                        f.write_char(')')?;
                    }
                    Branch::Cos(u) => {
                        f.write_str("cos(")?;
                        fmt_inner(expr, f, *u)?;
                        f.write_char(')')?;
                    }
                    Branch::Tan(u) => {
                        f.write_str("tan(")?;
                        fmt_inner(expr, f, *u)?;
                        f.write_char(')')?;
                    }
                    Branch::Asin(_) => todo!(),
                    Branch::Acos(_) => todo!(),
                    Branch::Atan(_) => todo!(),
                    Branch::Sinh(u) => {
                        f.write_str("sinh(")?;
                        fmt_inner(expr, f, *u)?;
                        f.write_char(')')?;
                    }
                    Branch::Cosh(u) => {
                        f.write_str("cosh(")?;
                        fmt_inner(expr, f, *u)?;
                        f.write_char(')')?;
                    }
                    Branch::Tanh(_) => todo!(),
                    Branch::Asinh(_) => todo!(),
                    Branch::Acosh(_) => todo!(),
                    Branch::Atanh(_) => todo!(),
                    Branch::Arg(_) => todo!(),
                    Branch::Conj(_) => todo!(),
                    Branch::Norm(_) => todo!(),
                    Branch::Sign(_) => todo!(),
                    Branch::Real(_) => todo!(),
                    Branch::Imag(_) => todo!(),
                    Branch::Pow { base, exp } => {
                        if joinability(expr, *base) <= Joinabability::WithParens
                        {
                            f.write_str("(")?;
                            fmt_inner(expr, f, *base)?;
                            f.write_char(')')?;
                        } else {
                            fmt_inner(expr, f, *base)?;
                        }

                        f.write_char('^')?;

                        if joinability(expr, *exp) <= Joinabability::WithParens
                        {
                            f.write_str("(")?;
                            fmt_inner(expr, f, *exp)?;
                            f.write_char(')')?;
                        } else {
                            fmt_inner(expr, f, *exp)?;
                        }
                    }
                    Branch::Log { base, arg } => {
                        if try { expr.node(*base).as_leaf()?.as_constant()? }
                            .is_some_and(|x| *x == constants::e)
                        {
                            f.write_str("ln(")?;
                            fmt_inner(expr, f, *arg)?;
                            f.write_char(')')?;
                        } else {
                            f.write_str("log(")?;
                            fmt_inner(expr, f, *base)?;
                            f.write_str(", ")?;
                            fmt_inner(expr, f, *arg)?;
                            f.write_char(')')?;
                        }
                    }
                    Branch::Atan2 { x, y } => {
                        f.write_str("atan2(")?;
                        fmt_inner(expr, f, *x)?;
                        f.write_str(", ")?;
                        fmt_inner(expr, f, *y)?;
                        f.write_char(')')?;
                    }
                    Branch::Matrix(matrix) => todo!(),
                    Branch::Transpose(_) => todo!(),
                    Branch::Det(_) => todo!(),
                    Branch::Rank(_) => todo!(),
                    Branch::Trace(_) => todo!(),
                    Branch::Conditional { cond, pass, fail } => todo!(),
                },
            };

            Ok(())
        }

        fmt_inner(self, f, self.root)
    }
}

impl Debug for Expr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

#[cfg(test)]
mod test {
    use crate as engi;
    use crate::symbols;

    #[test]
    fn formatting() {
        symbols!(x, y, z);
        assert_eq!(((x * y * z * 10 * 2) + 5).to_string(), "xyz·10·2 + 5");
        assert_eq!(((x * y * 10 * 2 * z) + 5).to_string(), "xy·10·2z + 5");
    }
}
