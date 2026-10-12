// /* --------------------------------- MODULES -------------------------------- */
pub mod dag;
pub mod domain;
pub mod fmt;
pub mod jit;
pub mod ops;
pub mod shape;

use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    iter::empty,
    mem,
    ops::{Add, Div, Mul, Neg, Sub},
    slice,
    sync::mpsc::Iter,
};

use ahash::AHashMap;
use derive_more::IsVariant;
use faer::{linalg::svd::ComputeSvdVectors::No, traits::ByRef};
use itertools::Itertools;
use kinded::Kinded;
use num::complex::{Complex64, c64};
use ordered_float::Pow;
use xxhash_rust::xxh3::{Xxh3, Xxh3Builder};

use crate::{
    core::{util::impl_as_variant, value::Value},
    expr::{
        dag::{
            Condition, ConditionId, Dag, Leaf, LeafId, Matrix, MatrixId,
            Operation, OperationId,
        },
        shape::Shape,
    },
    model::Variable,
    simplify::{self, EquivalencyGraph, rules},
    symbol::{
        Symbol,
        constants::{Constant, e},
    },
    units::{Quantity, Unit::Unitless},
};

/// A mathematical expression based on a directed acryllic graph
///
/// An Expr is just a [`Dag`] where an `Operand::Operation` refers to an operation.
/// In other words, a node refers to other nodes.
///
/// This is in contrast to an [`EquivalencyGraph`], where a node refers to a class.
#[derive(Clone)]
pub struct Expr {
    dag: Dag<Operand, OperationId>,
}

/// An [`Operation`]'s operand. Can be either a leaf, a matrix, or the result of another operation.
/// This is decoupled from the DAG because in Exprs an operand refers to either an operation, a matrix or a leaf,
/// while in an equivalency graph it refers only to classes.
#[derive(Eq, PartialEq, Clone, Copy, Hash, IsVariant)]
pub enum Operand<O = OperationId, L = LeafId, M = MatrixId> {
    Leaf(L),
    Matrix(M),
    Operation(O),
}

type FetchedOperand<'a> =
    Operand<&'a Operation<Operand, ConditionId>, &'a Leaf, &'a Matrix<Operand>>;

type BuiltOperand =
    Operand<Operation<Operand, ConditionId>, Leaf, Matrix<Operand>>;

type FoldedOperand<'a, T> =
    Operand<Operation<&'a T, ()>, &'a Leaf, Matrix<&'a T>>;

pub struct EditContext<'e> {
    expr: Cell<Option<&'e mut Expr>>,
}

#[derive(Eq, Clone, Copy, PartialEq)]
enum Order {
    Post,
    Pre,
}

/* ---------------------------------- IMPLS --------------------------------- */

impl Expr {
    pub fn fetch(&self, op: Operand) -> FetchedOperand<'_> {
        match op {
            Operand::Leaf(id) => Operand::Leaf(&self.dag.leaves[id]),
            Operand::Matrix(id) => Operand::Matrix(&self.dag.matrices[id]),
            Operand::Operation(id) => {
                Operand::Operation(&self.dag.operations[id])
            }
        }
    }
}

impl FetchedOperand<'_> {
    // pub fn iter_children(&self) -> slice::Iter<'_, Operand> {
    //     match self {
    //         Operand::Leaf(_) => [].iter(),
    //         Operand::Matrix(m) => m.elements().iter(),
    //         Operand::Operation(op) => op.,
    //     }
    // }

    pub fn for_each_operand(&self, f: impl FnMut(&Operand)) {
        match self {
            Operand::Leaf(_) => (),
            Operand::Matrix(m) => m.elements().iter().for_each(f),
            Operand::Operation(op) => op.for_each_operand(f),
        }
    }

    pub fn child(&self, at: usize) -> Option<Operand> {
        match self {
            Operand::Leaf(_) => None,
            Operand::Matrix(m) => m.elements().get(at).copied(),
            Operand::Operation(op) => op.operand(at).copied(),
        }
    }
}

impl<O, L, M> Operand<O, L, M> {
    pub fn as_leaf(&self) -> Option<&L> {
        if let Self::Leaf(v) = self { Some(v) } else { None }
    }

    pub fn as_matrix(&self) -> Option<&M> {
        if let Self::Matrix(v) = self { Some(v) } else { None }
    }

    pub fn as_operation(&self) -> Option<&O> {
        if let Self::Operation(v) = self { Some(v) } else { None }
    }
}

impl Expr {
    pub fn size(&self) -> usize {
        self.dag.operations.len()
            + self.dag.conditions.len()
            + self.dag.leaves.len()
            + self.dag.matrices.len()
    }

    pub fn root(&self) -> Option<Operand> {
        self.dag.root
    }

