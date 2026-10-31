use std::{
    fmt::{self, Debug, Display, Formatter, Pointer},
    mem::discriminant,
};

use cranelift::codegen::ir::Opcode;

use super::{
    Operand,
    dag::{Leaf, Operation},
};
use crate::{
    expr::{
        Expr, FetchedOperand, OperandKind,
        dag::OperationKind,
        fmt::Joinabability::{Disjoint, DisjointParens, Joinable, Lhs, Rhs},
    },
    symbol::{Symbol, constants::Constant},
    units::{Quantity, Unit::Unitless},
};

/// Determines whether two operands can be joined in a multiplication
/// When joined, the multiplication sign is omitted as in `2x` or `xyz`
/// Multiplication will be joined in the following situations:
/// Lhs * Joinable
/// Joinable * Rhs
/// Joinable * Joinable
/// Lhs * Rhs
#[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
enum Joinabability {
    /// This operand is not only disjoint, but also needs to be put in parenthesis like `x·(a + b)`
    DisjointParens,
    /// This operand cannot be joined but also dosent need parenthesis, like long symbols
    /// such as v_be or t_initial or functions like sin(x).
    Disjoint,
    /// This operand can be joined when its on the left-hand side of a multiplication,
    /// when the right-hand side is Joinable.
    /// Unitless numbers are only lhs-joinable, as it is visually pleasing to have 2x but not x2
    Lhs,
    /// This operand can be joined when its on the right-hand side of a multiplication,
    /// when the left-hand side is Joinable.
    /// Pow operations are rhs-joinable if their base is joinable, such that 10·x^y is just 10x^y
    Rhs,
    /// This operand is joinable under any circumstances.
    /// Symbols are joinable if their length is 1, like x or y.
    Joinable,
}

impl Expr {
    fn joinability_of(&self, op: Operand) -> Joinabability {
        match self.fetch(op) {
            Operand::Leaf(l) => match l {
                Leaf::Symbol(symbol) if symbol.name().len() == 1 => Joinable,
                Leaf::Constant(constant) if constant.name().len() == 1 => {
                    Joinable
                }
                Leaf::Quantity(quantity) => {
                    if quantity.unit() == Unitless {
                        Joinable
                    } else {
                        DisjointParens
                    }
                }
                _ => Disjoint,
            },
            Operand::Matrix(_) => Joinable,
            Operand::Operation(op) => match op {
                Operation::Add(_) => DisjointParens,
                Operation::Mul([a, b]) => {
                    match (self.joinability_of(*a), self.joinability_of(*b)) {
                        (Rhs, _) => Lhs,
                        (_, Lhs) => Rhs,
                        (Joinable, Joinable) => Joinable,
                        _ => Joinabability::Disjoint,
                    }
                }
                Operation::Min(_)
                | Operation::Max(_)
                | Operation::Sin(_)
                | Operation::Cos(_)
                | Operation::Tan(_)
                | Operation::Asin(_)
                | Operation::Acos(_)
                | Operation::Sinh(_)
                | Operation::Cosh(_)
                | Operation::Tanh(_)
                | Operation::Asinh(_)
                | Operation::Acosh(_)
                | Operation::Atanh(_)
                | Operation::Arg(_)
                | Operation::Conj(_)
                | Operation::Real(_)
                | Operation::Imag(_)
                | Operation::Norm(_)
                | Operation::Sign(_)
                | Operation::Atan(_)
                | Operation::Atan2 { .. }
                | Operation::Det(_)
                | Operation::Rank(_)
                | Operation::Trace(_)
                | Operation::Log { .. } => Disjoint,
                Operation::Conditional { .. } => {
                    todo!()
                }
                Operation::Pow { base, exp } => {
                    if self.joinability_of(*base) == Joinable {
                        Rhs
                    } else {
                        Disjoint
                    }
                }
                Operation::Transpose(_) => Rhs,
            },
            Operand::Wildcard(_) => Joinable,
        }
    }
}

macro_rules! impl_unary_fmt {
    ($f:ident, $x:ident, $expr:ident, $fn:ident) => {{
        write!($f, concat!(stringify!($fn), "("))?;
        fmt_inner($expr, *$x, $f)?;
        write!($f, ")")
    }};
}

