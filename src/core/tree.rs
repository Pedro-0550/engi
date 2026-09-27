use std::{
    cell::OnceCell,
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    iter::empty,
};

use itertools::Itertools;
use kinded::Kinded;
use xxhash_rust::xxh3::{Xxh3, Xxh3Builder};

use crate::{
    expr::shape::Shape,
    symbol::{Symbol, constants::Constant},
    units::Quantity,
};

#[derive(Eq, PartialEq, Clone, Hash, Kinded)]
#[kinded(derive(Hash))]
pub enum Branch<N> {
    Add([N; 2]),
    Mul([N; 2]),
    Min([N; 2]),
    Max([N; 2]),

    Sin(N),
    Cos(N),
    Tan(N),

    Asin(N),
    Acos(N),
    Atan(N),

    Sinh(N),
    Cosh(N),
    Tanh(N),

    Asinh(N),
    Acosh(N),
    Atanh(N),

    Arg(N),
    Conj(N),
    Norm(N),
    Sign(N),

    Real(N),
    Imag(N),

    Pow { base: N, exp: N },
    Log { base: N, arg: N },
    Atan2 { a: N, b: N },

    Matrix(Matrix<N>),
    Transpose(N),
    Det(N),
    Rank(N),
    Trace(N),

    Conditional { cond: Condition<N>, pass: N, fail: N },
}

#[derive(Eq, PartialEq, Clone, Hash)]
pub struct Matrix<N> {
    shape: Shape,
    elements: Box<[N]>,
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub enum Condition<N> {
    Eq(N, N),
    Ne(N, N),
    Lt(N, N),
    Le(N, N),
    Gt(N, N),
    Ge(N, N),

    And(Box<[Condition<N>]>),
    Or(Box<[Condition<N>]>),
    Not(Box<Condition<N>>),
}

#[derive(PartialEq, Clone, Eq, Hash, Kinded)]
#[kinded(derive(Hash))]
pub enum Leaf {
    Symbol(Symbol),
    Constant(Constant),
    Quantity(Quantity),
}

pub enum Node<N> {
    Leaf(Leaf),
    Branch(Branch<N>),
}

#[derive(PartialEq, Clone, Eq, Hash, Copy)]
pub enum NodeKind {
    Leaf(LeafKind),
    Branch(BranchKind),
}

#[derive(PartialEq, Clone, Eq, Hash, Copy)]
pub struct NodeId(usize);

#[derive(PartialEq, Clone, Eq, Hash, Copy)]
pub struct NodeKey(u128);

#[derive(PartialEq, Clone, Eq, Hash, Copy)]
pub struct ExprKey(u128);

pub struct Expr {
    nodes: Vec<(NodeKey, Node<NodeId>)>,
    cons: HashMap<NodeKey, NodeId>,
    root: NodeId,
}

impl Expr {
    fn root(&self) -> NodeId {
        self.root
    }

