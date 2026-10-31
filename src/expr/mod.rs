// /* --------------------------------- MODULES -------------------------------- */
pub mod dag;
pub mod domain;
pub mod fmt;
pub mod jit;
pub mod ops;
pub mod shape;

use std::{
    cell::Cell,
    hash::{BuildHasher, Hash, Hasher},
    mem::variant_count,
};

use ahash::AHashMap;
use derive_more::IsVariant;
use itertools::Itertools;
use kinded::Kinded;
use ordered_float::Pow;

use crate::{
    expr::dag::{
            Condition, ConditionId, Dag, Leaf, LeafId, LeafKind, Matrix,
            MatrixId, Operation, OperationId, OperationKind,
        },
    simplify::{EquivalencyGraph, rules},
    symbol::{
        Symbol,
        constants::e,
    },
};

/// A mathematical expression based on a directed acryllic graph
///
/// The W parameter is used for wildcards, its [`!`] (`never`) by default since wildcards shouldn't be present in
/// normal expressions, only e-graph patterns, which are of type `Expr<Wildcard>`.
#[derive(Clone)]
pub struct Expr<W = !>
where
    W: Eq + Hash + Copy, {
    pub(crate) dag: Dag<Operand<W>, OperationId, LeafId, MatrixId>,
}

/// An [`Operation`]'s operand. This is like a node handle in traditional DAGs
#[derive(Eq, PartialEq, Clone, Copy, Hash, IsVariant, PartialOrd, Ord)]
pub enum Operand<W = !, O = OperationId, L = LeafId, M = MatrixId> {
    Leaf(L),
    Matrix(M),
    Operation(O),
    Wildcard(W),
}

#[derive(Eq, PartialEq, Clone, Copy, Hash, IsVariant, PartialOrd, Ord)]
pub enum Node<W = !> {
    Operand(Operand<W>),
    Condition(ConditionId),
}

#[derive(Eq, PartialEq, Clone, Copy, Hash, IsVariant)]
pub enum OperandKind {
    Leaf(LeafKind),
    Operation(OperationKind),
    Matrix,
    Wildcard,
}

pub type FetchedOperand<'a, W = !> = Operand<
    W,
    &'a Operation<Operand<W>, ConditionId>,
    &'a Leaf,
    &'a Matrix<Operand<W>>,
>;

pub type BuiltOperand<W = !> =
    Operand<W, Operation<Operand<W>, ConditionId>, Leaf, Matrix<Operand<W>>>;

pub type FoldedOperand<'a, O, C, W = !> =
    Operand<W, Operation<&'a O, &'a C>, &'a Leaf, Matrix<&'a O>>;

pub type FoldedCondition<'a, O, C> = Condition<&'a O, &'a C>;

pub struct EditContext<'e, W = !>
where
    W: Eq + Hash + Copy, {
    expr: Cell<Option<&'e mut Expr<W>>>,
}

#[derive(Eq, Clone, Copy, PartialEq)]
pub enum Order {
    Post,
    Pre,
}

/* ---------------------------------- IMPLS --------------------------------- */

impl<W: Hash + Eq + Copy> Default for Expr<W> {
    fn default() -> Self {
        Self { dag: Dag::default() }
    }
}

impl OperandKind {
    pub fn id(&self) -> usize {
        match self {
            OperandKind::Leaf(leaf_kind) => *leaf_kind as usize,
            OperandKind::Operation(operation_kind) => {
                *operation_kind as usize + variant_count::<LeafKind>()
            }
            OperandKind::Matrix => {
                variant_count::<OperationKind>() + variant_count::<LeafKind>()
            }
            OperandKind::Wildcard => {
                1 + variant_count::<OperationKind>()
                    + variant_count::<LeafKind>()
            }
        }
    }
}

impl<W> FetchedOperand<'_, W>
where
    W: Copy,
{
    // pub fn iter_children(&self) -> slice::Iter<'_, Operand> {
    //     match self {
    //         Operand::Leaf(_) => [].iter(),
    //         Operand::Matrix(m) => m.elements().iter(),
    //         Operand::Operation(op) => op.,
    //     }
    // }

    pub fn for_each_operand(&self, f: impl FnMut(&Operand<W>)) {
        match self {
            Operand::Matrix(m) => m.elements().iter().for_each(f),
            Operand::Operation(op) => op.for_each_operand(f),
            _ => (),
        }
    }

    pub fn operand(&self, at: usize) -> Option<Operand<W>> {
        match self {
            Operand::Matrix(m) => m.elements().get(at).copied(),
            Operand::Operation(op) => op.operand(at as u8).copied(),
            _ => None,
        }
    }

    pub fn condition(&self) -> Option<&ConditionId> {
        match self {
            Operand::Operation(op) => op.condition(),
            _ => None,
        }
    }
}

