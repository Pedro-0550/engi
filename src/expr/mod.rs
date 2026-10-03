// /* --------------------------------- MODULES -------------------------------- */
pub mod domain;
pub mod fmt;
pub mod jit;
pub mod ops;
pub mod shape;
pub mod tree;

use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    iter::empty,
    mem,
    ops::{Add, Div, Mul, Neg, Sub},
    slice,
};

use ahash::AHashMap;
use derive_more::IsVariant;
use faer::linalg::svd::ComputeSvdVectors::No;
use itertools::Itertools;
use kinded::Kinded;
use num::complex::{Complex64, c64};
use ordered_float::Pow;
use xxhash_rust::xxh3::{Xxh3, Xxh3Builder};

use crate::{
    core::value::Value,
    expr::{
        shape::Shape,
        tree::{Branch, BranchKind, Leaf, Node, NodeKind},
    },
    model::Variable,
    simplify::{self, EquivalencyGraph, rules},
    symbol::{
        Symbol,
        constants::{Constant, e},
    },
    units::{Quantity, Unit::Unitless},
};

type ExprNode = Node<NodeId>;

#[derive(PartialEq, Clone, Eq, Hash, Copy)]
pub struct NodeId(usize);

#[derive(PartialEq, Clone, Eq, Hash, Copy)]
pub struct NodeKey(u128);

#[derive(PartialEq, Clone, Eq, Hash, Copy)]
pub struct ExprKey(u128);

#[derive(Eq, Clone)]
pub struct Expr {
    nodes: Vec<(NodeKey, ExprNode)>,
    cons: AHashMap<NodeKey, NodeId>,
    root: NodeId,
}

pub struct EditContext<'e> {
    expr: Cell<Option<&'e mut Expr>>,
}

#[derive(Eq, Clone, Copy, PartialEq)]
enum Order {
    Post,
    In,
    Pre,
}

/* ---------------------------------- IMPLS --------------------------------- */

