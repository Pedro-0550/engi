use std::fmt::{self, Debug, Display, Formatter, Pointer, Write};

use super::tree::{Branch, Leaf, Node};
use crate::{
    core::value::ComplexExt,
    expr::{Expr, NodeId, Order, fmt::Joinabability::WithParens},
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
                    Branch::Atan2 { a, b } => Joinabability::Disjoint,
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

                        if ja.min(jb) >= WithParens {
                            if ja == WithParens {
                                f.write_char('(')?;
                                fmt_inner(expr, f, *a)?;
                                f.write_char(')')?;
                            } else {
                                fmt_inner(expr, f, *a)?;
                            }

                            if jb == WithParens {
                                f.write_char('(')?;
                                fmt_inner(expr, f, *b)?;
                                f.write_char(')')?;
                            } else {
                                fmt_inner(expr, f, *b)?;
                            }
                        } else {
                            fmt_inner(expr, f, *a)?;
                            f.write_char('·')?;
                            fmt_inner(expr, f, *b)?;
                        }
                    }
                    Branch::Min(_) => todo!(),
                    Branch::Max(_) => todo!(),
                    Branch::Sin(_) => todo!(),
                    Branch::Cos(_) => todo!(),
                    Branch::Tan(_) => todo!(),
                    Branch::Asin(_) => todo!(),
                    Branch::Acos(_) => todo!(),
                    Branch::Atan(_) => todo!(),
                    Branch::Sinh(_) => todo!(),
                    Branch::Cosh(_) => todo!(),
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
                    Branch::Pow { base, exp } => todo!(),
                    Branch::Log { base, arg } => todo!(),
                    Branch::Atan2 { a, b } => todo!(),
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