    fn key_of(&self, node: &Node<NodeId>) -> NodeKey {
        let mut hasher = Xxh3Builder::new().with_seed(0).build();
        match node {
            Node::Leaf(leaf) => {
                leaf.hash(&mut hasher);
            }
            Node::Branch(branch) => {
                branch.kind().hash(&mut hasher);

                match branch {
                    Branch::Matrix(matrix) => {
                        matrix.shape.hash(&mut hasher);
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

    fn add(&mut self, node: Node<NodeId>) -> NodeId {
        self.add_with_key(self.key_of(&node), node)
    }

    fn add_with_key(&mut self, key: NodeKey, node: Node<NodeId>) -> NodeId {
        if let Some(&id) = self.cons.get(&key) {
            return id;
        }

        let id = NodeId(self.nodes.len());

        self.nodes.push((key, node));
        self.cons.insert(key, id);

        id
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

                let new_id = self.add_with_key(key, node);
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

    fn reroot(&mut self, new_root: Node<NodeId>) -> NodeId {
        let root = self.add(new_root);
        self.root = root;
        root
    }
}

impl<N> Node<N> {
    pub fn kind(&self) -> NodeKind {
        match self {
            Node::Leaf(leaf) => NodeKind::Leaf(leaf.kind()),
            Node::Branch(branch) => NodeKind::Branch(branch.kind()),
        }
    }

    pub fn children(&self) -> Box<dyn DoubleEndedIterator<Item = &N> + '_>
    where
        N: 'static, {
        match self {
            Node::Leaf(_) => Box::new(empty()),

            Node::Branch(branch) => match branch {
                Branch::Add(ns)
                | Branch::Mul(ns)
                | Branch::Min(ns)
                | Branch::Max(ns) => Box::new(ns.iter()),

                Branch::Sin(n)
                | Branch::Cos(n)
                | Branch::Tan(n)
                | Branch::Asin(n)
                | Branch::Acos(n)
                | Branch::Atan(n)
                | Branch::Sinh(n)
                | Branch::Cosh(n)
                | Branch::Tanh(n)
                | Branch::Asinh(n)
                | Branch::Acosh(n)
                | Branch::Atanh(n)
                | Branch::Arg(n)
                | Branch::Conj(n)
                | Branch::Norm(n)
                | Branch::Sign(n)
                | Branch::Real(n)
                | Branch::Imag(n)
                | Branch::Transpose(n)
                | Branch::Det(n)
                | Branch::Rank(n)
                | Branch::Trace(n) => Box::new(std::iter::once(n)),

                Branch::Pow { base, exp } | Branch::Log { base, arg: exp } => {
                    Box::new([base, exp].into_iter())
                }

                Branch::Atan2 { a, b } => Box::new([a, b].into_iter()),

                Branch::Matrix(matrix) => Box::new(matrix.elements.iter()),

                Branch::Conditional { cond, pass, fail } => Box::new(
                    cond.children()
                        .chain(std::iter::once(pass))
                        .chain(std::iter::once(fail)),
                ),
            },
        }
    }

    pub fn children_mut(
        &mut self,
    ) -> Box<dyn DoubleEndedIterator<Item = &mut N> + '_>
    where
        N: 'static, {
        match self {
            Node::Leaf(_) => Box::new(empty()),

            Node::Branch(branch) => match branch {
                Branch::Add(ns)
                | Branch::Mul(ns)
                | Branch::Min(ns)
                | Branch::Max(ns) => Box::new(ns.iter_mut()),

                Branch::Sin(n)
                | Branch::Cos(n)
                | Branch::Tan(n)
                | Branch::Asin(n)
                | Branch::Acos(n)
                | Branch::Atan(n)
                | Branch::Sinh(n)
                | Branch::Cosh(n)
                | Branch::Tanh(n)
                | Branch::Asinh(n)
                | Branch::Acosh(n)
                | Branch::Atanh(n)
                | Branch::Arg(n)
                | Branch::Conj(n)
                | Branch::Norm(n)
                | Branch::Sign(n)
                | Branch::Real(n)
                | Branch::Imag(n)
                | Branch::Transpose(n)
                | Branch::Det(n)
                | Branch::Rank(n)
                | Branch::Trace(n) => Box::new(std::iter::once(n)),

                Branch::Pow { base, exp } | Branch::Log { base, arg: exp } => {
                    Box::new([base, exp].into_iter())
                }

                Branch::Atan2 { a, b } => Box::new([a, b].into_iter()),

                Branch::Matrix(matrix) => Box::new(matrix.elements.iter_mut()),

                Branch::Conditional { cond, pass, fail } => Box::new(
                    cond.children_mut()
                        .chain(std::iter::once(pass))
                        .chain(std::iter::once(fail)),
                ),
            },
        }
    }

    pub fn into_children(self) -> Box<dyn DoubleEndedIterator<Item = N>>
    where
        N: 'static, {
        match self {
            Node::Leaf(_) => Box::new(empty()),

            Node::Branch(branch) => match branch {
                Branch::Add(ns)
                | Branch::Mul(ns)
                | Branch::Min(ns)
                | Branch::Max(ns) => Box::new(ns.into_iter()),

                Branch::Sin(n)
                | Branch::Cos(n)
                | Branch::Tan(n)
                | Branch::Asin(n)
                | Branch::Acos(n)
                | Branch::Atan(n)
                | Branch::Sinh(n)
                | Branch::Cosh(n)
                | Branch::Tanh(n)
                | Branch::Asinh(n)
                | Branch::Acosh(n)
                | Branch::Atanh(n)
                | Branch::Arg(n)
                | Branch::Conj(n)
                | Branch::Norm(n)
                | Branch::Sign(n)
                | Branch::Real(n)
                | Branch::Imag(n)
                | Branch::Transpose(n)
                | Branch::Det(n)
                | Branch::Rank(n)
                | Branch::Trace(n) => Box::new(std::iter::once(n)),

                Branch::Pow { base, exp } | Branch::Log { base, arg: exp } => {
                    Box::new([base, exp].into_iter())
                }

                Branch::Atan2 { a, b } => Box::new([a, b].into_iter()),

                Branch::Matrix(matrix) => {
                    Box::new(matrix.elements.into_vec().into_iter())
                }

                Branch::Conditional { cond, pass, fail } => Box::new(
                    cond.into_children()
                        .chain(std::iter::once(pass))
                        .chain(std::iter::once(fail)),
                ),
            },
        }
    }
}

impl<N> Condition<N> {
    pub fn children(&self) -> Box<dyn DoubleEndedIterator<Item = &N> + '_>
    where
        N: 'static, {
        match self {
            Condition::Eq(a, b)
            | Condition::Ne(a, b)
            | Condition::Lt(a, b)
            | Condition::Le(a, b)
            | Condition::Gt(a, b)
            | Condition::Ge(a, b) => Box::new([a, b].into_iter()),

            Condition::And(conditions) | Condition::Or(conditions) => {
                Box::new(conditions.iter().flat_map(|c| c.children()))
            }

            Condition::Not(condition) => condition.children(),
        }
    }

    pub fn children_mut(
        &mut self,
    ) -> Box<dyn DoubleEndedIterator<Item = &mut N> + '_>
    where
        N: 'static, {
        match self {
            Condition::Eq(a, b)
            | Condition::Ne(a, b)
            | Condition::Lt(a, b)
            | Condition::Le(a, b)
            | Condition::Gt(a, b)
            | Condition::Ge(a, b) => Box::new([a, b].into_iter()),

            Condition::And(conditions) | Condition::Or(conditions) => {
                Box::new(conditions.iter_mut().flat_map(|c| c.children_mut()))
            }

            Condition::Not(condition) => condition.children_mut(),
        }
    }

    pub fn into_children(self) -> Box<dyn DoubleEndedIterator<Item = N>>
    where
        N: 'static, {
        match self {
            Condition::Eq(a, b)
            | Condition::Ne(a, b)
            | Condition::Lt(a, b)
            | Condition::Le(a, b)
            | Condition::Gt(a, b)
            | Condition::Ge(a, b) => Box::new([a, b].into_iter()),

            Condition::And(conditions) | Condition::Or(conditions) => {
                Box::new(conditions.into_iter().flat_map(|c| c.into_children()))
            }

            Condition::Not(condition) => condition.into_children(),
        }
    }

    pub fn hash_structure<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Condition::And(conds) | Condition::Or(conds) => {
                conds.len().hash(state);
                for c in conds.iter() {
                    c.hash_structure(state);
                }
            }
            Condition::Not(cond) => {
                cond.hash_structure(state);
            }
            _ => (),
        }
    }
}