impl Expr {
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    pub fn edit(&mut self) -> EditContext<'_> {
        EditContext { expr: Cell::new(Some(self)) }
    }

    pub fn node(&self, id: NodeId) -> &ExprNode {
        &self.nodes[id.0].1
    }

    pub fn as_single(&self) -> Option<&ExprNode> {
        if self.len() == 1 { Some(&self.nodes[0].1) } else { None }
    }

    pub fn key(&self, id: NodeId) -> NodeKey {
        self.nodes[id.0].0
    }

    pub fn node_mut(&mut self, id: NodeId) -> &mut ExprNode {
        &mut self.nodes[id.0].1
    }

    fn key_of(&self, node: &ExprNode) -> NodeKey {
        let mut hasher = Xxh3Builder::new().with_seed(0).build();
        match node {
            Node::Leaf(leaf) => {
                leaf.hash(&mut hasher);
            }
            Node::Branch(branch) => {
                branch.kind().hash(&mut hasher);

                match branch {
                    Branch::Matrix(matrix) => {
                        matrix.shape().hash(&mut hasher);
                    }
                    Branch::Conditional { cond, .. } => {
                        cond.hash_structure(&mut hasher);
                    }
                    _ => {}
                }
            }
        }

        for child_id in node.children() {
            let (child_key, _) = self.nodes[child_id.0];
            child_key.hash(&mut hasher);
        }

        NodeKey(hasher.digest128())
    }

    pub fn push(&mut self, node: ExprNode) -> NodeId {
        self.push_with_key(self.key_of(&node), node)
    }

    fn push_with_key(&mut self, key: NodeKey, node: ExprNode) -> NodeId {
        if let Some(&id) = self.cons.get(&key) {
            return id;
        }

        let id = NodeId(self.nodes.len());

        self.nodes.push((key, node));
        self.cons.insert(key, id);

        id
    }

    pub fn substitute(&mut self, bindings: &[(Symbol, Expr)]) {
        let symbol_to_expr = bindings
            .iter()
            .filter_map(|(symbol, expr)| {
                let node_key = self.key_of(&Node::Leaf(Leaf::Symbol(*symbol)));
                let symbol_node_id = *self.cons.get(&node_key)?;
                let expr_node_id = self.append(expr.clone());

                Some((symbol_node_id, expr_node_id))
            })
            .collect::<AHashMap<_, _>>();

        for (_, node) in &mut self.nodes {
            for child_id in node.children_mut() {
                if let Some(new_id) = symbol_to_expr.get(&*child_id) {
                    *child_id = *new_id;
                }
            }
        }
    }

    /// Appends the given expr to the current expr, moving all of its nodes, changing IDs.
    /// Returns the new id of the other expr's root.
    fn append(&mut self, other: Expr) -> NodeId {
        let mut remapped = vec![None; other.nodes.len()];
        let mut other_nodes = other.nodes.into_iter().map(Some).collect_vec();
        let mut stack = vec![(other.root, false)];

        while let Some((id, processed)) = stack.pop() {
            if remapped[id.0].is_some() {
                continue;
            }

            if processed {
                let (key, mut node) = other_nodes[id.0].take().unwrap();

                for child in node.children_mut() {
                    *child = remapped[child.0].unwrap();
                }

                let new_id = self.push_with_key(key, node);
                remapped[id.0] = Some(new_id);
            } else {
                stack.push((id, true));

                if let Some((_, node)) = &other_nodes[id.0] {
                    for child in node.children() {
                        if remapped[child.0].is_none() {
                            stack.push((*child, false));
                        }
                    }
                }
            }
        }

        remapped[other.root.0].unwrap()
    }

    pub fn push_root(&mut self, new_root: ExprNode) -> NodeId {
        let root = self.push(new_root);
        self.root = root;
        root
    }

    pub fn set_root(&mut self, new_root: NodeId) {
        self.root = new_root;
    }

    fn dfs(&self, order: Order) -> Vec<NodeId> {
        let mut visit = Vec::with_capacity(self.nodes.len());

        fn dfs_inner<'e>(
            expr: &'e Expr,
            current: NodeId,
            into: &mut Vec<NodeId>,
            order: &Order,
        ) {
            let node = &expr.nodes[current.0].1;

            if *order == Order::Pre {
                into.push(current);
            }

            if *order == Order::In {
                let mut children = node.children().into_iter();
                match (children.next(), children.next()) {
                    (Some(left), Some(right)) => {
                        dfs_inner(expr, *left, into, order);
                        into.push(current);
                        dfs_inner(expr, *right, into, order);
                    }
                    (Some(child), None) => {
                        into.push(current);
                        dfs_inner(expr, *child, into, order);
                    }
                    (None, _) => {
                        into.push(current);
                    }
                }
            } else {
                // Pre and Post rely on standard order
                for child in node.children() {
                    dfs_inner(expr, *child, into, order);
                }
            }

            if *order == Order::Post {
                into.push(current);
            }
        }

        dfs_inner(self, self.root, &mut visit, &order);
        visit
    }

    pub fn fold_dfs<T: Clone>(
        &self,
        mut f: impl FnMut(NodeId, &Node<NodeId>, Node<T>) -> T,
    ) -> T {
        let mut mapped = AHashMap::<NodeId, T>::with_capacity(self.nodes.len());

        for id in self.dfs(Order::Post) {
            if mapped.contains_key(&id) {
                continue;
            }

            let node = self.node(id);

            let mapped_node = match node {
                Node::Leaf(leaf) => Node::Leaf(leaf.clone()),
                Node::Branch(_) => node.clone().map(&mut |child_id: NodeId| {
                    mapped.get(&child_id).unwrap().clone()
                }),
            };

            let result = f(id, node, mapped_node);
            mapped.insert(id, result);
        }

        mapped.remove(&self.root).expect("root node should be folded")
    }

    /// Imports an entire node's subtree from another expr, mapping IDs appropriately
    pub fn import(&mut self, src: &Expr, src_id: NodeId) -> NodeId {
        let mut cache = AHashMap::new();

        fn import_inner(
            expr: &mut Expr,
            src: &Expr,
            src_id: NodeId,
            mapped: &mut AHashMap<NodeId, NodeId>,
        ) -> NodeId {
            if let Some(&remapped) = mapped.get(&src_id) {
                return remapped;
            }

            let remapped_node =
                src.node(src_id).clone().map(&mut |child: NodeId| {
                    import_inner(expr, src, child, mapped)
                });

            let new_id = expr.push(remapped_node);
            mapped.insert(src_id, new_id);
            new_id
        }

        import_inner(self, src, src_id, &mut cache)
    }

    /// Folds values in branches by evaluating them as much as possible
    pub fn folded(&self) -> Self {
        #[derive(Clone)]
        struct Accumulated {
            value: Option<Value>,
            symbolic: Vec<NodeId>,
        }

        fn identity(kind: BranchKind) -> Value {
            match kind {
                BranchKind::Add => Value::ZERO,
                BranchKind::Mul => Value::ONE,
                BranchKind::Max => {
                    c64(f64::NEG_INFINITY, f64::NEG_INFINITY).into()
                }
                BranchKind::Min => c64(f64::INFINITY, f64::INFINITY).into(),
                _ => unreachable!(),
            }
        }

        impl Accumulated {
            fn into_value(self) -> Option<Value> {
                if self.symbolic.is_empty() { self.value } else { None }
            }

            fn value(v: Value) -> Self {
                Self { symbolic: vec![], value: Some(v) }
            }

            fn symbolic(node: NodeId) -> Self {
                Self { symbolic: vec![node], value: None }
            }

            fn push_into(self, op: BranchKind, into: &mut Expr) -> NodeId {
                let mut iter = self.symbolic.iter().copied();
                let init = self
                    .value
                    .map(|x| into.push(x.into()))
                    .or_else(|| iter.next())
                    .unwrap();

                iter.fold(init, |acc, v| match op {
                    BranchKind::Add => into.edit().add(acc, v),
                    BranchKind::Mul => into.edit().mul(acc, v),
                    BranchKind::Min => into.edit().min(acc, v),
                    BranchKind::Max => into.edit().max(acc, v),
                    _ => unreachable!(),
                })
            }

            fn fold_binary(
                a: Accumulated,
                b: Accumulated,
                kind: BranchKind,
            ) -> Self {
                Accumulated {
                    value: {
                        let a = a.value;
                        let b = b.value;

                        if a.is_none() && b.is_none() {
                            None
                        } else {
                            Some(a.iter().chain(b.iter()).fold(
                                identity(kind),
                                |acc, v| match kind {
                                    BranchKind::Add => acc + v,
                                    BranchKind::Mul => acc * v,
                                    BranchKind::Max | BranchKind::Min => {
                                        let acc = acc.as_scalar().unwrap();
                                        let v = v.as_scalar().unwrap();

                                        if kind == BranchKind::Max {
                                            Complex64 {
                                                re: acc.re.max(v.re),
                                                im: acc.im.max(v.im),
                                            }
                                        } else {
                                            Complex64 {
                                                re: acc.re.min(v.re),
                                                im: acc.im.min(v.im),
                                            }
                                        }
                                        .into()
                                    }
                                    _ => unreachable!(),
                                },
                            ))
                        }
                    },
                    symbolic: [a.symbolic, b.symbolic].concat(),
                }
            }
        }

        let mut new_expr = Expr::new();

        let acc =
            self.fold_dfs(
                &mut |id, old: &Node<NodeId>, new: Node<Accumulated>| {
                    match new {
                        Node::Leaf(leaf) => match leaf {
                            Leaf::Quantity(quantity) => {
                                Accumulated::value(quantity.into_value())
                            }
                            _ => {
                                let new_id = new_expr.push(old.clone());
                                Accumulated::symbolic(new_id)
                            }
                        },
                        Node::Branch(branch) => {
                            let branch_kind = branch.kind();

                            match branch {
                                Branch::Add(children)
                                | Branch::Mul(children)
                                | Branch::Max(children)
                                | Branch::Min(children) => {
                                    let this_node = self.node(id);
                                    let [mut a_acc, mut b_acc] = children;
                                    let [a_id, b_id] = *old.children() else {
                                        unreachable!()
                                    };

                                    // There is no order for complex numbers (although we still allow them elementwise), matrices, and sets
                                    if this_node.as_branch().is_some_and(|b| {
                                        b.is_max() || b.is_min()
                                    }) && !(a_acc
                                        .value
                                        .as_ref()
                                        .is_none_or(|v| v.is_scalar())
                                        && b_acc
                                            .value
                                            .as_ref()
                                            .is_none_or(|v| v.is_scalar()))
                                    {
                                        let new_id = new_expr.push(old.clone());
                                        return Accumulated::symbolic(new_id);
                                    }

                                    let a_node = self.node(*a_id);
                                    let b_node = self.node(*b_id);

                                    let foldable_a = a_node.kind()
                                        == this_node.kind()
                                        || a_node.is_leaf();
                                    let foldable_b = b_node.kind()
                                        == this_node.kind()
                                        || b_node.is_leaf();

                                    if foldable_a && foldable_b {
                                        Accumulated::fold_binary(
                                            a_acc,
                                            b_acc,
                                            branch_kind,
                                        )
                                    } else if foldable_a {
                                        let b_kind =
                                            b_node.as_branch().unwrap().kind();
                                        let new_b_id = b_acc
                                            .push_into(b_kind, &mut new_expr);

                                        a_acc.symbolic.push(new_b_id);
                                        Accumulated::symbolic(a_acc.push_into(
                                            branch_kind,
                                            &mut new_expr,
                                        ))
                                    } else if foldable_b {
                                        let a_kind =
                                            a_node.as_branch().unwrap().kind();
                                        let new_a_id = a_acc
                                            .push_into(a_kind, &mut new_expr);

                                        b_acc.symbolic.push(new_a_id);
                                        Accumulated::symbolic(b_acc.push_into(
                                            branch_kind,
                                            &mut new_expr,
                                        ))
                                    } else {
                                        let a_kind =
                                            a_node.as_branch().unwrap().kind();
                                        let b_kind =
                                            b_node.as_branch().unwrap().kind();

                                        let new_a_id = a_acc
                                            .push_into(a_kind, &mut new_expr);
                                        let new_b_id = b_acc
                                            .push_into(b_kind, &mut new_expr);

                                        let curr_acc = Accumulated {
                                            value: None,
                                            symbolic: vec![new_a_id, new_b_id],
                                        };

                                        Accumulated::symbolic(
                                            curr_acc.push_into(
                                                branch_kind,
                                                &mut new_expr,
                                            ),
                                        )
                                    }
                                }
                                Branch::Pow { base, exp } => {
                                    if let Some(base) = base.into_value()
                                        && let Some(exp) = exp.into_value()
                                    {
                                        Accumulated::value(base.pow(exp))
                                    } else {
                                        Accumulated::symbolic(
                                            new_expr.import(self, id),
                                        )
                                    }
                                }
                                Branch::Log { base, arg } => {
                                    if let Some(base) = base.into_value()
                                        && let Some(arg) = arg.into_value()
                                    {
                                        Accumulated::value(arg.ln() / base.ln())
                                    } else {
                                        Accumulated::symbolic(
                                            new_expr.import(self, id),
                                        )
                                    }
                                }

                                _ => Accumulated::symbolic(
                                    new_expr.import(self, id),
                                ),
                            }
                        }
                    }
                },
            );

        let root_node = self.node(self.root());
        let root_id = match root_node {
            Node::Branch(b)
                if matches!(
                    b.kind(),
                    BranchKind::Add
                        | BranchKind::Mul
                        | BranchKind::Min
                        | BranchKind::Max
                ) =>
            {
                acc.push_into(b.kind(), &mut new_expr)
            }
            _ => {
                if let Some(v) = acc.value {
                    new_expr.push(v.into())
                } else {
                    assert_eq!(
                        acc.symbolic.len(),
                        1,
                        "Expected exactly 1 symbolic root node"
                    );
                    acc.symbolic[0]
                }
            }
        };

        new_expr.set_root(root_id);
        new_expr
    }

    pub fn clean(&mut self) {}

    pub fn normalize(&mut self) {
        for id in self.dfs(Order::Post) {
            let _ = try {
                let branch = self.node(id).as_branch()?;
                let [a, b] = branch.as_binary()?;

                // Multiplicative commutativity rule
                if branch.is_mul()
                    && !self.shape_of(*a).is_scalar()
                    && !self.shape_of(*b).is_scalar()
                {
                    continue;
                }

                if self.key(*b).0 > self.key(*a).0 {
                    let [a, b] =
                        self.node_mut(id).as_branch_mut()?.as_binary_mut()?;

                    mem::swap(a, b);
                    let new_key = self.key_of(self.node(id));
                    self.cons.insert(new_key, id);
                    // We keep the old key because why not, its the same thing anyway
                }
            };
        }
    }

    pub fn new() -> Self {
        Self { cons: AHashMap::new(), nodes: Vec::new(), root: NodeId(0) }
    }

    pub fn symbols(&self) -> impl Iterator<Item = Symbol> {
        self.nodes.iter().filter_map(|x| x.1.as_leaf()?.as_symbol().copied())
    }

    pub fn simplified(&self) -> Expr {
        let mut egraph = EquivalencyGraph::build(&*self);
        egraph.rewrite(&[rules::algebraic(), rules::trig()].concat());
        egraph.extract().folded()
    }
}

