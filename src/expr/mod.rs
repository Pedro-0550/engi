// /* --------------------------------- MODULES -------------------------------- */
pub mod domain;
pub mod fmt;
pub mod jit;
pub mod normal;
pub mod ops;
pub mod shape;
pub mod tree;

use std::{
    cell::OnceCell,
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    iter::empty,
    mem,
    ops::{Add, Div, Mul, Neg, Sub},
    slice,
};

use derive_more::IsVariant;
use itertools::Itertools;
use kinded::Kinded;
use num::complex::Complex64;
use ordered_float::Pow;
use xxhash_rust::xxh3::{Xxh3, Xxh3Builder};

use crate::{
    core::value::Value,
    expr::{
        shape::Shape,
        tree::{Branch, Leaf, Node, NodeKind},
    },
    model::Variable,
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
    cons: HashMap<NodeKey, NodeId>,
    root: NodeId,
}

/* ---------------------------------- IMPLS --------------------------------- */

impl Expr {
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    pub fn node(&self, id: NodeId) -> &ExprNode {
        &self.nodes[id.0].1
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
            .collect::<HashMap<_, _>>();

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

    pub fn reroot(&mut self, new_root: ExprNode) -> NodeId {
        let root = self.push(new_root);
        self.root = root;
        root
    }

    pub fn set_root(&mut self, new_root: NodeId) {
        self.root = new_root;
    }

    fn pre_dfs(&self) -> Vec<NodeId> {
        let mut visit = Vec::with_capacity(self.nodes.len());

        fn pre_dfs_inner<'e>(
            expr: &'e Expr,
            current: NodeId,
            into: &mut Vec<NodeId>,
        ) {
            let node = &expr.nodes[current.0].1;
            into.push(current);
            for child in node.children() {
                pre_dfs_inner(expr, *child, into);
            }
        }

        pre_dfs_inner(self, self.root, &mut visit);
        visit
    }

    pub fn post_dfs(&self) -> Vec<NodeId> {
        let mut visit = Vec::with_capacity(self.nodes.len());

        fn post_dfs_inner<'e>(
            expr: &'e Expr,
            current: NodeId,
            into: &mut Vec<NodeId>,
        ) {
            let node = &expr.nodes[current.0].1;
            for child in node.children() {
                post_dfs_inner(expr, *child, into);
            }
            into.push(current);
        }

        post_dfs_inner(self, self.root, &mut visit);
        visit
    }

    pub fn fold_dfs<T: Clone>(
        &self,
        mut f: impl FnMut(NodeId, &Node<T>) -> T,
    ) -> T {
        let mut mapped = HashMap::<NodeId, T>::with_capacity(self.nodes.len());

        for (id) in self.post_dfs() {
            let node = self.node(id);
            if mapped.contains_key(&id) {
                continue;
            }

            let mapped_node = match node {
                Node::Leaf(leaf) => Node::Leaf(leaf.clone()),
                Node::Branch(_) => {
                    node.clone().map(|child_id| mapped[&child_id].clone())
                }
            };

            let result = f(id, &mapped_node);
            mapped.insert(id, result);
        }

        mapped.remove(&self.root).unwrap()
    }

    /// Imports an entire node's subtree from another expr, mapping IDs appropriately
    pub fn import(&mut self, src: &Expr, src_id: NodeId) -> NodeId {
        let mut cache = HashMap::new();

        fn import_inner(
            expr: &mut Expr,
            src: &Expr,
            src_id: NodeId,
            mapped: &mut HashMap<NodeId, NodeId>,
        ) -> NodeId {
            if let Some(&remapped) = mapped.get(&src_id) {
                return remapped;
            }

            let remapped_node = src
                .node(src_id)
                .clone()
                .map(|child| import_inner(expr, src, child, mapped));

            let new_id = expr.push(remapped_node);
            mapped.insert(src_id, new_id);
            new_id
        }

        import_inner(self, src, src_id, &mut cache)
    }

    pub fn normalize(&mut self) {
        for id in self.post_dfs() {
            let _ = try {
                let branch = self.node(id).as_branch()?;
                let [a, b] = branch.as_binary()?;

                if self.key(*b).0 > self.key(*a).0
                    && (!branch.is_mul()
                        || (self.shape_of(*a).is_scalar()
                            && self.shape_of(*b).is_scalar()))
                {
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
        Self { cons: HashMap::new(), nodes: Vec::new(), root: NodeId(0) }
    }

    pub fn symbols(&self) -> impl Iterator<Item = Symbol> {
        self.nodes.iter().filter_map(|x| x.1.as_leaf()?.as_symbol().copied())
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
        self.reroot(Node::Branch(Branch::Mul([a_id, b_id])));
        self
    }
}

impl Add<Expr> for Expr {
    type Output = Expr;

    fn add(mut self, rhs: Expr) -> Self::Output {
        let a_id = self.root();
        let b_id = self.append(rhs);
        self.reroot(Node::Branch(Branch::Add([a_id, b_id])));
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
        self.reroot(Node::Branch(Branch::Pow { base, exp }));
        self
    }
}

/* -------------------------------- FUNCTIONS ------------------------------- */

macro_rules! impl_unary_fn {
    ($fn:ident, $variant:ident, $name:literal) => {
        pub fn $fn(x: impl Into<Expr>) -> Expr {
            let mut expr = x.into();

            expr.reroot(Node::Branch(Branch::$variant(expr.root())));
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

            expr.reroot(Node::Branch(Branch::$variant(expr.root())));
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

    expr.reroot(Node::Branch(Branch::Atan2 { a, b }));
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

    expr.reroot(Node::Branch(Branch::Log { base, arg: x }));

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
