use std::{
    array,
    cell::{Cell, OnceCell},
    collections::{HashMap, HashSet},
    hash::{BuildHasher, Hash, RandomState},
    iter::once,
    mem::{self, variant_count},
    ops::Mul,
    rc::Rc,
    sync::LazyLock,
    thread,
    time::Duration,
};

use ahash::AHashMap;
use itertools::Itertools;
use kinded::Kinded;
use nlopt::Algorithm::Newuoa;
use num::{One, Zero, complex::ComplexFloat, pow::Pow as _};

use crate::{
    core::{
        consed::Insertion::{self, Existed},
        index::Indexed,
        interned::Interned,
        value::{ComplexExt, Value, gcd_f64},
    },
    expr::{
        BuiltOperand, Expr, FetchedOperand, Operand,
        dag::{
            self, ConditionId, Dag, Leaf, LeafKind, Matrix, Operation,
            OperationId, OperationKind,
        },
        domain::Domain,
        shape::Shape,
    },
    id,
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

id!(ClassId);

pub struct Substitution {
    bindings: AHashMap<Wildcard, ClassId>,
}

type EquivalencyDag = Dag<ClassId, ClassId, ClassId, ClassId>;

pub struct EquivalencyGraph {
    dag: EquivalencyDag,
    classes: Indexed<Vec<EquivalencyClass>, ClassId>,
    worklist: Vec<ClassId>,
    matchlist: Vec<ClassId>,
}

pub struct EquivalencyClass {
    // while this does use a bit more memory, its 12% faster over a hashmap,
    // even though a hashmap would only need to hash 2 bytes..
    // I guess its done so often that its worth it, at least thats what i got from profiling
    operands_by_kind: [Vec<Operand>; {
        variant_count::<LeafKind>() + variant_count::<OperationKind>() + 1 // 1 for matrix
    }],
    operands: Vec<Operand>,
    child_of: Vec<Operand>,

    parent: Cell<ClassId>,
    analysis: Analysis,
}

pub struct Analysis {
    domain: Domain,
    shape: Shape,
    qty: Option<Quantity>,
}

impl EquivalencyClass {
    pub fn domain(&self) -> Domain {
        self.analysis.domain
    }

    pub fn shape(&self) -> Shape {
        self.analysis.shape
    }
}

impl FetchedOperand<'_> {
    fn key(&self) -> u8 {
        match self {
            Operand::Leaf(leaf) => leaf.kind() as u8,
            Operand::Operation(op) => {
                op.kind() as u8 + (variant_count::<LeafKind>() as u8)
            }
            Operand::Matrix(_) => {
                variant_count::<OperationKind>() as u8
                    + variant_count::<LeafKind>() as u8
                    + 1
            }
            Operand::Wildcard(_) => {
                variant_count::<OperationKind>() as u8
                    + variant_count::<LeafKind>() as u8
                    + 2
            }
        }
    }
}

impl Operand<Wildcard> {
    fn build(
        &self,
        pat: Pattern,
        graph: &mut EquivalencyGraph,
        sub: &Substitution,
    ) -> ClassId {
        match pat.fetch(*self) {
            // Pattern::Wildcard(w) => sub.bindings[w],
            // Pattern::Node(node) => match node {
            //     Node::Leaf(leaf) => {
            //         let expr = EquivalencyExpr {
            //             key: OnceCell::new(),
            //             node: EquivalencyNode::Leaf(leaf.clone()),
            //         };

            //         graph.add(expr)
            //     }
            //     Node::Branch(branch) => {
            //         let expr = EquivalencyExpr {
            //             key: OnceCell::new(),
            //             node: node
            //                 .clone()
            //                 .map(&mut |x| Self::build(graph, &x, sub)),
            //         };

            //         graph.add(expr)
            //     }
            // },
            Operand::Leaf(b) => graph.add(Operand::Leaf(b)),
            Operand::Matrix(_) => todo!(),
            Operand::Operation(_) => todo!(),
            Operand::Wildcard(w) => sub.bindings[&w],
        }
    }

    // fn key(&self) -> Key {
    //     *self.key.get_or_init(|| {
    //         let mut hasher = Xxh3Builder::new().with_seed(1).build();

    //         self.node.structure().hash(&mut hasher);

    //         // for child in self.node.children() {
    //         //     child.hash(&mut hasher);
    //         // }
    //         self.node.for_each_child(|child| child.hash(&mut hasher));

    //         Key(hasher.digest128())
    //     })
    // }
}

// impl PartialEq for EquivalencyExpr {
//     fn eq(&self, other: &Self) -> bool {
//         self.key() == other.key()
//     }
// }

// impl Hash for EquivalencyExpr {
//     fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
//         state.write_u128(self.key().0);
//     }
// }