impl PartialEq for Expr {
    fn eq(&self, other: &Self) -> bool {
        self.nodes[self.root.0].0 == other.nodes[other.root.0].0
    }
}

impl Hash for Expr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.nodes[self.root.0].0.hash(state);
    }
}

/* -------------------------------------------------------------------------- */

impl Mul<Expr> for Expr {
    type Output = Expr;

    fn mul(mut self, rhs: Expr) -> Self::Output {
        let a_id = self.root();
        let b_id = self.append(rhs);
        self.push_root(Node::Branch(Branch::Mul([a_id, b_id])));
        self
    }
}

impl Add<Expr> for Expr {
    type Output = Expr;

    fn add(mut self, rhs: Expr) -> Self::Output {
        let a_id = self.root();
        let b_id = self.append(rhs);
        self.push_root(Node::Branch(Branch::Add([a_id, b_id])));
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
        let base = self.root();
        let exp = self.append(exp);
        self.push_root(Node::Branch(Branch::Pow { base, exp }));
        self
    }
}

/* -------------------------------- FUNCTIONS ------------------------------- */

macro_rules! impl_unary_fn {
    ($fn:ident, $variant:ident, $name:literal) => {
        pub fn $fn(x: impl Into<Expr>) -> Expr {
            let mut expr = x.into();

            expr.push_root(Node::Branch(Branch::$variant(expr.root())));
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

            expr.push_root(Node::Branch(Branch::$variant(expr.root())));
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

pub fn atan2(a: impl Into<Expr>, b: impl Into<Expr>) -> Expr {
    let mut expr = a.into();
    let b = b.into();

    assert!(
        expr.shape().is_scalar() && b.shape().is_scalar(),
        "atan2 is only defined for scalars"
    );

    let a = expr.root();
    let b = expr.append(b);

    expr.push_root(Node::Branch(Branch::Atan2 { a, b }));
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
    let base = expr.root();
    let x = expr.append(x);

    expr.push_root(Node::Branch(Branch::Log { base, arg: x }));

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
