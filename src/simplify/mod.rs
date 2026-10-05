use std::{
    array,
    cell::{Cell, OnceCell},
    collections::{HashMap, HashSet},
    hash::{BuildHasher, Hash, RandomState},
    iter::once,
    mem,
    ops::Mul,
    rc::Rc,
    sync::LazyLock,
    time::Duration,
};

use ahash::AHashMap;
use itertools::Itertools;
use kinded::Kinded;
use nlopt::Algorithm::Newuoa;
use num::{One, Zero, complex::ComplexFloat, pow::Pow as _};
use xxhash_rust::xxh3::Xxh3Builder;

use crate::{
    core::{
        interned::Interned,
        value::{ComplexExt, Value, gcd_f64},
    },
    expr::{
        Expr, NodeId,
        domain::Domain,
        shape::Shape,
        tree::{Branch, BranchKind, Leaf, Node, NodeKind},
    },
    simplify::pattern::{Machine, Pattern, Program, Rule, Wildcard},
    symbol::{Symbol, constants::Constant},
    units::Quantity,
};

/* --------------------------------- MODULES -------------------------------- */

pub mod pattern;
pub mod rules;

#[cfg(test)]
mod test;

/* --------------------------------- STRUCTS -------------------------------- */

#[derive(Clone, Copy, Hash, PartialEq, Eq, Default)]
pub struct ClassId(usize);

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct ExprId(usize);

pub struct Substitution {
    bindings: AHashMap<Wildcard, ClassId>,
}

#[derive(Default)]
pub struct EquivalencyGraph {
    cons: AHashMap<Key, ClassId>,
    exprs: Vec<EquivalencyExpr>,
    classes: Vec<EquivalencyClass>,
    root: ClassId,
}

pub struct EquivalencyClass {
    exprs: Vec<ExprId>,
    exprs_by_kind: AHashMap<NodeKind, Vec<ExprId>>,
    parent: Cell<ClassId>,
    domain: Domain,
    shape: Shape,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key(u128);

type EquivalencyNode = Node<ClassId>;

#[derive(PartialEq, Clone, Eq, Hash)]
pub enum EquivalencyNodeStructure {
    Leaf(Leaf),
    Branch(BranchKind),
}

#[derive(Clone, Eq)]
pub struct EquivalencyExpr {
    node: EquivalencyNode,
    key: OnceCell<Key>,
}

impl EquivalencyClass {
    pub fn domain(&self) -> Domain {
        self.domain
    }

    pub fn shape(&self) -> Shape {
        self.shape
    }
}

impl EquivalencyNodeStructure {
    fn kind(&self) -> NodeKind {
        match self {
            EquivalencyNodeStructure::Leaf(leaf) => NodeKind::Leaf(leaf.kind()),
            EquivalencyNodeStructure::Branch(b) => NodeKind::Branch(*b),
        }
    }
}

impl EquivalencyNode {
    fn structure(&self) -> EquivalencyNodeStructure {
        match self {
            EquivalencyNode::Branch(branch) => {
                EquivalencyNodeStructure::Branch(branch.kind())
            }
            EquivalencyNode::Leaf(leaf) => {
                EquivalencyNodeStructure::Leaf(leaf.clone())
            }
        }
    }
}

impl EquivalencyExpr {
    fn build(
        graph: &mut EquivalencyGraph,
        pat: &Pattern,
        sub: &Substitution,
    ) -> ClassId {
        match pat {
            Pattern::Wildcard(w) => sub.bindings[w],
            Pattern::Node(node) => match node {
                Node::Leaf(leaf) => {
                    let expr = EquivalencyExpr {
                        key: OnceCell::new(),
                        node: EquivalencyNode::Leaf(leaf.clone()),
                    };

                    graph.add(expr)
                }
                Node::Branch(branch) => {
                    let expr = EquivalencyExpr {
                        key: OnceCell::new(),
                        node: node
                            .clone()
                            .map(&mut |x| Self::build(graph, &x, sub)),
                    };

                    graph.add(expr)
                }
            },
        }
    }

    fn key(&self) -> Key {
        *self.key.get_or_init(|| {
            let mut hasher = Xxh3Builder::new().with_seed(1).build();

            self.node.structure().hash(&mut hasher);

            // for child in self.node.children() {
            //     child.hash(&mut hasher);
            // }
            self.node.for_each_child(|child| child.hash(&mut hasher));

            Key(hasher.digest128())
        })
    }
}

impl PartialEq for EquivalencyExpr {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl Hash for EquivalencyExpr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u128(self.key().0);
    }
}

