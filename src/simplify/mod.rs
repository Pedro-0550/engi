use std::{
    array,
    cell::{Cell, OnceCell},
    collections::HashMap,
    hash::{BuildHasher, Hash, RandomState},
    iter::once,
    mem,
    ops::Mul,
    rc::Rc,
    sync::LazyLock,
    time::Duration,
};

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
        tree::{Branch, BranchKind, Leaf, Node},
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
    bindings: HashMap<Wildcard, ClassId>,
}

#[derive(Default)]
pub struct EquivalencyGraph {
    cons: HashMap<Key, ClassId>,
    exprs: Vec<EquivalencyExpr>,
    classes: Vec<EquivalencyClass>,
    root: ClassId,
}

pub struct EquivalencyClass {
    exprs: Vec<ExprId>,
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

            for child in self.node.children() {
                child.hash(&mut hasher);
            }

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
            let node_id = ExprId(self.exprs.len());
            let class_id = ClassId(self.classes.len());
            self.cons.insert(expr.key(), class_id);

            let class = EquivalencyClass {
                parent: Cell::new(class_id),
                exprs: vec![node_id],
                domain: expr.node.domain(),
                shape: expr.node.shape(),
            };

            self.exprs.push(expr);
            self.classes.push(class);

            class_id
        }
    }

    pub fn rewrite(&mut self, rules: &[Rule]) {
        const CLASS_LIMIT: usize = 10_000;
        const MATCH_LIMIT: usize = 10_000;
        const PER_RULE_MATCH_LIMIT: usize = 1_000;

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
            .map(|r| (r.from.compile(&r.conds), &r.to))
            .collect_vec();

        let mut prod = vec![Productivity::default(); compiled.len()];

        loop {
            let mut matches = Vec::new();

            // Search

            for (rule_idx, (program, to)) in compiled.iter().enumerate() {
                let prod = &mut prod[rule_idx];
                if prod.banned_for > 0 {
                    prod.banned_for -= 1;
                    continue;
                }

                if prod.matched > 12 && prod.matched > prod.merged * 5 {
                    prod.banned_for += prod.ban_length;
                    prod.ban_length *= 2;
                    prod.merged = 0;
                    prod.matched = 0;
                    continue;
                }

                prod.merged = 0;
                prod.matched = 0;

                for class_id in 0..self.classes.len() {
                    let id = ClassId(class_id);

                    let mut subs = Vec::new();
                    Machine::execute(self, &program, id, &mut subs);

                    for sub in subs {
                        prod.matched += 1;
                        if prod.matched >= PER_RULE_MATCH_LIMIT
                            || matches.len() >= MATCH_LIMIT
                        {
                            break;
                        }

                        matches.push((rule_idx, id, to, sub));
                    }
                }
            }

            let mut graph_changed = false;

            // Apply
            for (rule_idx, id, to, sub) in matches {
                let found = EquivalencyExpr::build(self, &to, &sub);

                if self.find(id) != self.find(found) {
                    self.union(id, found);
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
        let mut converged = false;

        while !converged {
            converged = true;
            let mut unions = HashMap::new();

            for id in 0..self.classes.len() {
                let id = ClassId(id);
                let parent = self.find(id);
                unions.insert(id, parent);

                let class = &mut self.classes[id.0];
                if parent != id {
                    let mut exprs = mem::take(&mut class.exprs);
                    let parent = &mut self.classes[parent.0];
                    parent.exprs.append(&mut exprs);
                }
            }

            self.cons.clear();

            for class_id in 0..self.classes.len() {
                let class = &self.classes[class_id];

                for expr_id in &class.exprs {
                    let expr = &mut self.exprs[expr_id.0];

                    for child in expr.node.children_mut() {
                        *child = unions[&*child]
                    }
                    expr.key = OnceCell::new();

                    if let Some(existing) = self.cons.get(&expr.key()) {
                        let root_a = self.find(*existing);
                        let root_b = self.find(ClassId(class_id));

                        if root_a != root_b {
                            self.union(root_a, root_b);
                            converged = false;
                        }
                    } else {
                        self.cons.insert(expr.key(), ClassId(class_id));
                    }
                }
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
            best_nodes: &HashMap<ClassId, (usize, EquivalencyNode)>,
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

            for child_id in node.children() {
                if let Some((child_cost, _)) = best_nodes.get(&child_id) {
                    node_cost = node_cost + *child_cost;
                } else {
                    return None;
                }
            }
            Some(node_cost)
        }

        let mut best_nodes =
            HashMap::<ClassId, (usize, EquivalencyNode)>::new();
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
            best_nodes: &HashMap<ClassId, (usize, EquivalencyNode)>,
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

        let root = expr.fold_dfs(|_, _, node| {
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
