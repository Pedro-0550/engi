use std::{
    array,
    cell::Cell,
    collections::HashSet,
    hash::Hash,
    mem::{self, variant_count},
    ops::Index,
};

use ahash::{AHashMap, AHashSet};
use itertools::Itertools;
use kinded::Kinded;
use num::{Zero, complex::ComplexFloat, pow::Pow as _};

use crate::{
    core::{
        consed::Insertion::{self},
        indexed::{Indexed, Size},
    },
    expr::{
        Expr, FoldedOperand, Operand, OperandKind,
        dag::{
            Condition, ConditionId, Dag, Leaf, LeafId, LeafKind, Matrix,
            Operation, OperationKind,
        },
        domain::Domain,
        shape::Shape,
    },
    id,
    simplify::pattern::{
        Machine, Pattern, Program, Rule, WILDCARD_COUNT, Wildcard,
    },
    unit::Quantity,
};

/* --------------------------------- MODULES -------------------------------- */

pub mod pattern;
pub mod rules;

#[cfg(test)]
mod test;

/* --------------------------------- STRUCTS -------------------------------- */

id!(pub(crate) ClassId);
id!(pub(crate) MatchlistCursor);

#[derive(Hash, PartialEq, Eq, Clone)]
pub struct Substitution {
    bindings: Indexed<[Option<ClassId>; WILDCARD_COUNT], Wildcard>,
}

#[derive(Default)]
pub struct EquivalencyGraph {
    dag: EquivalencyDag,
    classes: Indexed<Vec<EquivalencyClass>, ClassId>,
    matchlist: Matchlist,
    updatelist: Vec<ClassId>,
    dirty: bool,
}

// Todo: convert to AoS
pub struct EquivalencyClass {
    // while this does use a bit more memory, its 12% faster over a hashmap,
    // even though a hashmap would only need to hash 2 bytes..
    // I guess its done so often that its worth it, at least thats what i got from profiling
    operands_by_kind: [Vec<Operand>; {
        variant_count::<LeafKind>() + variant_count::<OperationKind>() + 2
    }],
    operands: Vec<Operand>,
    child_of: Vec<ClassId>,
    size: Size,

    parent: Cell<ClassId>,
    analysis: Analysis,
}