impl<W, O, C, M> Operand<W, Operation<O, C>, Leaf, M> {
    pub fn kind(&self) -> OperandKind {
        match self {
            Operand::Leaf(l) => OperandKind::Leaf(l.kind()),
            Operand::Matrix(_m) => OperandKind::Matrix,
            Operand::Operation(op) => OperandKind::Operation(op.kind()),
            Operand::Wildcard(_) => OperandKind::Wildcard,
        }
    }
}

impl<W, O, C, M> Operand<W, &Operation<O, C>, &Leaf, M> {
    pub fn kind(&self) -> OperandKind {
        match self {
            Operand::Leaf(l) => OperandKind::Leaf(l.kind()),
            Operand::Matrix(_m) => OperandKind::Matrix,
            Operand::Operation(op) => OperandKind::Operation(op.kind()),
            Operand::Wildcard(_) => OperandKind::Wildcard,
        }
    }
}

impl<W, O, L, M> Operand<W, O, L, M> {
    pub fn as_wildcard(&self) -> Option<&W> {
        if let Self::Wildcard(v) = self { Some(v) } else { None }
    }

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

impl<W> Expr<W>
where
    W: Hash + Eq + Clone + Copy,
{
    #[inline(always)]
    pub fn fetch(&self, op: Operand<W>) -> FetchedOperand<'_, W> {
        match op {
            Operand::Leaf(id) => Operand::Leaf(&self.dag.leaves[id]),
            Operand::Matrix(id) => Operand::Matrix(&self.dag.matrices[id]),
            Operand::Operation(id) => {
                Operand::Operation(&self.dag.operations[id])
            }
            Operand::Wildcard(w) => Operand::Wildcard(w),
        }
    }

    pub fn size(&self) -> usize {
        self.dag.operations.len()
            + self.dag.conditions.len()
            + self.dag.leaves.len()
            + self.dag.matrices.len()
    }

    pub fn root(&self) -> Option<Operand<W>> {
        self.dag.root
    }