impl EquivalencyGraph {
    fn add(
        &mut self,
        built_op: Operand<
            !,
            Operation<ClassId, ConditionId>,
            Leaf,
            Matrix<ClassId>,
        >,
    ) -> ClassId {
        let next_id = self.classes.next();
        let op = match built_op {
            Operand::Leaf(leaf) => {
                match self.dag.leaves.insert_with(leaf, next_id) {
                    Insertion::Existed(existing) => return self.find(existing),
                    Insertion::Added(id) => Operand::Leaf(id),
                }
            }
            Operand::Matrix(m) => {
                match self.dag.matrices.insert_with(m, next_id) {
                    Insertion::Existed(existing) => return self.find(existing),
                    Insertion::Added(id) => Operand::Matrix(id),
                }
            }
            Operand::Operation(c) => {
                match self.dag.operations.insert_with(c, next_id) {
                    Insertion::Existed(existing) => return self.find(existing),
                    Insertion::Added(id) => Operand::Operation(id),
                }
            }
            Operand::Wildcard(_) => unreachable!(),
        };

        let qty = self.evaluate(op);

        let class = EquivalencyClass {
            operands_by_kind: array::from_fn(|i| {
                if built_op.kind().id() == i as u8 { vec![op] } else { vec![] }
            }),
            operands: vec![op],
            child_of: vec![],
            parent: Cell::new(next_id),
            analysis: Analysis {
                domain: self.domain(op),
                shape: self.shape(op),
                qty,
            },
        };

        self.matchlist.push(next_id);
        self.worklist.push(next_id);
        self.classes.push(class);

        if let Some(ref q) = qty {
            let qty_class = self.add(q.into());
            self.union(next_id, qty_class);
        }

        next_id
    }

    fn evaluate(&self, op: Operand) -> Option<Quantity> {
        match op {
            Operand::Leaf(id) => match &self.dag.leaves[id] {
                Leaf::Quantity(quantity) => Some(quantity.clone()),
                _ => None,
            },
            Operand::Operation(id) => Some(
                match self.dag.operations[id].map(
                    |c| self.classes[self.find(*c)].analysis.qty.clone(),
                    |_| (),
                ) {
                    Operation::Add([a, b]) => {
                        if a.as_ref()?.value().is_zero() {
                            return b;
                        } else if b.as_ref()?.value().is_zero() {
                            return a;
                        }

                        if a.as_ref()?.unit() == b.as_ref()?.unit() {
                            a? + b?
                        } else {
                            return None;
                        }
                    }
                    Operation::Mul([a, b]) => {
                        if a.as_ref().or(b.as_ref())?.value().is_zero() {
                            0.0.into()
                        } else {
                            a? * b?
                        }
                    }
                    Operation::Min([a, b]) => a?.max(&b?),
                    Operation::Max([a, b]) => a?.min(&b?),
                    Operation::Pow { base, exp } => base?.pow(exp?),
                    Operation::Log { base, arg } => arg?.log(base?),
                    Operation::Atan2 { x, y } => x?.atan2(&y?),
                    Operation::Sin(u) => u?.sin(),
                    Operation::Cos(u) => u?.cos(),
                    Operation::Tan(u) => u?.tan(),
                    Operation::Asin(u) => u?.asin(),
                    Operation::Acos(u) => u?.acos(),
                    Operation::Atan(u) => u?.atan(),
                    Operation::Sinh(u) => u?.sinh(),
                    Operation::Cosh(u) => u?.cosh(),
                    Operation::Tanh(u) => u?.tanh(),
                    Operation::Asinh(u) => u?.asinh(),
                    Operation::Acosh(u) => u?.acosh(),
                    Operation::Atanh(u) => u?.atanh(),
                    Operation::Arg(u) => u?.arg(),
                    Operation::Conj(u) => u?.conj(),
                    Operation::Norm(u) => u?.norm(),
                    Operation::Sign(u) => u?.sign(),
                    Operation::Real(u) => u?.real(),
                    Operation::Imag(u) => u?.imag(),
                    Operation::Transpose(_) => todo!(),
                    Operation::Det(_) => todo!(),
                    Operation::Rank(_) => todo!(),
                    Operation::Trace(_) => todo!(),
                    Operation::Conditional { cond, pass, fail } => return None,
                },
            ),
            Operand::Matrix(_) => todo!(),
        }
    }

