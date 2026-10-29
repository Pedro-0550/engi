use std::{
    array,
    cell::{Cell, OnceCell},
    collections::{HashMap, HashSet, hash_map::Entry},
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
use float_eq::assert_float_eq;
use itertools::Itertools;
use kinded::Kinded;
use nlopt::Algorithm::Newuoa;
use num::{One, Zero, complex::ComplexFloat, pow::Pow as _};

use crate::{
    core::{
        consed::Insertion::{self, Existed},
        index::{Indexed, Size},
        interned::Interned,
        value::{ComplexExt, Value, gcd_f64},
    },
    expr::{
        Expr, FoldedOperand, Operand,
        dag::{
            self, ConditionId, Dag, Leaf, LeafKind, Matrix, Operation,
            OperationId, OperationKind,
        },
        domain::Domain,
        shape::Shape,
    },
    id,
    simplify::pattern::{
        Machine, Pattern, Program, Rule, WILDCARD_COUNT, Wildcard,
    },
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
    bindings: Indexed<[Option<ClassId>; WILDCARD_COUNT], Wildcard>,
}

#[derive(Default)]
pub struct EquivalencyGraph {
    dag: EquivalencyDag,
    classes: Indexed<Vec<EquivalencyClass>, ClassId>,
    matchlist: Vec<ClassId>,
    updatelist: Vec<ClassId>,
    dirty: bool,
}

pub struct EquivalencyClass {
    // while this does use a bit more memory, its 12% faster over a hashmap,
    // even though a hashmap would only need to hash 2 bytes..
    // I guess its done so often that its worth it, at least thats what i got from profiling
    operands_by_kind: [Vec<Operand>; {
        variant_count::<LeafKind>() + variant_count::<OperationKind>() + 1 // 1 for matrix
    }],
    operands: Vec<Operand>,
    child_of: Vec<ClassId>,
    size: Size,

    parent: Cell<ClassId>,
    analysis: Analysis,
}

#[derive(Clone)]
pub struct Analysis {
    domain: Domain,
    shape: Shape,
    qty: Option<Quantity>,
}

/* --------------------------------- ALIASES -------------------------------- */

type EquivalencyDag = Dag<ClassId, ClassId, ClassId, ClassId>;

// An equivalency graph's fetched operand, which is different from an expression's fetched operand
type FetchedOperand<'a> = Operand<
    !,
    &'a Operation<ClassId, ConditionId>,
    &'a Leaf,
    &'a Matrix<ClassId>,
>;

type BuiltOperand =
    Operand<!, Operation<ClassId, ConditionId>, Leaf, Matrix<ClassId>>;

/* ---------------------------------- IMPLS --------------------------------- */

impl Analysis {
    pub fn merge_with(&mut self, other: Analysis) -> bool {
        // shapes must be eq
        // domain merges to the most restrictive of the two (eventually)
        // qty merges when one of them is None and needs to be approx eq if both are Some
        // returns true if either side gains information
        assert_eq!(self.shape, other.shape);

        match (&self.qty, other.qty) {
            (None, Some(other)) => {
                self.qty.insert(other);
                true
            }
            // This is true because it implies that the other side gained information as well, even if we dont mutate it here.
            (Some(_), None) => true,
            (Some(a), Some(b)) => {
                // we should implemnet some form of approx comparison, which is kinda hard since Value
                // can be a lot of different things
                false
            }
            _ => false,
        }
    }

    pub fn update(id: ClassId, graph: &mut EquivalencyGraph) -> Option<Self> {
        // Once we have domain min (like returning the smallest of two domains)
        // we should implemnet domain updating as well
        let class = &graph.classes[id];

        if class.analysis.qty.is_some() {
            return None;
        };

        let mut new = class.analysis.clone();

        for op in &class.operands {
            if let Some(qty) = graph.evaluate(*op) {
                new.qty = Some(qty);
                return Some(new);
            }
        }

        None
    }
}

impl FetchedOperand<'_> {
    // fn key(&self) -> usize {
    //     match self {
    //         Operand::Leaf(leaf) => leaf.kind() as usize,
    //         Operand::Operation(op) => {
    //             op.kind() as usize + (variant_count::<LeafKind>() as usize)
    //         }
    //         Operand::Matrix(_) => {
    //             variant_count::<OperationKind>() as usize
    //                 + variant_count::<LeafKind>() as usize
    //         }
    //         Operand::Wildcard(_) => {
    //             variant_count::<OperationKind>() as usize
    //                 + variant_count::<LeafKind>() as usize
    //                 + 1
    //         }
    //     }
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
    #[inline(always)]
    pub fn fetch(&self, op: Operand) -> FetchedOperand<'_> {
        match op {
            Operand::Leaf(id) => Operand::Leaf(&self.dag.leaves[id]),
            Operand::Matrix(id) => Operand::Matrix(&self.dag.matrices[id]),
            Operand::Operation(id) => {
                Operand::Operation(&self.dag.operations[id])
            }
            Operand::Wildcard(w) => w,
        }
    }

    fn add(&mut self, built_op: BuiltOperand) -> ClassId {
        let next_id = self.classes.next();
        let kind = built_op.kind();

        let canon_op = match built_op {
            Operand::Operation(o) => {
                Operand::Operation(o.map(|c| self.find(*c), |c| *c))
            }
            Operand::Matrix(m) => Operand::Matrix(m.map(|c| self.find(*c))),
            other => other,
        };

        let op = match canon_op {
            Operand::Leaf(leaf) => {
                match self.dag.leaves.insert_with(leaf, next_id) {
                    Insertion::Existed(existing) => return self.find(existing),
                    Insertion::Added(id) => Operand::Leaf(id),
                }
            }
            Operand::Matrix(m) => {
                match self.dag.matrices.insert_with(m, next_id) {
                    Insertion::Existed(existing) => return self.find(existing),
                    Insertion::Added(id) => {
                        let op = Operand::Matrix(id);
                        for el in self.dag.matrices[id].elements() {
                            self.classes[*el].child_of.push(next_id);
                        }
                        op
                    }
                }
            }
            Operand::Operation(o) => {
                match self.dag.operations.insert_with(o, next_id) {
                    Insertion::Existed(existing) => return self.find(existing),
                    Insertion::Added(id) => {
                        let op = Operand::Operation(id);
                        o.for_each_operand(|c| {
                            self.classes[*c].child_of.push(next_id)
                        });
                        op
                    }
                }
            }
            Operand::Wildcard(_) => unreachable!(),
        };

        let qty = self.evaluate(op);

        let class = EquivalencyClass {
            operands_by_kind: array::from_fn(|i| {
                if kind.id() == i { vec![op] } else { vec![] }
            }),
            operands: vec![op],
            size: 1,
            child_of: vec![],
            parent: Cell::new(next_id),
            analysis: Analysis {
                domain: self.domain(op),
                shape: self.shape(op),
                qty: qty.clone(),
            },
        };

        self.matchlist.push(next_id);
        self.classes.push(class);

        if let Some(q) = qty {
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
                    Operation::Min([a, b]) => a?.min(&b?),
                    Operation::Max([a, b]) => a?.max(&b?),
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
                let subbed = to.build(self, &sub);

                if self.union(id, subbed).is_some() {
                    self.updatelist.push(id);

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
        // let mut updatelist = mem::take(&mut self.updatelist);

        loop {
            self.dirty = false;

            let mut changed = Vec::new();

            // Step 1: move over all the operands in every class to its parent class
            for i in 0..self.classes.len() {
                let id = ClassId(i as Size);
                let parent = self.find(id);

                if id != parent {
                    let class = &mut self.classes[id];

                    if !class.operands.is_empty() || !class.child_of.is_empty()
                    {
                        let mut operands = mem::take(&mut class.operands);
                        let mut child_of = mem::take(&mut class.child_of);
                        let mut operands_by_kind = class
                            .operands_by_kind
                            .each_mut()
                            .map(|v| mem::take(v));
                        let analysis = class.analysis.clone();

                        let parent_class = &mut self.classes[parent];
                        parent_class.operands.append(&mut operands);
                        parent_class.child_of.append(&mut child_of);

                        if parent_class.analysis.merge_with(analysis) {
                            self.updatelist
                                .extend(parent_class.child_of.iter().copied());
                        }

                        for (i, mut ids) in
                            operands_by_kind.iter_mut().enumerate()
                        {
                            parent_class.operands_by_kind[i].append(&mut ids);
                        }

                        changed.push(parent);
                    }
                }
            }

            changed.sort_unstable();
            changed.dedup();

            // Step 2: dedup merged roots & append qty leaves
            for id in changed {
                let class = &mut self.classes[id];

                class.operands.sort_unstable();
                class.operands.dedup();
                class.size = class.operands.len() as Size;

                class.child_of.sort_unstable();
                class.child_of.dedup();

                for entry in &mut class.operands_by_kind {
                    entry.sort_unstable();
                    entry.dedup();
                }

                if let Some(ref qty) = class.analysis.qty {
                    let qty = qty.clone();
                    let qty_class = self.add(qty.into());
                    self.union(id, qty_class);
                }
            }

            // Step 3: canonicalize operation & matrix ids
            // in this step we also merge classes if any operations in them canonicalize to the same thing

            for i in 0..self.classes.len() {
                let id = ClassId(i as u16);
                let parent = self.find(id);

                if parent != id {
                    continue;
                }

                let mut operands = {
                    let class = &mut self.classes[parent];

                    mem::take(&mut class.operands)
                };

                operands.retain_mut(|op| {
                    let (is_canonical, other) = match op {
                        Operand::Leaf(_) => return true,

                        Operand::Matrix(m) => {
                            let canon =
                                self.dag.matrices[*m].map(|x| self.find(*x));
                            let is_canonical = canon == self.dag.matrices[*m];

                            match self
                                .dag
                                .matrices
                                .insert_with(canon, self.find(id))
                            {
                                Insertion::Existed(other) => {
                                    (is_canonical, other)
                                }
                                Insertion::Added(idx) => {
                                    *m = idx;
                                    return true;
                                }
                            }
                        }

                        Operand::Operation(o) => {
                            let canon = self.dag.operations[*o]
                                .map(|x| self.find(*x), |c| *c);
                            let is_canonical = canon == self.dag.operations[*o];

                            match self
                                .dag
                                .operations
                                .insert_with(canon, self.find(id))
                            {
                                Insertion::Existed(other) => {
                                    (is_canonical, other)
                                }
                                Insertion::Added(idx) => {
                                    *o = idx;
                                    return true;
                                }
                            }
                        }

                        Operand::Wildcard(_) => unreachable!(),
                    };

                    if let Some(kept) = self.union(other, parent) {
                        self.updatelist.push(kept);
                    }

                    is_canonical
                });

                operands.sort_unstable();
                operands.dedup();

                let mut operands_by_kind = array::repeat(Vec::new());

                for op in &operands {
                    operands_by_kind[self.fetch(*op).kind().id()].push(*op)
                }

                let class = &mut self.classes[parent];
                class.size = operands.len() as Size;
                class.operands.append(&mut operands);
                class.operands_by_kind = operands_by_kind;
            }

            // Step 4: update analysis-es

            while let Some(class) = self.updatelist.pop() {
                let parent = self.find(class);

                let Some(new) = Analysis::update(parent, self) else {
                    continue;
                };
                let qty = new.qty.clone();
                self.classes[parent].analysis = new;

                if let Some(qty) = qty {
                    let qty_class = self.add(qty.into());
                    self.union(parent, qty_class);
                }

                self.updatelist
                    .extend(self.classes[parent].child_of.iter().copied());
            }

            if !self.dirty {
                break;
            }
        }

        //  update matchlist recursively to include parents of changed classes
        let mut new_matchlist = HashSet::new();
        fn queue_parents(
            into: &mut HashSet<ClassId>,
            of: ClassId,
            graph: &EquivalencyGraph,
        ) {
            let class = &graph.classes[of];

            for parent in &class.child_of {
                let parent = graph.find(*parent);
                if into.insert(parent) {
                    queue_parents(into, parent, graph);
                }
            }
        }

        for pending in mem::take(&mut self.matchlist) {
            let pending = self.find(pending);
            if new_matchlist.insert(pending) {
                queue_parents(&mut new_matchlist, pending, &*self);
            }
        }

        self.matchlist.extend(new_matchlist.into_iter());
    }

    fn find(&self, id: ClassId) -> ClassId {
        let class = &self.classes[id];

        if class.parent.get() != id {
            let up = self.find(class.parent.get());
            self.classes[id].parent.set(up);
            up
        } else {
            id
        }
    }

    /// Merges a and b together, moving the smallest into the largest class.
    /// Returns the class which was moved into by the other.
    fn union(&mut self, a: ClassId, b: ClassId) -> Option<ClassId> {
        let root_a = self.find(a);
        let root_b = self.find(b);

        if root_a != root_b {
            assert_eq!(
                self.classes[root_a].analysis.shape,
                self.classes[root_b].analysis.shape,
                "Cannot union two classes of different shapes"
            );
            self.dirty = true;

            // Merge smallest into the largest
            let (kept, moved) =
                if self.classes[root_a].size >= self.classes[root_b].size {
                    (root_a, root_b)
                } else {
                    (root_b, root_a)
                };

            self.classes[moved].parent.set(kept);
            self.matchlist.push(kept);

            Some(kept)
        } else {
            None
        }
    }

    pub(crate) fn extract(&self) -> Expr {
        todo!()
    }

    pub(crate) fn new(expr: &Expr) -> Self {
        let mut graph = Self::default();

        let root = expr.fold(
            &mut graph,
            |graph, _, o| {
                graph.add(match o {
                    Operand::Leaf(l) => Operand::Leaf(l.clone()),
                    Operand::Matrix(m) => Operand::Matrix(m.map(|el| **el)),
                    Operand::Operation(o) => {
                        Operand::Operation(o.map(|o| **o, |c| **c))
                    }
                    Operand::Wildcard(_) => unreachable!(),
                })
            },
            |graph, _, c| graph.dag.conditions.push(c.map(|o| **o, |c| **c)),
        );
        graph.dag.root = Some(root);
        graph.rebuild();

        graph
    }

    fn domain(&self, op: Operand) -> Domain {
        match op {
            Operand::Leaf(l) => {
                FoldedOperand::<Domain, ()>::Leaf(&self.dag.leaves[l]).domain()
            }
            Operand::Matrix(m) => FoldedOperand::<Domain, ()>::Matrix(
                self.dag.matrices[m]
                    .map(|el| &self.classes[*el].analysis.domain),
            )
            .domain(),
            Operand::Operation(op) => FoldedOperand::<Domain, ()>::Operation(
                self.dag.operations[op]
                    .map(|o| &self.classes[*o].analysis.domain, |_| &()),
            )
            .domain(),
            Operand::Wildcard(_) => unreachable!(),
        }
    }

    fn shape(&self, op: Operand) -> Shape {
        match op {
            Operand::Leaf(l) => self.dag.leaves[l].shape(),
            Operand::Matrix(m) => self.dag.matrices[m].shape(),
            Operand::Operation(op) => FoldedOperand::<Shape, ()>::Operation(
                self.dag.operations[op]
                    .map(|o| &self.classes[*o].analysis.shape, |_| &()),
            )
            .shape(),
            Operand::Wildcard(_) => unreachable!(),
        }
    }
}

impl FetchedOperand<'_> {
    fn cost(&self) -> usize {
        match self {
            Operand::Leaf(leaf) => match leaf {
                Leaf::Symbol(symbol) => 1,
                Leaf::Constant(constant) => 1,
                Leaf::Quantity(quantity) => 1,
            },
            Operand::Operation(op) => match op {
                Operation::Add(_)
                | Operation::Mul(_)
                | Operation::Min(_)
                | Operation::Max(_) => 2,
                Operation::Sin(_)
                | Operation::Cos(_)
                | Operation::Tan(_)
                | Operation::Asin(_)
                | Operation::Acos(_)
                | Operation::Atan(_)
                | Operation::Sinh(_)
                | Operation::Cosh(_)
                | Operation::Tanh(_)
                | Operation::Asinh(_)
                | Operation::Acosh(_)
                | Operation::Atanh(_) => 3,
                Operation::Atan2 { .. }
                | Operation::Arg(_)
                | Operation::Norm(_) => 4,
                Operation::Conj(_)
                | Operation::Sign(_)
                | Operation::Real(_)
                | Operation::Imag(_) => 2,
                Operation::Pow { .. } => 5,
                Operation::Log { .. } => 5,
                Operation::Transpose(_) => todo!(),
                Operation::Det(_) => todo!(),
                Operation::Rank(_) => todo!(),
                Operation::Trace(_) => todo!(),
                Operation::Conditional { .. } => todo!(),
            },
            Operand::Matrix(_) => todo!(),
            Operand::Wildcard(_) => unreachable!(),
        }
    }
}