impl Display for Expr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        fn fmt_inner(
            expr: &Expr,
            op: Operand,
            f: &mut Formatter<'_>,
        ) -> fmt::Result {
            match expr.fetch(op) {
                Operand::Leaf(l) => match l {
                    Leaf::Symbol(symbol) => <Symbol as Display>::fmt(symbol, f),
                    Leaf::Constant(constant) => {
                        <Constant as Display>::fmt(constant, f)
                    }
                    Leaf::Quantity(qty) => <Quantity as Display>::fmt(qty, f),
                },
                Operand::Matrix(_) => todo!(),
                Operand::Operation(o) => match o {
                    Operation::Add([a, b]) => {
                        fmt_inner(expr, *a, f)?;
                        write!(f, " + ")?;
                        fmt_inner(expr, *b, f)
                    }
                    Operation::Mul(_) => todo!(),
                    Operation::Min([a, b]) | Operation::Max([a, b]) => {
                        let mut flattened = Vec::new();

                        fn push_flattened(
                            expr: &Expr,
                            op: Operand,
                            target: OperationKind,
                            into: &mut Vec<Operand>,
                        ) {
                            match expr.fetch(op) {
                                Operand::Operation(o) if o.kind() == target => {
                                    o.for_each_operand(|op| {
                                        push_flattened(expr, *op, target, into)
                                    });
                                }
                                _ => into.push(op),
                            }
                        }

                        push_flattened(expr, *a, o.kind(), &mut flattened);
                        push_flattened(expr, *b, o.kind(), &mut flattened);

                        if o.kind() == OperationKind::Max {
                            write!(f, "max(")?;
                        } else {
                            write!(f, "min(")?;
                        }
                        for op in flattened {
                            fmt_inner(expr, op, f)?;
                            write!(f, ", ")?;
                        }

                        write!(f, ")")
                    }
                    Operation::Sin(x) => impl_unary_fmt!(f, x, expr, sin),
                    Operation::Cos(x) => impl_unary_fmt!(f, x, expr, cos),
                    Operation::Tan(x) => impl_unary_fmt!(f, x, expr, tan),
                    Operation::Asin(x) => impl_unary_fmt!(f, x, expr, asin),
                    Operation::Acos(x) => impl_unary_fmt!(f, x, expr, acos),
                    Operation::Atan(x) => impl_unary_fmt!(f, x, expr, atan),
                    Operation::Sinh(x) => impl_unary_fmt!(f, x, expr, sinh),
                    Operation::Cosh(x) => impl_unary_fmt!(f, x, expr, cosh),
                    Operation::Tanh(x) => impl_unary_fmt!(f, x, expr, tanh),
                    Operation::Asinh(x) => impl_unary_fmt!(f, x, expr, asinh),
                    Operation::Acosh(x) => impl_unary_fmt!(f, x, expr, acosh),
                    Operation::Atanh(x) => impl_unary_fmt!(f, x, expr, atanh),
                    Operation::Arg(x) => impl_unary_fmt!(f, x, expr, arg),
                    Operation::Conj(x) => impl_unary_fmt!(f, x, expr, conj),
                    Operation::Real(x) => impl_unary_fmt!(f, x, expr, real),
                    Operation::Imag(x) => impl_unary_fmt!(f, x, expr, imag),
                    Operation::Atan2 { x, y } => todo!(),
                    Operation::Norm(_) => todo!(),
                    Operation::Sign(_) => todo!(),
                    Operation::Pow { base, exp } => todo!(),
                    Operation::Log { base, arg } => todo!(),
                    Operation::Transpose(_) => todo!(),
                    Operation::Det(_) => todo!(),
                    Operation::Rank(_) => todo!(),
                    Operation::Trace(_) => todo!(),
                    Operation::Conditional { cond, pass, fail } => todo!(),
                },
                Operand::Wildcard(_) => unreachable!(),
            }
        }

        fmt_inner(self, self.dag.root.unwrap(), f)
    }
}

impl Debug for Expr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        <Self as Display>::fmt(&self, f)
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