    pub fn edit(&mut self) -> EditContext<'_> {
        EditContext { expr: Cell::new(Some(self)) }
    }

    pub fn as_single(&self) -> Option<Operand> {
        if self.size() == 1 { self.dag.root } else { None }
    }

    pub fn substituted(&self, bindings: &AHashMap<Symbol, Expr>) -> Expr {
        if bindings.is_empty() {
            return self.clone();
        }

        struct RebuildContext<'a> {
            mapped_conds: AHashMap<ConditionId, ConditionId>,
            mapped_ops: AHashMap<Operand, Operand>,
            bindings: &'a AHashMap<Symbol, Expr>,
            out: Expr,
            src: &'a Expr,
        }

        fn rebuild_condition(
            cond: ConditionId,
            ctx: &mut RebuildContext<'_>,
        ) -> ConditionId {
            if let Some(&existing) = ctx.mapped_conds.get(&cond) {
                return existing;
            }

            let mapped = ctx.src.dag.conditions[cond].map_with(
                ctx,
                |ctx, op| rebuild_operand(*op, ctx),
                |ctx, cond| rebuild_condition(*cond, ctx),
            );

            let mapped_id = ctx.out.dag.conditions.insert(mapped);
            ctx.mapped_conds.insert(cond, mapped_id);

            mapped_id
        }

        fn rebuild_operand(
            op: Operand,
            ctx: &mut RebuildContext<'_>,
        ) -> Operand {
            if let Some(&existing) = ctx.mapped_ops.get(&op) {
                return existing;
            }

            let new_op = match op {
                Operand::Leaf(id) => {
                    let leaf = &ctx.src.dag.leaves[id];
                    if let Leaf::Symbol(s) = leaf
                        && ctx.bindings.contains_key(&s)
                    {
                        let bound = &ctx.bindings[&s];
                        ctx.out.import(
                            bound,
                            bound.root().expect(
                                "Cannot substitute for a rootless expression",
                            ),
                        )
                    } else {
                        Operand::Leaf(ctx.out.dag.leaves.insert(leaf.clone()))
                    }
                }
                Operand::Matrix(id) => {
                    let mapped_matrix = ctx.src.dag.matrices[id]
                        .map(|child| rebuild_operand(*child, ctx));
                    Operand::Matrix(ctx.out.dag.matrices.insert(mapped_matrix))
                }
                Operand::Operation(id) => {
                    let mapped_operation =
                        ctx.src.dag.operations[id].clone().map_with(
                            ctx,
                            |ctx, op| rebuild_operand(*op, ctx),
                            |ctx, cond| rebuild_condition(*cond, ctx),
                        );
                    Operand::Operation(
                        ctx.out.dag.operations.insert(mapped_operation),
                    )
                }
            };

            ctx.mapped_ops.insert(op, new_op);
            new_op
        }

        let mut ctx = RebuildContext {
            mapped_conds: AHashMap::new(),
            mapped_ops: AHashMap::new(),
            bindings,
            out: Expr::new(),
            src: &self,
        };

        let root = rebuild_operand(
            self.root().expect("Cannot substitute for a rootless expression"),
            &mut ctx,
        );
        ctx.out.set_root(root);
        ctx.out
    }

    pub fn push(&mut self, operand: BuiltOperand) -> Operand {
        let op = match operand {
            Operand::Leaf(l) => Operand::Leaf(self.dag.leaves.insert(l)),
            Operand::Matrix(m) => Operand::Matrix(self.dag.matrices.insert(m)),
            Operand::Operation(op) => {
                Operand::Operation(self.dag.operations.insert(op))
            }
        };
        op
    }

    pub fn push_root(&mut self, new_root: BuiltOperand) -> Operand {
        let op = self.push(new_root);
        self.dag.root = Some(op);
        op
    }

    pub fn set_root(&mut self, new_root: Operand) {
        self.dag.root = Some(new_root);
    }

    fn dfs(&self, order: Order) -> Vec<Operand> {
        let mut visit = Vec::with_capacity(self.size());

        fn dfs_inner<'e>(
            expr: &'e Expr,
            current: Operand,
            into: &mut Vec<Operand>,
            order: &Order,
        ) {
            if *order == Order::Pre {
                into.push(current);
            }

            expr.fetch(current).for_each_operand(|child| {
                dfs_inner(expr, *child, into, order);
            });

            if *order == Order::Post {
                into.push(current);
            }
        }

        dfs_inner(
            self,
            self.root().expect("Cannot DFS-traverse a rootless expression"),
            &mut visit,
            &order,
        );
        visit
    }

    pub fn fold<T: Clone>(
        &self,
        f: impl FnMut(Operand, FoldedOperand<'_, T>) -> T,
    ) -> T {
        self.fold_from(
            self.root().expect("Cannot fold root of a rootless expression"),
            f,
        )
    }

    pub fn fold_from<T: Clone>(
        &self,
        root: Operand,
        mut f: impl FnMut(Operand, FoldedOperand<'_, T>) -> T,
    ) -> T {
        let mut mapped = AHashMap::<Operand, T>::with_capacity(self.size());

        for op in self.dfs(Order::Post) {
            if mapped.contains_key(&op) {
                continue;
            }

            let mapped_node = match self.fetch(op) {
                Operand::Leaf(l) => Operand::Leaf(l),
                Operand::Matrix(m) => {
                    Operand::Matrix(m.map(|el| mapped.get(el).unwrap()))
                }
                Operand::Operation(op) => Operand::Operation(
                    op.map(|child| mapped.get(child).unwrap(), |_| ()),
                ),
            };

            let result = f(op, mapped_node);
            mapped.insert(op, result);
        }

        mapped.remove(&root).expect("root node should be folded")
    }

    /// Imports an entire operand's subtree from another Expr, mapping Ids appropriately
    pub fn import(&mut self, src: &Expr, src_id: Operand) -> Operand {
        struct ImportContext<'a> {
            dest: &'a mut Expr,
            src: &'a Expr,
            mapped_ops: AHashMap<Operand, Operand>,
            mapped_conds: AHashMap<ConditionId, ConditionId>,
        }

        fn import_condition(
            src_id: ConditionId,
            ctx: &mut ImportContext<'_>,
        ) -> ConditionId {
            if let Some(&existing) = ctx.mapped_conds.get(&src_id) {
                return existing;
            }

            let mapped = ctx.src.dag.conditions[src_id].map_with(
                ctx,
                |ctx, op| import_operand(*op, ctx),
                |ctx, cond| import_condition(*cond, ctx),
            );

            let mapped_id = ctx.dest.dag.conditions.insert(mapped);
            ctx.mapped_conds.insert(src_id, mapped_id);

            mapped_id
        }

        fn import_operand(
            src_op: Operand,
            ctx: &mut ImportContext<'_>,
        ) -> Operand {
            if let Some(&mapped) = ctx.mapped_ops.get(&src_op) {
                return mapped;
            }

            let mapped_op = match ctx.src.fetch(src_op) {
                Operand::Leaf(l) => {
                    Operand::Leaf(ctx.dest.dag.leaves.insert(l.clone()))
                }
                Operand::Matrix(mat) => {
                    let mapped_matrix = mat.map(|op| import_operand(*op, ctx));
                    Operand::Matrix(ctx.dest.dag.matrices.insert(mapped_matrix))
                }
                Operand::Operation(op) => {
                    let mapped_operation = op.map_with(
                        ctx,
                        |ctx, op| import_operand(*op, ctx),
                        |ctx, cond| import_condition(*cond, ctx),
                    );
                    Operand::Operation(
                        ctx.dest.dag.operations.insert(mapped_operation),
                    )
                }
            };

            ctx.mapped_ops.insert(src_op, mapped_op);
            mapped_op
        }

        let mut ctx = ImportContext {
            dest: self,
            mapped_conds: AHashMap::new(),
            mapped_ops: AHashMap::new(),
            src,
        };

        import_operand(src_id, &mut ctx)
    }

    pub fn new() -> Self {
        Self { dag: Dag::new() }
    }

    pub fn symbols(&self) -> impl Iterator<Item = Symbol> {
        self.dag.leaves.iter().filter_map(|x| x.as_symbol().copied())
    }

    pub fn simplified(&self) -> Expr {
        let mut egraph = EquivalencyGraph::build(&*self);
        egraph.rewrite(&[rules::algebraic(), rules::trig()].concat());
        egraph.extract()
    }
}

