use std::{
    collections::{HashMap, HashSet, VecDeque},
    hash::Hash,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BipartiteGraph<L: Hash + Eq, R: Hash + Eq> {
    left_adj: HashMap<L, HashSet<R>>,
    right_adj: HashMap<R, HashSet<L>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matching<L: Hash + Eq, R: Hash + Eq> {
    left_to_right: HashMap<L, R>,
    right_to_left: HashMap<R, L>,
}

impl<L, R> Matching<L, R>
where
    L: Eq + Hash + Clone,
    R: Eq + Hash + Clone,
{
    pub fn new() -> Self {
        Self { left_to_right: HashMap::new(), right_to_left: HashMap::new() }
    }

    pub fn size(&self) -> usize {
        self.left_to_right.len()
    }

    pub fn is_empty(&self) -> bool {
        self.left_to_right.is_empty()
    }

    pub fn left_count(&self) -> usize {
        self.left_to_right.len()
    }

    pub fn right_count(&self) -> usize {
        self.right_to_left.len()
    }

    pub fn right(&self, left: &L) -> Option<&R> {
        self.left_to_right.get(left)
    }

    pub fn left(&self, right: &R) -> Option<&L> {
        self.right_to_left.get(right)
    }

    pub fn is_matched_left(&self, left: &L) -> bool {
        self.left_to_right.contains_key(left)
    }

    pub fn is_matched_right(&self, right: &R) -> bool {
        self.right_to_left.contains_key(right)
    }

    pub fn unmatched_left<'a>(
        &'a self,
        nodes: impl Iterator<Item = &'a L>,
    ) -> impl Iterator<Item = &'a L> {
        nodes.filter(|left| !self.left_to_right.contains_key(*left))
    }

    pub fn unmatched_right<'a>(
        &'a self,
        nodes: impl Iterator<Item = &'a R>,
    ) -> impl Iterator<Item = &'a R> {
        nodes.filter(|right| !self.right_to_left.contains_key(*right))
    }

    pub fn edges(&self) -> impl Iterator<Item = (&L, &R)> {
        self.left_to_right.iter()
    }

    pub(crate) fn match_pair(&mut self, left: L, right: R) {
        debug_assert!(!self.left_to_right.contains_key(&left));
        debug_assert!(!self.right_to_left.contains_key(&right));

        self.left_to_right.insert(left.clone(), right.clone());
        self.right_to_left.insert(right, left);
    }
}

impl<L, R> Default for Matching<L, R>
where
    L: Eq + Hash + Clone,
    R: Eq + Hash + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<L, R> BipartiteGraph<L, R>