#[derive(Default)]
pub struct Matchlist {
    entries: Vec<ClassId>,
    offset: Size,
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
            (Some(_a), Some(_b)) => {
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

impl Matchlist {
    fn push(&mut self, class: ClassId) {
        self.entries.push(class);
    }

    fn compact(&mut self, to: MatchlistCursor) {
        self.entries.drain(..(to.0 - self.offset) as usize);
        self.offset = to.0;
    }

    fn since(&self, start: MatchlistCursor) -> &[ClassId] {
        &self.entries[(start.0 - self.offset) as usize..]
    }

    fn end(&self) -> MatchlistCursor {
        MatchlistCursor(self.offset + self.entries.len() as u16)
    }

    fn start(&self) -> MatchlistCursor {
        MatchlistCursor(self.offset)
    }
}

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

        let kind = canon_op.kind();

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

        let qty = if kind != OperandKind::Leaf(LeafKind::Quantity) {
            self.evaluate(op)
        } else {
            None
        };

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

        self.matchlist.entries.push(next_id);
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
                    Operation::Conditional { cond: _, pass: _, fail: _ } => {
                        return None;
                    }
                },
            ),
            Operand::Matrix(_) => todo!(),
        }
    }

    pub fn rewrite(&mut self, rules: &[Rule]) {
        /// Ratio between a rule'ss matched and merged classes.
        /// If its larger than this value in any iteration it will be banned.
        const MATCH_MERGE_RATIO: usize = 2;
        /// Minimum number of matches a rule needs to have to qualify for scheduling
        const MIN_SCHEDULING_MATCHES: usize = 5;

        const BASE_BAN: usize = 10;

        const CLASS_LIMIT: usize = 5000;
        const MATCH_LIMIT: usize = 100;

        struct RuleState<'p> {
            program: Program,
            to: &'p Pattern,
            root_kind: OperandKind,

            cursor: MatchlistCursor,
            banned_for: usize,
            times_banned: usize,
            times_exceeded: usize,

            applied: AHashSet<(ClassId, Substitution)>,
            matched: usize,
            merged: usize,
        }

        // Compile

        let mut states = rules
            .iter()
            .map(|r| RuleState {
                program: r.from.compile(&r.conds, self),
                to: &r.to,
                root_kind: r
                    .from
                    .root()
                    .map(|op| r.from.fetch(op))
                    .unwrap()
                    .kind(),
                cursor: self.matchlist.start(),
                banned_for: 0,
                times_banned: 0,
                applied: AHashSet::new(),
                times_exceeded: 0,
                matched: 0,
                merged: 0,
            })
            .collect_vec();

        loop {
            let mut grouped_matches = Vec::new();

            let mut changed = false;
            let mut hit_class_limit = false;

            let end = self.matchlist.end();

            for (i, state) in states.iter_mut().enumerate() {
                if state.banned_for > 0 {
                    state.banned_for -= 1;
                    continue;
                }

                if state.matched > MIN_SCHEDULING_MATCHES
                    && state.matched > state.merged * MATCH_MERGE_RATIO
                {
                    state.banned_for =
                        BASE_BAN << state.times_banned.min(Size::BITS as usize);
                    state.times_banned += 1;
                    state.merged = 0;
                    state.matched = 0;
                    continue;
                }

                state.merged = 0;
                state.matched = 0;

                // Search

                let mut work = self
                    .matchlist
                    .since(state.cursor)
                    .iter()
                    .map(|c| self.find(*c))
                    .collect_vec();
                work.sort_unstable();
                work.dedup();

                let limit = MATCH_LIMIT
                    << state.times_exceeded.min(Size::BITS as usize);
                let mut exceeded = false;

                let mut matches = Vec::new();

                'search: for class_id in &work {
                    // why even bother matching if this class dosent contain any nodes of that kind
                    if self.classes[*class_id].operands_by_kind
                        [state.root_kind.id()]
                    .is_empty()
                    {
                        continue;
                    }

                    let mut subs = Vec::new();
                    Machine::execute(
                        self,
                        &state.program,
                        *class_id,
                        &mut subs,
                    );

                    for sub in subs {
                        if state.applied.contains(&(*class_id, sub.clone())) {
                            continue;
                        }

                        state.matched += 1;

                        matches.push((*class_id, sub));
                    }

                    if matches.len() > limit {
                        exceeded = true;
                        break 'search;
                    }
                }

                if exceeded {
                    state.banned_for =
                        BASE_BAN << state.times_banned.min(Size::BITS as usize);
                    state.times_banned += 1;
                    state.times_exceeded += 1;
                    state.matched = 0;
                    continue;
                }

                grouped_matches.push((i, matches));
            }

            // Apply
            'apply: for (i, matches) in grouped_matches {
                let state = &mut states[i];
                for (id, sub) in matches {
                    if self.classes.len() >= CLASS_LIMIT {
                        hit_class_limit = true;
                        break 'apply;
                    }
                    state.applied.insert((id, sub.clone()));

                    let subbed = state.to.build(self, &sub);

                    if self.union(id, subbed).is_some() {
                        self.updatelist.push(id);

                        state.merged += 1;
                        changed = true;
                    }
                }
                state.cursor = end
            }

            // Compact
            if let Some(min_cursor) = states.iter().map(|x| x.cursor).min() {
                self.matchlist.compact(min_cursor);
            }

            // Rebuild
            self.rebuild(end);
            if hit_class_limit {
                break;
            }

            // if !changed {
            //     if discarded {
            //         for s in &mut states {
            //             s.banned_for = 0;
            //             s.matched = 0;
            //             s.merged = 0;
            //         }
            //         continue;
            //     }

            //     if states.iter().any(|s| s.banned_for > 0) {
            //         states.iter_mut().for_each(|s| s.banned_for = 0);
            //         continue;
            //     }
            //     if states.iter().all(|s| s.cursor == self.matchlist.end()) {
            //         break;
            //     }
            //     break;
            // }
            if !changed {
                // if skipped {
                //     for s in &mut states {
                //         s.banned_for = 0;
                //         s.matched = 0;
                //         s.merged = 0; // otherwise the ratio check re-bans immediately
                //     }
                //     continue;
                // }
                break;
            }

            // println!("{} classes", self.classes.len(),);
        }
    }

    fn rebuild(&mut self, since: MatchlistCursor) {
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
                        let mut operands_by_kind =
                            class.operands_by_kind.each_mut().map(mem::take);
                        let analysis = class.analysis.clone();

                        let parent_class = &mut self.classes[parent];
                        parent_class.operands.append(&mut operands);
                        parent_class.child_of.append(&mut child_of);

                        if parent_class.analysis.merge_with(analysis) {
                            self.updatelist
                                .extend(parent_class.child_of.iter().copied());
                        }

                        for (i, ids) in operands_by_kind.iter_mut().enumerate()
                        {
                            parent_class.operands_by_kind[i].append(ids);
                        }

                        changed.push(parent);
                    }
                }
            }

            changed.sort_unstable();
            changed.dedup();

            // Step 2: dedup merged roots & append qty leaves
            for id in changed {
                let parent = self.find(id);
                let class = &mut self.classes[parent];

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
                    self.union(parent, qty_class);
                }
            }

            // Step 3: canonicalize operation & matrix ids
            // in this step we also merge classes if any operations in them canonicalize to the same thing

            for i in 0..self.classes.len() {
                let id = ClassId(i as Size);
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
        let mut touched: Vec<ClassId> =
            self.matchlist.since(since).iter().map(|c| self.find(*c)).collect();
        touched.sort_unstable();
        touched.dedup();

        let mut seen: HashSet<ClassId> = touched.iter().copied().collect();
        let mut stack = touched;
        while let Some(c) = stack.pop() {
            let parents: Vec<ClassId> = self.classes[c]
                .child_of
                .iter()
                .map(|p| self.find(*p))
                .collect();
            for p in parents {
                if seen.insert(p) {
                    self.matchlist.push(p);
                    stack.push(p);
                }
            }
        }
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
            self.matchlist.entries.push(kept);

            Some(kept)
        } else {
            None
        }
    }

    pub(crate) fn extract(&self) -> Expr {
        let mut best_ops = AHashMap::new();

        fn calculate_cost(
            graph: &EquivalencyGraph,
            op: Operand,
            best_ops: &AHashMap<ClassId, (usize, Operand)>,
        ) -> Option<usize> {
            let fetched = graph.fetch(op);

            let mut cost = Some(fetched.cost());
            match fetched {
                Operand::Matrix(m) => {
                    for el in m.elements() {
                        cost = cost.zip_with(best_ops.get(el), |a, b| a + b.0);
                    }
                }
                Operand::Operation(o) => o.for_each_operand(|op| {
                    cost = cost.zip_with(best_ops.get(op), |a, b| a + b.0)
                }),
                _ => (),
            }

            cost
        }

        loop {
            let mut changed = false;

            for (id, class) in self.classes.iter() {
                if self.find(id) != id {
                    continue;
                }

                for op in &class.operands {
                    if let Some(cost) = calculate_cost(self, *op, &best_ops) {
                        let best_cost = best_ops
                            .get(&id)
                            .map(|entry| entry.0)
                            .unwrap_or(usize::MAX);

                        if best_cost > cost {
                            best_ops.insert(id, (cost, *op));
                            changed = true
                        }
                    }
                }
            }

            if !changed {
                break;
            }
        }

        let mut expr = Expr::new();

        fn build_condition(
            graph: &EquivalencyGraph,
            cond: ConditionId,
            best_ops: &AHashMap<ClassId, (usize, Operand)>,
            into: &mut Expr,
        ) -> ConditionId {
            let cond = graph.dag.conditions[cond];
            let mapped = match cond {
                Condition::Eq(a, b) => Condition::Eq(
                    build_operand(graph, a, best_ops, into),
                    build_operand(graph, b, best_ops, into),
                ),
                Condition::Ne(a, b) => Condition::Ne(
                    build_operand(graph, a, best_ops, into),
                    build_operand(graph, b, best_ops, into),
                ),
                Condition::Lt(a, b) => Condition::Lt(
                    build_operand(graph, a, best_ops, into),
                    build_operand(graph, b, best_ops, into),
                ),
                Condition::Le(a, b) => Condition::Le(
                    build_operand(graph, a, best_ops, into),
                    build_operand(graph, b, best_ops, into),
                ),
                Condition::Gt(a, b) => Condition::Gt(
                    build_operand(graph, a, best_ops, into),
                    build_operand(graph, b, best_ops, into),
                ),
                Condition::Ge(a, b) => Condition::Ge(
                    build_operand(graph, a, best_ops, into),
                    build_operand(graph, b, best_ops, into),
                ),
                Condition::And(conds) => Condition::And(
                    conds.map(|x| build_condition(graph, x, best_ops, into)),
                ),
                Condition::Or(conds) => Condition::Or(
                    conds.map(|x| build_condition(graph, x, best_ops, into)),
                ),
                Condition::Not(x) => {
                    Condition::Not(build_condition(graph, x, best_ops, into))
                }
            };

            into.dag.conditions.push(mapped)
        }

        fn build_operand(
            graph: &EquivalencyGraph,
            class: ClassId,
            best_ops: &AHashMap<ClassId, (usize, Operand)>,
            into: &mut Expr,
        ) -> Operand {
            let op = best_ops[&graph.find(class)].1;
            let fetched = graph.fetch(op);
            let mapped = match fetched {
                Operand::Leaf(l) => Operand::Leaf(l.clone()),
                Operand::Matrix(m) => Operand::Matrix(
                    m.map(|el| build_operand(graph, *el, best_ops, into)),
                ),
                Operand::Operation(o) => Operand::Operation(o.map_with(
                    into,
                    |into, o| build_operand(graph, *o, best_ops, into),
                    |into, c| build_condition(graph, *c, best_ops, into),
                )),
                Operand::Wildcard(_) => unreachable!(),
            };
            into.push(mapped)
        }

        let root_id = build_operand(
            self,
            self.dag.root.expect("Cant go far without a root"),
            &best_ops,
            &mut expr,
        );
        expr.set_root(root_id);

        expr
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
        graph.rebuild(MatchlistCursor(0));

        graph
    }

    fn intern_leaf(&mut self, leaf: &Leaf) -> LeafId {
        let class = self.add(Operand::Leaf(leaf.clone()));
        let class = self.find(class);

        self.classes[class]
            .operands
            .iter()
            .find_map(|op| match op {
                Operand::Leaf(id) if self.dag.leaves[*id] == *leaf => Some(*id),
                _ => None,
            })
            .expect("interned leaf must be in its own class")
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
                Leaf::Symbol(_symbol) => 1,
                Leaf::Constant(_constant) => 1,
                Leaf::Quantity(_quantity) => 1,
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