// impl PartialEq for Expr {
//     fn eq(&self, other: &Self) -> bool {
//         self.nodes[self.root.0].0 == other.nodes[other.root.0].0
//     }
// }

// impl Hash for Expr {
//     fn hash<H: Hasher>(&self, state: &mut H) {
//         self.nodes[self.root.0].0.hash(state);
//     }
// }

/* -------------------------------------------------------------------------- */

impl Mul<Expr> for Expr {
    type Output = Expr;

    fn mul(mut self, rhs: Expr) -> Self::Output {
        let a_id = self.root().unwrap();
        let b_id = self.import(&rhs, rhs.root().unwrap());
        self.push_root(Operand::Operation(Operation::Mul([a_id, b_id])));
        self
    }
}

impl Add<Expr> for Expr {
    type Output = Expr;

    fn add(mut self, rhs: Expr) -> Self::Output {
        let a_id = self.root().unwrap();
        let b_id = self.import(&rhs, rhs.root().unwrap());
        self.push_root(Operand::Operation(Operation::Add([a_id, b_id])));
        self
    }
}

impl Sub<Expr> for Expr {
    type Output = Expr;

    fn sub(self, rhs: Expr) -> Self::Output {
        self + -rhs
    }
}

impl Div<Expr> for Expr {
    type Output = Expr;