impl EquivalencyGraph {
    fn add(&mut self, expr: EquivalencyExpr) -> ClassId {
        if let Some(existing) = self.cons.get(&expr.key()) {
            return self.find(*existing);
        } else {
            let expr_id = ExprId(self.exprs.len());
            let class_id = ClassId(self.classes.len());
            self.cons.insert(expr.key(), class_id);

            let class = EquivalencyClass {
                parent: Cell::new(class_id),
                exprs: vec![expr_id],
                exprs_by_kind: [(expr.node.kind(), vec![expr_id])].into(),
                domain: expr.node.domain(),
                shape: expr.node.shape(),
            };

            self.exprs.push(expr);
            self.classes.push(class);
            // self.pending.insert(class_id);

            class_id
        }
    }

    pub fn rewrite(&mut self, rules: &[Rule]) {
        const CLASS_LIMIT: usize = 5000;
        const MATCH_LIMIT: usize = 5000;
        const PER_RULE_MATCH_LIMIT: usize = 500;

        #[derive(Default, Clone, Copy)]
        struct Productivity {
            merged: usize,
            matched: usize,
            ban_length: usize = 4,
            banned_for: usize,
        }

        // Compile

        let compiled = rules
            .iter()
            .map(|r| (r.from.kind(), r.from.compile(&r.conds), &r.to))
            .collect_vec();

        let mut prod = vec![Productivity::default(); compiled.len()];

        loop {
            let mut matches = Vec::new();

            // Search

            for (rule_idx, (root_kind, program, to)) in
                compiled.iter().enumerate()
            {
                let prod = &mut prod[rule_idx];
                if prod.banned_for > 0 {
                    prod.banned_for -= 1;
                    continue;
                }

                if prod.matched > 10 && prod.matched > prod.merged * 2 {
                    prod.banned_for += prod.ban_length;
                    prod.ban_length *= 3;
                    prod.merged = 0;
                    prod.matched = 0;
                    continue;
                }

                prod.merged = 0;
                prod.matched = 0;

                for class_id in 0..self.classes.len() {
                    // why even bother matching if this class
                    // dosent even contain any nodes with that kind
                    if let Some(root_kind) = root_kind
                        && !self.classes[class_id]
                            .exprs_by_kind
                            .contains_key(root_kind)
                    {
                        continue;
                    }

                    let class_id = ClassId(class_id);

                    let mut subs = Vec::new();
                    Machine::execute(self, &program, class_id, &mut subs);

                    for sub in subs {
                        prod.matched += 1;
                        if prod.matched >= PER_RULE_MATCH_LIMIT
                            || matches.len() >= MATCH_LIMIT
                        {
                            break;
                        }

                        matches.push((rule_idx, class_id, to, sub));
                    }
                }
            }

            let mut graph_changed = false;

            // Apply
            for (rule_idx, id, to, sub) in matches {
                let found = EquivalencyExpr::build(self, &to, &sub);

                if self.find(id) != self.find(found) {
                    self.union(id, found);
                    // self.pending.insert(id);
                    prod[rule_idx].merged += 1;
                    graph_changed = true;
                }
            }

            // Rebuild
            self.rebuild();

            if !graph_changed || self.classes.len() > CLASS_LIMIT {
                break;
            }
        }
    }

    fn rebuild(&mut self) {
        loop {
            let mut changed_roots = Vec::new();

            let mut unions = Vec::with_capacity(self.classes.len());
            for i in 0..self.classes.len() {
                unions.push(self.find(ClassId(i)));
            }

            for i in 0..self.classes.len() {
                let id = ClassId(i);
                let root = unions[i];

                if id != root {
                    let class = &mut self.classes[id.0];

                    if !class.exprs.is_empty() {
                        let mut exprs = mem::take(&mut class.exprs);
                        let exprs_by_kind = mem::take(&mut class.exprs_by_kind);

                        let parent_class = &mut self.classes[root.0];
                        parent_class.exprs.append(&mut exprs);

                        for (kind, mut ids) in exprs_by_kind {
                            parent_class
                                .exprs_by_kind
                                .entry(kind)
                                .or_default()
                                .append(&mut ids);
                        }

                        changed_roots.push(root);
                    }
                }
            }

            changed_roots.sort_unstable_by_key(|x| x.0);
            changed_roots.dedup();

            for root in changed_roots {
                let class = &mut self.classes[root.0];
                class.exprs.sort_unstable_by_key(|x| x.0);
                class.exprs.dedup();
                for entry in class.exprs_by_kind.values_mut() {
                    entry.sort_unstable_by_key(|x| x.0);
                    entry.dedup();
                }
            }

            self.cons.clear();
            let mut converged = true;

            for class_id in 0..self.classes.len() {
                let id = ClassId(class_id);

                if unions[class_id] != id {
                    continue;
                }

                let class = &self.classes[class_id];

                for &expr_id in &class.exprs {
                    let key = {
                        let expr = &mut self.exprs[expr_id.0];

                        let mut changed = false;
                        expr.node.for_each_child_mut(|child| {
                            let root = unions[child.0];
                            if *child != root {
                                *child = root;
                                changed = true;
                            }
                        });

                        if changed {
                            expr.key = OnceCell::new();
                        }

                        expr.key()
                    };
                    if let Some(&existing) = self.cons.get(&key) {
                        let root_a = self.find(existing);
                        let root_b = self.find(id);

                        if root_a != root_b {
                            self.union(root_a, root_b);
                            converged = false;
                        }
                    } else {
                        self.cons.insert(key, id);
                    }
                }
            }

            if converged {
                break;
            }
        }
    }

