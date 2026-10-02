use std::fmt::{Debug, Display};

use super::tree::{Branch, Leaf, Node};
use crate::expr::{Expr, Order};

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for id in self.dfs(Order::In) {
            let node = self.node(id);

            match node {
                Node::Leaf(leaf) => match leaf {
                    Leaf::Symbol(symbol) => f.write_str(symbol.name()),
                    Leaf::Constant(constant) => f.write_str(constant.name()),
                    Leaf::Quantity(quantity) => Display::fmt(&quantity, f),
                },
                Node::Branch(branch) => match branch {
                    Branch::Add(_) => f.write_str(" + "),
                    Branch::Mul(_) => f.write_str(" * "),
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
            }?
        }

        Ok(())
    }
}

impl Debug for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}