    fn div(self, rhs: Expr) -> Self::Output {
        self * rhs.pow(-1)
    }
}

impl Pow<Expr> for Expr {
    type Output = Expr;

    fn pow(mut self, exp: Expr) -> Self::Output {
        let base = self.root().unwrap();
        let exp = self.import(&exp, exp.root().unwrap());
        self.push_root(Operand::Operation(Operation::Pow { base, exp }));
        self
    }
}

/* -------------------------------- FUNCTIONS ------------------------------- */

macro_rules! impl_unary_fn {
    ($fn:ident, $variant:ident, $name:literal) => {
        pub fn $fn(x: impl Into<Expr>) -> Expr {
            let mut expr = x.into();

            expr.push_root(Operand::Operation(Operation::$variant(
                expr.root().unwrap(),
            )));
            expr
        }
    };

    ($fn:ident, $variant:ident, $name:literal, square) => {
        pub fn $fn(x: impl Into<Expr>) -> Expr {
            let mut expr = x.into();
            let shape = expr.shape();

            assert!(
                shape.is_square_mat() || shape.is_scalar(),
                "Matrix-valued {} is only defined for square matrices",
                $name
            );

            expr.push_root(Operand::Operation(Operation::$variant(
                expr.root().unwrap(),
            )));
            expr
        }
    };
}

impl_unary_fn!(sin, Sin, "sine", square);
impl_unary_fn!(cos, Cos, "cosine", square);
impl_unary_fn!(tan, Tan, "tangent", square);

impl_unary_fn!(asin, Asin, "inverse sine", square);
impl_unary_fn!(acos, Acos, "inverse cosine", square);
impl_unary_fn!(atan, Atan, "inverse tangent", square);

impl_unary_fn!(sinh, Sinh, "hyperbolic sine", square);
impl_unary_fn!(cosh, Cosh, "hyperbolic cosine", square);
impl_unary_fn!(tanh, Tanh, "hyperbolic tangent", square);

impl_unary_fn!(asinh, Asinh, "inverse hyperbolic sine", square);
impl_unary_fn!(acosh, Acosh, "inverse hyperbolic cosine", square);
impl_unary_fn!(atanh, Atanh, "inverse hyperbolic tangent", square);

impl_unary_fn!(real, Real, "real component of z");
impl_unary_fn!(imag, Imag, "imaginary component of z");
impl_unary_fn!(
    sign,
    Sign,
    "sign(z) = sign(real(z)) + i * sign(imag(z)), for a complex number z"
);

impl_unary_fn!(
    norm,
    Norm,
    "Norm of the given number, or Frobenius norm for matrices"
);

pub fn atan2(x: impl Into<Expr>, y: impl Into<Expr>) -> Expr {
    let mut expr = x.into();
    let y = y.into();

    assert!(
        expr.shape().is_scalar() && y.shape().is_scalar(),
        "atan2 is only defined for scalars"
    );

    let x = expr.root().unwrap();
    let y = expr.import(&y, y.root().unwrap());

    expr.push_root(Operand::Operation(Operation::Atan2 { x, y }));
    expr
}

pub fn log(base: impl Into<Expr>, x: impl Into<Expr>) -> Expr {
    let base = base.into();
    let x = x.into();

    assert!(
        base.shape().is_scalar(),
        "Logarithm is only defined for scalar bases"
    );

    assert!(
        x.shape().is_square_mat() || x.shape().is_scalar(),
        "Matrix-valued logarithm is only defined for square matrices"
    );

    let mut expr = base;
    let base = expr.root().unwrap();
    let x = expr.import(&x, x.root().unwrap());

    expr.push_root(Operand::Operation(Operation::Log { base, arg: x }));

    expr
}

pub fn ln(x: impl Into<Expr>) -> Expr {
    log(e, x)
}

pub fn exp(x: impl Into<Expr>) -> Expr {
    e.pow(x.into())
}

pub fn sqrt(x: impl Into<Expr>) -> Expr {
    x.into().pow(1 / 2)
}

pub fn cbrt(x: impl Into<Expr>) -> Expr {
    x.into().pow(1 / 3)
}

pub fn qtrt(x: impl Into<Expr>) -> Expr {
    x.into().pow(1 / 4)
}
