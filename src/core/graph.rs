#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BipartiteGraph {
    left_adj: Vec<Vec<RightNode>>,
    right_adj: Vec<Vec<LeftNode>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LeftNode(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RightNode(usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matching {
    left_to_right: Vec<Option<RightNode>>,
    right_to_left: Vec<Option<LeftNode>>,
    size: usize,
}

impl Matching {
    pub fn new(left_count: usize, right_count: usize) -> Self {
        Self {
            left_to_right: vec![None; left_count],
            right_to_left: vec![None; right_count],
            size: 0,
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    pub fn left_count(&self) -> usize {
        self.left_to_right.len()
    }

    pub fn right_count(&self) -> usize {
        self.right_to_left.len()
    }

    /// Number of unmatched left nodes.
    pub fn unmatched_left_count(&self) -> usize {
        self.left_count() - self.size
    }

    /// Number of unmatched right nodes.
    pub fn unmatched_right_count(&self) -> usize {
        self.right_count() - self.size
    }

    /// Returns true if every left node is matched.
    pub fn covers_left(&self) -> bool {
        self.size == self.left_count()
    }

    /// Returns true if every right node is matched.
    pub fn covers_right(&self) -> bool {
        self.size == self.right_count()
    }

    /// A matching is perfect iff every node on both sides is matched.
    pub fn is_perfect(&self) -> bool {
        self.left_count() == self.right_count()
            && self.size == self.left_count()
    }

    /// Returns the right node matched to `left`.
    pub fn right(&self, left: LeftNode) -> Option<RightNode> {
        self.left_to_right[left.index()]
    }

    /// Returns the left node matched to `right`.
    pub fn left(&self, right: RightNode) -> Option<LeftNode> {
        self.right_to_left[right.index()]
    }

    pub fn is_matched_left(&self, left: LeftNode) -> bool {
        self.right(left).is_some()
    }

    pub fn is_matched_right(&self, right: RightNode) -> bool {
        self.left(right).is_some()
    }

    pub fn is_unmatched_left(&self, left: LeftNode) -> bool {
        self.right(left).is_none()
    }

    pub fn is_unmatched_right(&self, right: RightNode) -> bool {
        self.left(right).is_none()
    }

    pub fn unmatched_left(&self) -> impl Iterator<Item = LeftNode> + '_ {
        self.left_to_right.iter().enumerate().filter_map(|(i, right)| {
            right.is_none().then_some(LeftNode::new(i))
        })
    }

    pub fn unmatched_right(&self) -> impl Iterator<Item = RightNode> + '_ {
        self.right_to_left
            .iter()
            .enumerate()
            .filter_map(|(i, left)| left.is_none().then_some(RightNode::new(i)))
    }

    pub fn edges(&self) -> impl Iterator<Item = (LeftNode, RightNode)> + '_ {
        self.left_to_right.iter().enumerate().filter_map(|(i, right)| {
            right.map(|right| (LeftNode::new(i), right))
        })
    }

    pub fn contains_left(&self, left: LeftNode) -> bool {
        self.is_matched_left(left)
    }

    pub fn contains_right(&self, right: RightNode) -> bool {
        self.is_matched_right(right)
    }

    pub(crate) fn match_pair(&mut self, left: LeftNode, right: RightNode) {
        debug_assert!(left.index() < self.left_count());
        debug_assert!(right.index() < self.right_count());
        debug_assert!(self.left(right).is_none());
        debug_assert!(self.right(left).is_none());

        self.left_to_right[left.index()] = Some(right);
        self.right_to_left[right.index()] = Some(left);
        self.size += 1;
    }
}

impl LeftNode {
    pub const fn new(id: usize) -> Self {
        Self(id)
    }

    pub const fn index(self) -> usize {
        self.0
    }
}

impl RightNode {
    pub const fn new(id: usize) -> Self {
        Self(id)
    }

    pub const fn index(self) -> usize {
        self.0
    }
}

impl BipartiteGraph {
    pub fn new() -> Self {
        Self { left_adj: Vec::new(), right_adj: Vec::new() }
    }

    pub fn with_capacity(left: usize, right: usize) -> Self {
        Self {
            left_adj: Vec::with_capacity(left),
            right_adj: Vec::with_capacity(right),
        }
    }

    pub fn add_left(&mut self) -> LeftNode {
        let node = LeftNode::new(self.left_adj.len());
        self.left_adj.push(Vec::new());
        node
    }

    pub fn add_right(&mut self) -> RightNode {
        let node = RightNode::new(self.right_adj.len());
        self.right_adj.push(Vec::new());
        node
    }

    pub fn add_edge(&mut self, left: LeftNode, right: RightNode) {
        debug_assert!(left.index() < self.left_adj.len());
        debug_assert!(right.index() < self.right_adj.len());

        if !self.left_adj[left.index()].contains(&right) {
            self.left_adj[left.index()].push(right);
            self.right_adj[right.index()].push(left);
        }
    }

    pub fn left_neighbors(&self, left: LeftNode) -> &[RightNode] {
        &self.left_adj[left.index()]
    }

    pub fn right_neighbors(&self, right: RightNode) -> &[LeftNode] {
        &self.right_adj[right.index()]
    }

    pub fn left_degree(&self, left: LeftNode) -> usize {
        self.left_adj[left.index()].len()
    }

    pub fn right_degree(&self, right: RightNode) -> usize {
        self.right_adj[right.index()].len()
    }

    pub fn left_count(&self) -> usize {
        self.left_adj.len()
    }

    pub fn right_count(&self) -> usize {
        self.right_adj.len()
    }

    pub fn edge_count(&self) -> usize {
        self.left_adj.iter().map(Vec::len).sum()
    }

    pub fn left_nodes(&self) -> impl Iterator<Item = LeftNode> {
        (0..self.left_adj.len()).map(LeftNode::new)
    }

    pub fn right_nodes(&self) -> impl Iterator<Item = RightNode> {
        (0..self.right_adj.len()).map(RightNode::new)
    }

    pub fn has_edge(&self, left: LeftNode, right: RightNode) -> bool {
        self.left_adj[left.index()].contains(&right)
    }
}

impl Default for BipartiteGraph {
    fn default() -> Self {
        Self::new()
    }
}

// START OF LLM CODED SECTION

use std::collections::VecDeque;

impl BipartiteGraph {
    // -------------------------------------------------------------------------
    // Generic BFS
    // -------------------------------------------------------------------------

    /// Breadth-first traversal from a left node.
    ///
    /// Visits every reachable edge exactly once.
    pub fn bfs<F>(&self, start: LeftNode, mut visit: F)
    where
        F: FnMut(LeftNode, RightNode), {
        let mut visited_left = vec![false; self.left_count()];
        let mut visited_right = vec![false; self.right_count()];
        let mut queue = VecDeque::new();

        visited_left[start.index()] = true;
        queue.push_back(start);

        while let Some(left) = queue.pop_front() {
            for &right in self.left_neighbors(left) {
                if visited_right[right.index()] {
                    continue;
                }

                visited_right[right.index()] = true;
                visit(left, right);

                for &next_left in self.right_neighbors(right) {
                    if !visited_left[next_left.index()] {
                        visited_left[next_left.index()] = true;
                        queue.push_back(next_left);
                    }
                }
            }
        }
    }

    // -------------------------------------------------------------------------
    // Generic DFS
    // -------------------------------------------------------------------------

    /// Depth-first traversal from a left node.
    ///
    /// Visits every reachable edge exactly once.
    pub fn dfs<F>(&self, start: LeftNode, mut visit: F)
    where
        F: FnMut(LeftNode, RightNode), {
        let mut visited_left = vec![false; self.left_count()];
        let mut visited_right = vec![false; self.right_count()];

        self.dfs_inner(
            start,
            &mut visited_left,
            &mut visited_right,
            &mut visit,
        );
    }

    fn dfs_inner<F>(
        &self,
        left: LeftNode,
        visited_left: &mut [bool],
        visited_right: &mut [bool],
        visit: &mut F,
    ) where
        F: FnMut(LeftNode, RightNode), {
        if visited_left[left.index()] {
            return;
        }

        visited_left[left.index()] = true;

        for &right in self.left_neighbors(left) {
            if visited_right[right.index()] {
                continue;
            }

            visited_right[right.index()] = true;
            visit(left, right);

            for &next_left in self.right_neighbors(right) {
                if !visited_left[next_left.index()] {
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

    // -------------------------------------------------------------------------
    // Hopcroft-Karp
    // -------------------------------------------------------------------------

    /// Computes a maximum-cardinality matching using Hopcroft-Karp.
    ///
    /// Complexity: O(E * sqrt(V))
    pub fn maximum_matching(&self) -> Matching {
        let n_left = self.left_count();
        let n_right = self.right_count();

        let mut pair_left = vec![None; n_left];
        let mut pair_right = vec![None; n_right];

        let mut distance = vec![usize::MAX; n_left];

        while Self::hopcroft_karp_bfs(
            self,
            &pair_left,
            &pair_right,
            &mut distance,
        ) {
            for left in self.left_nodes() {
                if pair_left[left.index()].is_none() {
                    Self::hopcroft_karp_dfs(
                        self,
                        left,
                        &mut pair_left,
                        &mut pair_right,
                        &mut distance,
                    );
                }
            }
        }

        let mut matching = Matching::new(n_left, n_right);

        for left in self.left_nodes() {
            if let Some(right) = pair_left[left.index()] {
                matching.match_pair(left, right);
            }
        }

        matching
    }

    /// BFS phase of Hopcroft-Karp.
    ///
    /// Builds the layered graph containing shortest augmenting paths.
    fn hopcroft_karp_bfs(
        &self,
        pair_left: &[Option<RightNode>],
        pair_right: &[Option<LeftNode>],
        distance: &mut [usize],
    ) -> bool {
        let mut queue = VecDeque::new();

        // Every currently-unmatched left node is a possible
        // starting point of an augmenting path.
        for left in self.left_nodes() {
            if pair_left[left.index()].is_none() {
                distance[left.index()] = 0;
                queue.push_back(left);
            } else {
                distance[left.index()] = usize::MAX;
            }
        }

        let mut found_augmenting_path = false;

        while let Some(left) = queue.pop_front() {
            let current_distance = distance[left.index()];

            for &right in self.left_neighbors(left) {
                match pair_right[right.index()] {
                    None => {
                        // We found a free right node.
                        found_augmenting_path = true;
                    }

                    Some(next_left) => {
                        // Follow the matched edge right -> left.
                        if distance[next_left.index()] == usize::MAX {
                            distance[next_left.index()] = current_distance + 1;

                            queue.push_back(next_left);
                        }
                    }
                }
            }
        }

        found_augmenting_path
    }

    /// DFS phase of Hopcroft-Karp.
    ///
    /// Searches for an augmenting path in the layered graph created by BFS.
    fn hopcroft_karp_dfs(
        &self,
        left: LeftNode,
        pair_left: &mut [Option<RightNode>],
        pair_right: &mut [Option<LeftNode>],
        distance: &mut [usize],
    ) -> bool {
        for &right in self.left_neighbors(left) {
            match pair_right[right.index()] {
                // Free right node => augmenting path found.
                None => {
                    pair_left[left.index()] = Some(right);
                    pair_right[right.index()] = Some(left);
                    return true;
                }

                // Right is already matched. Follow its matched edge
                // only if it belongs to the next BFS layer.
                Some(next_left)
                    if distance[next_left.index()]
                        == distance[left.index()] + 1 =>
                {
                    if Self::hopcroft_karp_dfs(
                        self, next_left, pair_left, pair_right, distance,
                    ) {
                        pair_left[left.index()] = Some(right);
                        pair_right[right.index()] = Some(left);
                        return true;
                    }
                }

                _ => {}
            }
        }

        // No augmenting path exists through this node in the
        // current layered graph.
        distance[left.index()] = usize::MAX;

        false
    }
}