    pub fn edit(&mut self) -> EditContext<'_, W> {
        EditContext { expr: Cell::new(Some(self)) }
    }

    pub fn as_single(&self) -> Option<FetchedOperand<'_, W>> {
        if self.size() == 1 {
            self.dag.root.map(|op| self.fetch(op))
        } else {
            None
        }
    }

    /// Substitutes every occurance of the given symbols with their bound expressions.
    /// Returns Some if at least one substitution was made, None otherwise.
    pub fn substituted(
        &self,
        bindings: &AHashMap<Symbol, Expr<W>>,
    ) -> Option<Expr<W>> {
        if bindings.is_empty() {
            return None;
        }

        if !self.dag.leaves.iter().any(|(_, leaf)| {
            leaf.as_symbol().map(|s| bindings.contains_key(s)).unwrap_or(false)
        }) {
            return None;
        }

        struct RebuildContext<'a, W: Copy + Eq + Hash> {
            mapped_conds: AHashMap<ConditionId, ConditionId>,
            mapped_ops: AHashMap<Operand<W>, Operand<W>>,
            bindings: &'a AHashMap<Symbol, Expr<W>>,
            out: Expr<W>,
            src: &'a Expr<W>,
        }

        fn rebuild_condition<W: Copy + Eq + Hash>(
            cond: ConditionId,
            ctx: &mut RebuildContext<'_, W>,
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

        fn rebuild_operand<W: Copy + Eq + Hash>(
            op: Operand<W>,
            ctx: &mut RebuildContext<'_, W>,
        ) -> Operand<W> {
            if let Some(&existing) = ctx.mapped_ops.get(&op) {
                return existing;
            }

            let new_op = match op {
                Operand::Leaf(id) => {
                    if let Leaf::Symbol(s) = ctx.src.dag.leaves[id]
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
                        Operand::Leaf(
                            ctx.out
                                .dag
                                .leaves
                                .insert(ctx.src.dag.leaves[id].clone()),
                        )
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

                Operand::Wildcard(w) => Operand::Wildcard(w),
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

        rebuild_operand(
            self.root().expect("Cannot substitute for a rootless expression"),
            &mut ctx,
        );

        Some(ctx.out)
    }

    pub fn push(&mut self, operand: BuiltOperand<W>) -> Operand<W> {
        let op = match operand {
            Operand::Leaf(l) => Operand::Leaf(self.dag.leaves.insert(l)),
            Operand::Matrix(m) => Operand::Matrix(self.dag.matrices.insert(m)),
            Operand::Operation(op) => {
                Operand::Operation(self.dag.operations.insert(op))
            }
            Operand::Wildcard(w) => Operand::Wildcard(w),
        };
        op
    }

    pub fn push_root(&mut self, new_root: BuiltOperand<W>) -> Operand<W> {
        let op = self.push(new_root);
        self.dag.root = Some(op);
        op
    }

    pub fn set_root(&mut self, new_root: Operand<W>) {
        self.dag.root = Some(new_root);
    }

    pub fn dfs(&self, order: Order) -> Vec<Node<W>> {
        let mut visit = Vec::with_capacity(self.size());

        fn dfs_inner<'e, W: Hash + Eq + Copy>(
            expr: &'e Expr<W>,
            current: Node<W>,
            into: &mut Vec<Node<W>>,
            order: &Order,
        ) {
            if *order == Order::Pre {
                into.push(current);
            }

            match current {
                Node::Operand(operand) => {
                    let fetched = expr.fetch(operand);

                    fetched.for_each_operand(|op| {
                        dfs_inner(expr, Node::Operand(*op), into, order);
                    });

                    if let Some(cond) = fetched.condition() {
                        dfs_inner(expr, Node::Condition(*cond), into, order);
                    }
                }
                Node::Condition(condition) => {
                    let fetched = expr.dag.conditions[condition];

                    fetched.for_each_operand(|op| {
                        dfs_inner(expr, Node::Operand(*op), into, order);
                    });

                    fetched.for_each_condition(|cond| {
                        dfs_inner(expr, Node::Condition(*cond), into, order);
                    });
                }
            }

            if *order == Order::Post {
                into.push(current);
            }
        }

        dfs_inner(
            self,
            Node::Operand(
                self.root().expect("Cannot DFS-traverse a rootless expression"),
            ),
            &mut visit,
            &order,
        );
        visit
    }

    /// Imports an entire operand's subtree from another Expr, mapping Ids appropriately
    pub fn import(&mut self, src: &Expr<W>, src_id: Operand<W>) -> Operand<W> {
        struct ImportContext<'a, W: Copy + Hash + Eq> {
            dest: &'a mut Expr<W>,
            src: &'a Expr<W>,
            mapped_ops: AHashMap<Operand<W>, Operand<W>>,
            mapped_conds: AHashMap<ConditionId, ConditionId>,
        }

        fn import_condition<W: Copy + Hash + Eq>(
            src_id: ConditionId,
            ctx: &mut ImportContext<'_, W>,
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

        fn import_operand<W: Copy + Hash + Eq>(
            src_op: Operand<W>,
            ctx: &mut ImportContext<'_, W>,
        ) -> Operand<W> {
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
                Operand::Wildcard(w) => Operand::Wildcard(w),
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
        self.dag.leaves.iter().filter_map(|(_, leaf)| leaf.as_symbol().copied())
    }

    pub fn fold<O, C, S>(
        &self,
        s: &mut S,
        o: impl FnMut(&mut S, Operand<W>, FoldedOperand<'_, O, C, W>) -> O,
        c: impl FnMut(&mut S, ConditionId, FoldedCondition<'_, O, C>) -> C,
    ) -> O {
        self.fold_from(
            self.root().expect("Cannot fold root of a rootless expression"),
            s,
            o,
            c,
        )
    }

    pub fn fold_from<O, C, S>(
        &self,
        root: Operand<W>,
        s: &mut S,
        mut o: impl FnMut(&mut S, Operand<W>, FoldedOperand<'_, O, C, W>) -> O,
        mut c: impl FnMut(&mut S, ConditionId, FoldedCondition<'_, O, C>) -> C,
    ) -> O {
        let mut mapped_ops =
            AHashMap::<Operand<W>, O>::with_capacity(self.size());
        let mut mapped_conds = AHashMap::<ConditionId, C>::with_capacity(
            self.dag.conditions.len(),
        );

        for node in self.dfs(Order::Post) {
            match node {
                Node::Operand(op) => {
                    if mapped_ops.contains_key(&op) {
                        continue;
                    }

                    let mapped = match self.fetch(op) {
                        Operand::Matrix(m) => {
                            Operand::Matrix(m.map(|el| &mapped_ops[el]))
                        }
                        Operand::Operation(op) => Operand::Operation(
                            op.map(|o| &mapped_ops[o], |c| &mapped_conds[c]),
                        ),
                        Operand::Leaf(l) => Operand::Leaf(l),
                        Operand::Wildcard(w) => Operand::Wildcard(w),
                    };

                    mapped_ops.insert(op, o(s, op, mapped));
                }
                Node::Condition(cond) => {
                    if mapped_conds.contains_key(&cond) {
                        continue;
                    }

                    let mapped = self.dag.conditions[cond]
                        .map(|o| &mapped_ops[o], |c| &mapped_conds[c]);

                    mapped_conds.insert(cond, c(s, cond, mapped));
                }
            }
        }

        mapped_ops.remove(&root).expect("root node should be folded")
    }
}

impl PartialEq for Expr {
    fn eq(&self, other: &Self) -> bool {
        let rs = ahash::RandomState::new();
        let mut a = rs.build_hasher();
        let mut b = rs.build_hasher();

        self.fold(&mut a, |a, _, o| o.hash(a), |a, _, c| c.hash(a));
        other.fold(&mut b, |b, _, o| o.hash(b), |b, _, c| c.hash(b));

        a.finish() == b.finish()
    }
}

impl Expr<!> {
    pub fn simplified(&self) -> Expr {
        let mut egraph = EquivalencyGraph::new(self);
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
