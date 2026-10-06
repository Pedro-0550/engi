use std::num::NonZero;

use super::tree::{Branch, Leaf, Node};
use crate::expr::{Expr, NodeId, tree::Matrix};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct Shape {
    pub rows: NonZero<usize>,
    pub cols: NonZero<usize>,
}

impl From<(usize, usize)> for Shape {
    fn from(value: (usize, usize)) -> Self {
        Self::rect(value.0, value.1)
    }
}

impl Shape {
    // SAFETY:
    // As of August 2026, 1 is not equal to 0.
    // If this changes in the future, use checked version instead.
    pub const SCALAR: Self = unsafe {
        Shape {
            cols: NonZero::<usize>::new_unchecked(1),
            rows: NonZero::<usize>::new_unchecked(1),
        }
    };

    pub fn transpose(self) -> Self {
        Self { rows: self.cols, cols: self.rows }
    }

    pub fn square(size: usize) -> Self {
        Self { rows: size.try_into().unwrap(), cols: size.try_into().unwrap() }
    }

    pub fn rect(rows: usize, cols: usize) -> Self {
        Self { rows: rows.try_into().unwrap(), cols: cols.try_into().unwrap() }
    }

    pub fn is_scalar(&self) -> bool {
        self.rows.get() == 1 && self.cols.get() == 1
    }

    pub fn is_row(&self) -> bool {
        self.rows.get() > 1 && self.cols.get() == 1
    }

    pub fn is_col(&self) -> bool {
        self.rows.get() == 1 && self.cols.get() > 1
    }

    pub fn is_vec(&self) -> bool {
        (self.rows.get() > 1) ^ (self.cols.get() > 1)
    }

    pub fn is_rect_mat(&self) -> bool {
        self.rows.get() > 1 && self.cols.get() > 1
    }

    pub fn is_square_mat(&self) -> bool {
        self.rows.get() > 1 && self.rows == self.rows
    }
}

impl Expr {
    pub fn shape(&self) -> Shape {
        self.shape_of(self.root())
    }

    pub fn shape_of(&self, id: NodeId) -> Shape {
        self.fold_from(id, |_, _, node: Node<Shape>| match node {
            Node::Leaf(leaf) => match leaf {
                Leaf::Symbol(symbol) => symbol.shape(),
                Leaf::Constant(constant) => constant.quantity().value().shape(),
                Leaf::Quantity(quantity) => quantity.value().shape(),
            },
            Node::Branch(branch) => match branch {
                Branch::Add([a, b]) => {
                    assert_eq!(
                        a, b,
                        "Cannot add two values of different shapes",
                    );
                    a
                }
                Branch::Mul(_) => todo!(),
                Branch::Min([a, b]) | Branch::Max([a, b]) => {
                    assert!(
                        a.is_scalar() && b.is_scalar(),
                        "Min/max is only defined for scalar arguments",
                    );

                    Shape::SCALAR
                }
                Branch::Cos(u)
                | Branch::Tan(u)
                | Branch::Asin(u)
                | Branch::Acos(u)
                | Branch::Atan(u)
                | Branch::Sinh(u)
                | Branch::Cosh(u)
                | Branch::Tanh(u)
                | Branch::Asinh(u)
                | Branch::Acosh(u)
                | Branch::Atanh(u)
                | Branch::Sin(u) => {
                    assert!(
                        u.is_scalar() || u.is_square_mat(),
                        "Function only defined for scalars and square matrices"
                    );

                    u
                }
                Branch::Arg(u) => {
                    assert_eq!(
                        u,
                        Shape::SCALAR,
                        "Arg(z) is only defined for scalar z",
                    );

                    Shape::SCALAR
                }
                Branch::Conj(u) | Branch::Sign(u) | Branch::Real(u) | Branch::Imag(u) => u,
                Branch::Norm(_) => Shape::SCALAR,
                Branch::Pow { base, exp } => {
                    assert!(base.is_scalar() || base.is_square_mat(), "Pow base must be a scalar or square matrix");
                    assert!(exp.is_scalar() || exp.is_square_mat(), "Pow exponent must be a scalar or square matrix");

                    if base.is_square_mat() && exp.is_square_mat() {
                        todo!("A^B where A and B are matrices is not implemented yet");
                    } else if base.is_square_mat() {
                        base
                    } else if exp.is_square_mat() {
                        exp
                    } else {
                        Shape::SCALAR
                    }
                },
                Branch::Log { base, arg } => {
                    assert!(base.is_scalar() || base.is_square_mat(), "Log base must be a scalar or square matrix");
                    assert!(arg.is_scalar() || arg.is_square_mat(), "Log argument must be a scalar or square matrix");

                    if base.is_square_mat() && arg.is_square_mat() {
                        todo!("log_A(B) where A and B are matrices is not implemented yet");
                    } else if base.is_square_mat() {
                        base
                    } else if arg.is_square_mat() {
                        arg
                    } else {
                        Shape::SCALAR
                    }
                },
                Branch::Atan2 { x: a, y: b } => {
                    assert!(a.is_scalar() && b.is_scalar(), "Atan arguments must be scalar");
                    Shape::SCALAR
                },
                Branch::Matrix(matrix) => matrix.shape(),
                Branch::Transpose(u) => u.transpose(),
                Branch::Det(_) => todo!(),
                Branch::Rank(_) => todo!(),
                Branch::Trace(_) => todo!(),
                Branch::Conditional { pass, fail, .. } => {
                    assert_eq!(pass, fail, "Both branches of a conditional must be of the same shape");
                    pass
                },
            },
        })
    }

    pub fn scalarize(self) -> Matrix<Expr> {
        todo!()
    }
}