where
    L: Eq + Hash + Clone,
    R: Eq + Hash + Clone,
{
    pub fn new() -> Self {
        Self { left_adj: HashMap::new(), right_adj: HashMap::new() }
    }

    pub fn add_left(&mut self, left: L) {
        self.left_adj.entry(left).or_default();
    }

    pub fn add_right(&mut self, right: R) {
        self.right_adj.entry(right).or_default();
    }

    pub fn add_edge(&mut self, left: L, right: R) {
        self.left_adj.entry(left.clone()).or_default().insert(right.clone());

        self.right_adj.entry(right).or_default().insert(left);
    }

    pub fn left_neighbors(&self, left: &L) -> Option<&HashSet<R>> {
        self.left_adj.get(left)
    }

    pub fn right_neighbors(&self, right: &R) -> Option<&HashSet<L>> {
        self.right_adj.get(right)
    }

    pub fn left_degree(&self, left: &L) -> usize {
        self.left_adj.get(left).map_or(0, HashSet::len)
    }

    pub fn right_degree(&self, right: &R) -> usize {
        self.right_adj.get(right).map_or(0, HashSet::len)
    }

    pub fn left_count(&self) -> usize {
        self.left_adj.len()
    }

    pub fn right_count(&self) -> usize {
        self.right_adj.len()
    }

    pub fn edge_count(&self) -> usize {
        self.left_adj.values().map(HashSet::len).sum()
    }

    pub fn left_nodes(&self) -> impl Iterator<Item = &L> {
        self.left_adj.keys()
    }

    pub fn right_nodes(&self) -> impl Iterator<Item = &R> {
        self.right_adj.keys()
    }

    pub fn has_edge(&self, left: &L, right: &R) -> bool {
        self.left_adj
            .get(left)
            .is_some_and(|neighbors| neighbors.contains(right))
    }

    pub fn bfs<F>(&self, start: &L, mut visit: F)
    where
        F: FnMut(&L, &R), {
        let mut visited_left = HashSet::new();
        let mut visited_right = HashSet::new();
        let mut queue = VecDeque::new();

        if !self.left_adj.contains_key(start) {
            return;
        }

        visited_left.insert(start.clone());
        queue.push_back(start.clone());

        while let Some(left) = queue.pop_front() {
            let Some(neighbors) = self.left_adj.get(&left) else {
                continue;
            };

            for right in neighbors {
                if !visited_right.insert(right.clone()) {
                    continue;
                }

                visit(&left, right);

                if let Some(next_lefts) = self.right_adj.get(right) {
                    for next_left in next_lefts {
                        if visited_left.insert(next_left.clone()) {
                            queue.push_back(next_left.clone());
                        }
                    }
                }
            }
        }
    }

    pub fn dfs<F>(&self, start: &L, mut visit: F)
    where
        F: FnMut(&L, &R), {
        let mut visited_left = HashSet::new();
        let mut visited_right = HashSet::new();

        self.dfs_inner(
            start,
            &mut visited_left,
            &mut visited_right,
            &mut visit,
        );
    }

    fn dfs_inner<F>(
        &self,
        left: &L,
        visited_left: &mut HashSet<L>,
        visited_right: &mut HashSet<R>,
        visit: &mut F,
    ) where
        F: FnMut(&L, &R), {
        if !visited_left.insert(left.clone()) {
            return;
        }

        let Some(neighbors) = self.left_adj.get(left) else {
            return;
        };

        for right in neighbors {
            if !visited_right.insert(right.clone()) {
                continue;
            }

            visit(left, right);

            if let Some(next_lefts) = self.right_adj.get(right) {
                for next_left in next_lefts {
                    self.dfs_inner(
                        next_left,
                        visited_left,
                        visited_right,
                        visit,
                    );
                }
            }
        }
    }

    pub fn maximum_matching(&self) -> Matching<L, R> {
        let mut pair_left: HashMap<L, R> = HashMap::new();
        let mut pair_right: HashMap<R, L> = HashMap::new();
        let mut distance: HashMap<L, usize> = HashMap::new();

        while self.hopcroft_karp_bfs(&pair_left, &pair_right, &mut distance) {
            for left in self.left_nodes() {
                if !pair_left.contains_key(left) {
                    self.hopcroft_karp_dfs(
                        left,
                        &mut pair_left,
                        &mut pair_right,
                        &mut distance,
                    );
                }
            }
        }

        Matching { left_to_right: pair_left, right_to_left: pair_right }
    }

    fn hopcroft_karp_bfs(
        &self,
        pair_left: &HashMap<L, R>,
        pair_right: &HashMap<R, L>,
        distance: &mut HashMap<L, usize>,
    ) -> bool {
        let mut queue = VecDeque::new();

        distance.clear();

        for left in self.left_nodes() {
            if !pair_left.contains_key(left) {
                distance.insert(left.clone(), 0);
                queue.push_back(left.clone());
            }
        }

        let mut found = false;

        while let Some(left) = queue.pop_front() {
            let current_distance = distance[&left];

            let Some(neighbors) = self.left_adj.get(&left) else {
                continue;
            };

            for right in neighbors {
                match pair_right.get(right) {
                    None => {
                        found = true;
                    }

                    Some(next_left) => {
                        if !distance.contains_key(next_left) {
                            distance.insert(
                                next_left.clone(),
                                current_distance + 1,
                            );
                            queue.push_back(next_left.clone());
                        }
                    }
                }
            }
        }

        found
    }

    fn hopcroft_karp_dfs(
        &self,
        left: &L,
        pair_left: &mut HashMap<L, R>,
        pair_right: &mut HashMap<R, L>,
        distance: &mut HashMap<L, usize>,
    ) -> bool {
        let Some(neighbors) = self.left_adj.get(left) else {
            return false;
        };

        for right in neighbors {
            let Some(next_left) = pair_right.get(right).cloned() else {
                pair_left.insert(left.clone(), right.clone());
                pair_right.insert(right.clone(), left.clone());
                return true;
            };

            if distance.get(&next_left).copied()
                == distance.get(left).map(|d| d + 1)
            {
                if self.hopcroft_karp_dfs(
                    &next_left, pair_left, pair_right, distance,
                ) {
                    pair_left.insert(left.clone(), right.clone());
                    pair_right.insert(right.clone(), left.clone());
                    return true;
                }
            }
        }

        distance.remove(left);
        false
    }
}

impl<L, R> Default for BipartiteGraph<L, R>
where
    L: Eq + Hash + Clone,
    R: Eq + Hash + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct DirectedGraph<T> {
    edges: HashMap<T, HashSet<T>>,
}

impl<T> DirectedGraph<T>
where
    T: Eq + std::hash::Hash + Clone,
{
    pub fn new() -> Self {
        Self { edges: HashMap::new() }
    }

    pub fn add_node(&mut self, node: T) {
        self.edges.entry(node).or_default();
    }

    pub fn add_edge(&mut self, from: T, to: T) {
        self.edges.entry(from).or_default().insert(to.clone());
        self.edges.entry(to).or_default();
    }

    pub fn sccs(&self) -> Vec<Vec<T>> {
        struct State<T> {
            index: usize,
            indices: HashMap<T, usize>,
            lowlink: HashMap<T, usize>,
            stack: Vec<T>,
            on_stack: HashSet<T>,
            result: Vec<Vec<T>>,
        }

        fn dfs<T>(graph: &DirectedGraph<T>, v: T, state: &mut State<T>)
        where
            T: Eq + std::hash::Hash + Clone, {
            let index = state.index;
            state.index += 1;

            state.indices.insert(v.clone(), index);
            state.lowlink.insert(v.clone(), index);

            state.stack.push(v.clone());
            state.on_stack.insert(v.clone());

            for w in graph.edges[&v].iter().cloned() {
                if !state.indices.contains_key(&w) {
                    dfs(graph, w.clone(), state);

                    let low = state.lowlink[&v].min(state.lowlink[&w]);
                    state.lowlink.insert(v.clone(), low);
                } else if state.on_stack.contains(&w) {
                    let low = state.lowlink[&v].min(state.indices[&w]);
                    state.lowlink.insert(v.clone(), low);
                }
            }

            if state.lowlink[&v] == state.indices[&v] {
                let mut component = Vec::new();

                loop {
                    let w = state.stack.pop().unwrap();
                    state.on_stack.remove(&w);
                    component.push(w.clone());

                    if w == v {
                        break;
                    }
                }

                state.result.push(component);
            }
        }

        let mut state = State {
            index: 0,
            indices: HashMap::new(),
            lowlink: HashMap::new(),
            stack: Vec::new(),
            on_stack: HashSet::new(),
            result: Vec::new(),
        };

        for node in self.edges.keys().cloned() {
            if !state.indices.contains_key(&node) {
                dfs(self, node, &mut state);
            }
        }

        state.result
    }
}