    pub fn rewrite(&mut self, rules: &[Rule]) {
        const CLASS_LIMIT: usize = 5000;
        const MATCH_LIMIT: usize = 1000;
        const PER_RULE_MATCH_LIMIT: usize = 100;

        #[derive(Default, Clone, Copy)]
        struct Productivity {
            merged: usize,
            matched: usize,
            ban_length: usize = 8,
            banned_for: usize,
        }

        // Compile

        let compiled = rules
            .iter()
            .map(|r| {
                (
                    r.from.root().map(|op| r.from.fetch(op)).unwrap().kind(),
                    r.from.compile(&r.conds, &mut self.dag),
                    &r.to,
                )
            })
            .collect_vec();

        let mut prod = vec![Productivity::default(); compiled.len()];

        loop {
            let mut matches = Vec::new();

            let mut pending_match = mem::take(&mut self.matchlist);

            for pending in &mut pending_match {
                *pending = self.find(*pending);
            }

            pending_match.sort_unstable();
            pending_match.dedup();

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

                for class_id in &pending_match {
                    // why even bother matching if this class dosent contain any nodes of that kind
                    if self.classes[*class_id].operands_by_kind
                        [root_kind.id() as usize]
                        .is_empty()
                    {
                        continue;
                    }

                    let mut subs = Vec::new();
                    Machine::execute(self, &program, *class_id, &mut subs);

                    for sub in subs {
                        prod.matched += 1;
                        if prod.matched >= PER_RULE_MATCH_LIMIT
                            || matches.len() >= MATCH_LIMIT
                        {
                            break;
                        }

                        matches.push((rule_idx, *class_id, to, sub));
                    }
                }
            }

            let mut graph_changed = false;

            // Apply
            for (rule_idx, id, to, sub) in matches {
                let found = EquivalencyExpr::build(self, &to, &sub);

                if self.find(id) != self.find(found) {
                    self.union(id, found);
                    self.matchlist.push(id);
                    self.pending_fold.push(id);

                    prod[rule_idx].merged += 1;
                    graph_changed = true;
                }
            }

            // Rebuild
            self.rebuild();

            if !graph_changed || self.classes.len() > CLASS_LIMIT {
                break;
            }

            // println!(
            //     "tid {:?} -> iter {i}, {} classes, {} exprs",
            //     thread::current().id(),
            //     self.classes.len(),
            //     self.exprs.len()
            // );
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
                        let mut child_of = mem::take(&mut class.child_of);
                        let mut exprs_by_kind = class
                            .exprs_by_kind
                            .each_mut()
                            .map(|exprs| mem::take(exprs));
                        let qty = class.qty.take();

                        let parent_class = &mut self.classes[root.0];
                        parent_class.exprs.append(&mut exprs);
                        parent_class.child_of.append(&mut child_of);

                        if let Some(ref parent_qty) = parent_class.qty
                            && let Some(ref new_qty) = qty
                        {
                            // assert_eq!(
                            //     parent_qty, new_qty,
                            //     "Constant parts of unioned classes should be equal"
                            // );
                        } else {
                            parent_class.qty = parent_class.qty.take().or(qty)
                        }

                        for (i, mut ids) in exprs_by_kind.iter_mut().enumerate()
                        {
                            parent_class.exprs_by_kind[i].append(&mut ids);
                        }

                        changed_roots.push(root);
                    }
                }
            }

            changed_roots.sort_unstable_by_key(|x| x.0);
            changed_roots.dedup();

            for root in changed_roots {
                self.matchlist.push(root);

                let class = &mut self.classes[root.0];
                class.exprs.sort_unstable_by_key(|x| x.0);
                class.exprs.dedup();

                class.child_of.sort_unstable_by_key(|x| x.0);
                class.child_of.dedup();

                for entry in &mut class.exprs_by_kind {
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
                            self.matchlist.push(id);
                            self.pending_fold.push(id);
                        }

                        expr.key()
                    };
                    if let Some(&existing) = self.cons.get(&key) {
                        let root_a = self.find(existing);
                        let root_b = self.find(id);

                        if root_a != root_b {
                            self.union(root_a, root_b);
                            self.matchlist.push(root_a);
                            self.pending_fold.push(root_a);

                            converged = false;
                        }
                    } else {
                        self.cons.insert(key, id);
                    }
                }
            }

            if converged {
                let mut pending_fold = mem::take(&mut self.pending_fold);
                pending_fold.sort_unstable();
                pending_fold.dedup();

                let mut discovered = Vec::new();

                for id in pending_fold {
                    let root = self.find(id);

                    if self.classes[root.0].qty.is_some() {
                        continue;
                    }

                    for &expr_id in &self.classes[root.0].exprs {
                        if let Some(qty) =
                            self.evaluate_expr(&self.exprs[expr_id.0])
                        {
                            discovered.push((root, qty));
                            break;
                        }
                    }
                }

                if !discovered.is_empty() {
                    for (root, qty) in discovered {
                        self.classes[root.0].qty = Some(qty.clone());

                        let qty_expr = EquivalencyExpr {
                            node: Node::Leaf(qty.into()),
                            key: OnceCell::new(),
                        };

                        let qty_id = self.add(qty_expr);
                        let new_root = self.find(root);
                        let qty_root = self.find(qty_id);

                        if new_root != qty_root {
                            self.union(new_root, qty_root);
                            self.matchlist.push(new_root);
                            self.pending_fold.push(new_root);

                            converged = false;
                        }
                    }
                }

                if converged {
                    break;
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
        #[inline]
        fn calculate_cost(
            node: &EquivalencyNode,
            best_nodes: &AHashMap<ClassId, (usize, EquivalencyNode)>,
        ) -> Option<usize> {
            let mut node_cost = match node {
                Node::Leaf(leaf) => match leaf {
                    Leaf::Symbol(symbol) => 1,
                    Leaf::Constant(constant) => 1,
                    Leaf::Quantity(quantity) => 1,
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

    fn domain(&self, expr: &EquivalencyExpr) -> Domain {
        expr.node.clone().map(&mut |id| self.classes[id.0].domain).domain()
    }

    fn shape(&self, expr: &EquivalencyExpr) -> Shape {
        expr.node.clone().map(&mut |id| self.classes[id.0].shape).shape()
    }
}