    fn find(&self, id: ClassId) -> ClassId {
        let class = &self.classes[id.0];

        if class.parent.get() != id {
            let up = self.find(class.parent.get());
            self.classes[id.0].parent.set(up);
            up
        } else {
            id
        }
    }

    fn union(&self, a: ClassId, b: ClassId) {
        let root_a = self.find(a);
        let root_b = self.find(b);

        if root_a != root_b {
            assert_eq!(
                self.classes[root_a.0].shape, self.classes[root_b.0].shape,
                "Cannot union two classes of different shapes"
            );
            // Merge root_b into root_a
            self.classes[root_b.0].parent.set(root_a);
        }
    }

    pub(crate) fn extract(&self) -> Expr {
        fn calculate_cost(
            node: &EquivalencyNode,
            best_nodes: &AHashMap<ClassId, (usize, EquivalencyNode)>,
        ) -> Option<usize> {
            let mut node_cost = match node {
                Node::Leaf(leaf) => match leaf {
                    Leaf::Symbol(symbol) => 1,
                    Leaf::Constant(constant) => 0,
                    Leaf::Quantity(quantity) => 0,
                },
                Node::Branch(branch) => match branch {
                    Branch::Add(_)
                    | Branch::Mul(_)
                    | Branch::Min(_)
                    | Branch::Max(_) => 2,
                    Branch::Sin(_)
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
                    | Branch::Atanh(_) => 3,
                    Branch::Atan2 { .. } | Branch::Arg(_) | Branch::Norm(_) => {
                        4
                    }
                    Branch::Conj(_)
                    | Branch::Sign(_)
                    | Branch::Real(_)
                    | Branch::Imag(_) => 2,
                    Branch::Pow { .. } => 5,
                    Branch::Log { .. } => 5,
                    Branch::Matrix(_) => todo!(),
                    Branch::Transpose(_) => todo!(),
                    Branch::Det(_) => todo!(),
                    Branch::Rank(_) => todo!(),
                    Branch::Trace(_) => todo!(),
                    Branch::Conditional { .. } => todo!(),
                },
            };

            let mut has_cost = true;
            node.for_each_child(|child_id| {
                if let Some((child_cost, _)) = best_nodes.get(&child_id)
                    && has_cost
                {
                    node_cost = node_cost + *child_cost;
                } else {
                    has_cost = false;
                }
            });

            if has_cost { Some(node_cost) } else { None }
        }

        let mut best_nodes =
            AHashMap::<ClassId, (usize, EquivalencyNode)>::new();
        let mut converged = false;

        while !converged {
            converged = true;

            for class_id in 0..self.classes.len() {
                let class_id = ClassId(class_id);
                let root_id = self.find(class_id);
                let class = &self.classes[root_id.0];

                for expr_id in &class.exprs {
                    let expr = &self.exprs[expr_id.0];

                    if let Some(node_cost) =
                        calculate_cost(&expr.node, &best_nodes)
                    {
                        let current_best_cost = best_nodes
                            .get(&class_id)
                            .map(|(cost, _)| *cost)
                            .unwrap_or(usize::MAX);

                        if node_cost < current_best_cost {
                            best_nodes.insert(
                                class_id,
                                (node_cost, expr.node.clone()),
                            );
                            converged = false;
                        }
                    }
                }
            }
        }

        let mut expr = Expr::new();

        fn build_extracted(
            class_id: ClassId,
            graph: &EquivalencyGraph,
            best_nodes: &AHashMap<ClassId, (usize, EquivalencyNode)>,
            into: &mut Expr,
        ) -> NodeId {
            let (_, node) = &best_nodes[&class_id];

            let mapped_node = node.clone().map(&mut |child_id| {
                build_extracted(child_id, graph, best_nodes, into)
            });

            into.push(mapped_node)
        }

        let root_id = build_extracted(self.root, &self, &best_nodes, &mut expr);
        expr.set_root(root_id);
        expr
    }

    pub(crate) fn build(expr: &Expr) -> Self {
        let mut graph = Self::default();

        let root = expr.fold(|_, _, node| {
            let expr = EquivalencyExpr { node, key: OnceCell::new() };
            graph.add(expr)
        });
        graph.root = root;

        graph
    }
}

impl EquivalencyNode {
    fn domain(&self) -> Domain {
        Domain::REAL
    }

    fn shape(&self) -> Shape {
        Shape::SCALAR
    }
}
