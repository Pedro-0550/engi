use std::num::NonZero;

use super::dag::Leaf;
use crate::expr::{
    Expr, FoldedOperand, Operand, OperationId,
    dag::{self, Matrix, Operation},
};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct Shape {
    pub rows: NonZero<dag::Size>,
    pub cols: NonZero<dag::Size>,
}

impl From<(dag::Size, dag::Size)> for Shape {
    fn from(value: (dag::Size, dag::Size)) -> Self {
        Self::rect(value.0, value.1)
    }
}

impl Shape {
    // SAFETY:
    // As of August 2026, 1 is not equal to 0.
    // If this changes in the future, use checked version instead.
    pub const SCALAR: Self = unsafe {
        Shape {
            cols: NonZero::<dag::Size>::new_unchecked(1),
            rows: NonZero::<dag::Size>::new_unchecked(1),
        }
    };

    pub fn transpose(self) -> Self {
        Self { rows: self.cols, cols: self.rows }
    }

    pub fn square(size: dag::Size) -> Self {
        Self { rows: size.try_into().unwrap(), cols: size.try_into().unwrap() }
    }

    pub fn rect(rows: dag::Size, cols: dag::Size) -> Self {
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

impl FoldedOperand<'_, Shape> {
    pub fn shape(&self) -> Shape {
        match self {
            Operand::Symbol(symbol) => symbol.shape(),
            Operand::Leaf(leaf) => match leaf {
                Leaf::Constant(constant) => constant.quantity().value().shape(),
                Leaf::Quantity(quantity) => quantity.value().shape(),
            },
            Operand::Operation(operation) => match *operation {
                Operation::Add([a, b]) => {
                    assert_eq!(
                        a, b,
                        "Cannot add two values of different shapes",
                    );
                    *a
                }
                Operation::Mul([a, b]) => {
                    if a.cols == b.rows {
                        Shape::rect(a.rows.get(), b.cols.get())
                    } else if a.is_vec() && b.is_vec() {
                        Shape::SCALAR
                    } else {
                        panic!(
                            "a * b must satisfy cols(a) == rows(b) for matrix multiplication, unless a and b are vectors, in which case it will be the dot product instead"
                        )
                    }
                }
                Operation::Min([a, b]) | Operation::Max([a, b]) => {
                    assert!(
                        a.is_scalar() && b.is_scalar(),
                        "Min/max is only defined for scalar arguments",
                    );

                    Shape::SCALAR
                }
                Operation::Cos(u)
                | Operation::Tan(u)
                | Operation::Asin(u)
                | Operation::Acos(u)
                | Operation::Atan(u)
                | Operation::Sinh(u)
                | Operation::Cosh(u)
                | Operation::Tanh(u)
                | Operation::Asinh(u)
                | Operation::Acosh(u)
                | Operation::Atanh(u)
                | Operation::Sin(u) => {
                    assert!(
                        u.is_scalar() || u.is_square_mat(),
                        "Function only defined for scalars and square matrices"
                    );

                    *u
                }
                Operation::Arg(u) => {
                    assert_eq!(
                        *u,
                        Shape::SCALAR,
                        "Arg(z) is only defined for scalar z",
                    );

                    Shape::SCALAR
                }
                Operation::Conj(u)
                | Operation::Sign(u)
                | Operation::Real(u)
                | Operation::Imag(u) => *u,
                Operation::Norm(_) => Shape::SCALAR,
                Operation::Pow { base, exp } => {
                    assert!(
                        base.is_scalar() || base.is_square_mat(),
                        "Pow base must be a scalar or square matrix"
                    );
                    assert!(
                        exp.is_scalar() || exp.is_square_mat(),
                        "Pow exponent must be a scalar or square matrix"
                    );

                    if base.is_square_mat() && exp.is_square_mat() {
                        todo!(
                            "A^B where A and B are matrices is not implemented yet"
                        );
                    } else if base.is_square_mat() {
                        *base
                    } else if exp.is_square_mat() {
                        *exp
                    } else {
                        Shape::SCALAR
                    }
                }
                Operation::Log { base, arg } => {
                    assert!(
                        base.is_scalar() || base.is_square_mat(),
                        "Log base must be a scalar or square matrix"
                    );
                    assert!(
                        arg.is_scalar() || arg.is_square_mat(),
                        "Log argument must be a scalar or square matrix"
                    );

                    if base.is_square_mat() && arg.is_square_mat() {
                        todo!(
                            "log_A(B) where A and B are matrices is not implemented yet"
                        );
                    } else if base.is_square_mat() {
                        *base
                    } else if arg.is_square_mat() {
                        *arg
                    } else {
                        Shape::SCALAR
                    }
                }
                Operation::Atan2 { x: a, y: b } => {
                    assert!(
                        a.is_scalar() && b.is_scalar(),
                        "Atan arguments must be scalar"
                    );
                    Shape::SCALAR
                }
                Operation::Transpose(u) => u.transpose(),
                Operation::Det(_) => todo!(),
                Operation::Rank(_) => todo!(),
                Operation::Trace(_) => todo!(),
                Operation::Conditional { pass, fail, .. } => {
                    assert_eq!(
                        pass, fail,
                        "Both branches of a conditional must be of the same shape"
                    );
                    *pass
                }
            },
            Operand::Matrix(mat) => mat.shape(),
            Operand::Wildcard(never) => *never,
        }
    }
}

impl Expr {
    pub fn shape(&self) -> Shape {
        self.shape_of(self.root().unwrap())
    }

    pub fn shape_of(&self, id: Operand) -> Shape {
        self.fold_from(id, |_, node: FoldedOperand<'_, Shape>| node.shape())
    }

    pub fn scalarize(self) -> Matrix<Expr> {
        todo!()
    }
}
