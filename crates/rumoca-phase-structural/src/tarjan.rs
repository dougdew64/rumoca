//! Tarjan's algorithm for strongly connected components.

use crate::live_trace::LiveTrace;

/// One step of Tarjan's SCC algorithm, recorded for animation replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TarjanStep {
    /// Visiting node `v` for the first time (assigned index/lowlink).
    Visit(usize),
    /// Exploring edge v → w.
    ExploreEdge { from: usize, to: usize },
    /// Edge v → w leads to an unvisited node — recursing.
    TreeEdge { from: usize, to: usize },
    /// Edge v → w leads to an on-stack node — back edge, lowlink updated.
    BackEdge { from: usize, to: usize },
    /// Returning from recursion into `to`, updating `from`'s lowlink.
    Return { from: usize, to: usize },
    /// SCC found rooted at `root`, containing `members`.
    SccFound { root: usize, members: Vec<usize> },
}

/// Snapshot of Tarjan state at each step, for animation rendering.
#[derive(Debug, Clone)]
pub struct TarjanFrame {
    pub step: TarjanStep,
    /// Current DFS stack (nodes on the Tarjan stack).
    pub stack: Vec<usize>,
    /// SCCs discovered so far.
    pub sccs_so_far: Vec<Vec<usize>>,
}

/// Result of `tarjan_scc_with_trace`: the SCCs plus the frame sequence.
pub struct TarjanTraceResult {
    pub sccs: Vec<Vec<usize>>,
    pub frames: Vec<TarjanFrame>,
}

/// Like `tarjan_scc`, but records every algorithmic step for animation.
///
/// When `live` is `Some`, each frame is also pushed to the shared
/// [`LiveTrace`] buffer for live debugger stepping.
pub fn tarjan_scc_with_trace(
    n: usize,
    adj: &[Vec<usize>],
    live: Option<&LiveTrace<TarjanFrame>>,
) -> TarjanTraceResult {
    let mut state = TracedTarjanState::new(n, live.cloned());
    for v in 0..n {
        if state.index[v].is_none() {
            state.strongconnect(v, adj);
        }
    }
    TarjanTraceResult {
        sccs: state.sccs,
        frames: state.frames,
    }
}

/// Find all strongly connected components using Tarjan's algorithm.
///
/// Returns SCCs in reverse topological order. Each SCC is a `Vec` of node indices.
pub(crate) fn tarjan_scc(n: usize, adj: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut state = TarjanState::new(n);
    for v in 0..n {
        if state.index[v].is_none() {
            state.strongconnect(v, adj);
        }
    }
    state.sccs
}

struct TarjanState {
    index_counter: usize,
    stack: Vec<usize>,
    on_stack: Vec<bool>,
    index: Vec<Option<usize>>,
    lowlink: Vec<usize>,
    sccs: Vec<Vec<usize>>,
}

impl TarjanState {
    fn new(n: usize) -> Self {
        Self {
            index_counter: 0,
            stack: Vec::new(),
            on_stack: vec![false; n],
            index: vec![None; n],
            lowlink: vec![0; n],
            sccs: Vec::new(),
        }
    }

    fn strongconnect(&mut self, v: usize, adj: &[Vec<usize>]) {
        self.index[v] = Some(self.index_counter);
        self.lowlink[v] = self.index_counter;
        self.index_counter += 1;
        self.stack.push(v);
        self.on_stack[v] = true;

        for &w in &adj[v] {
            if self.index[w].is_none() {
                self.strongconnect(w, adj);
                self.lowlink[v] = self.lowlink[v].min(self.lowlink[w]);
            } else if self.on_stack[w] {
                self.lowlink[v] = self.lowlink[v]
                    .min(self.index[w].expect("tarjan invariant: on-stack node must have index"));
            }
        }

        if self.lowlink[v] == self.index[v].expect("tarjan invariant: visited node must have index")
        {
            self.pop_scc(v);
        }
    }

    fn pop_scc(&mut self, root: usize) {
        let mut scc = Vec::new();
        loop {
            let w = self
                .stack
                .pop()
                .expect("tarjan invariant: stack must contain SCC root");
            self.on_stack[w] = false;
            scc.push(w);
            if w == root {
                break;
            }
        }
        self.sccs.push(scc);
    }
}

struct TracedTarjanState {
    index_counter: usize,
    stack: Vec<usize>,
    on_stack: Vec<bool>,
    index: Vec<Option<usize>>,
    lowlink: Vec<usize>,
    sccs: Vec<Vec<usize>>,
    frames: Vec<TarjanFrame>,
    live: Option<LiveTrace<TarjanFrame>>,
}

impl TracedTarjanState {
    fn new(n: usize, live: Option<LiveTrace<TarjanFrame>>) -> Self {
        Self {
            index_counter: 0,
            stack: Vec::new(),
            on_stack: vec![false; n],
            index: vec![None; n],
            lowlink: vec![0; n],
            sccs: Vec::new(),
            frames: Vec::new(),
            live,
        }
    }

    fn record(&mut self, step: TarjanStep) {
        let frame = TarjanFrame {
            step,
            stack: self.stack.clone(),
            sccs_so_far: self.sccs.clone(),
        };
        if let Some(lt) = &self.live {
            lt.push(frame.clone());
        }
        self.frames.push(frame);
    }

    fn strongconnect(&mut self, v: usize, adj: &[Vec<usize>]) {
        self.index[v] = Some(self.index_counter);
        self.lowlink[v] = self.index_counter;
        self.index_counter += 1;
        self.stack.push(v);
        self.on_stack[v] = true;
        self.record(TarjanStep::Visit(v));

        for &w in &adj[v] {
            self.record(TarjanStep::ExploreEdge { from: v, to: w });
            if self.index[w].is_none() {
                self.record(TarjanStep::TreeEdge { from: v, to: w });
                self.strongconnect(w, adj);
                self.lowlink[v] = self.lowlink[v].min(self.lowlink[w]);
                self.record(TarjanStep::Return { from: v, to: w });
            } else if self.on_stack[w] {
                self.lowlink[v] = self.lowlink[v]
                    .min(self.index[w].expect("tarjan invariant: on-stack node must have index"));
                self.record(TarjanStep::BackEdge { from: v, to: w });
            }
        }

        if self.lowlink[v] == self.index[v].expect("tarjan invariant: visited node must have index")
        {
            let mut scc = Vec::new();
            loop {
                let w = self.stack.pop().expect("tarjan invariant: stack must contain SCC root");
                self.on_stack[w] = false;
                scc.push(w);
                if w == v {
                    break;
                }
            }
            self.record(TarjanStep::SccFound { root: v, members: scc.clone() });
            self.sccs.push(scc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tarjan_no_loops() {
        let adj = vec![vec![1], vec![2], vec![]];
        let sccs = tarjan_scc(3, &adj);
        assert!(
            sccs.iter().all(|scc| scc.len() == 1),
            "no cycles means all SCCs are singletons"
        );
    }

    #[test]
    fn test_tarjan_single_loop() {
        let adj = vec![vec![1], vec![2], vec![0]];
        let sccs = tarjan_scc(3, &adj);
        let loops: Vec<_> = sccs.iter().filter(|scc| scc.len() > 1).collect();
        assert_eq!(loops.len(), 1, "should find one loop");
        assert_eq!(loops[0].len(), 3, "loop should contain all 3 nodes");
    }

    #[test]
    fn test_tarjan_two_loops() {
        let adj = vec![vec![1], vec![0], vec![3], vec![2]];
        let sccs = tarjan_scc(4, &adj);
        let loops: Vec<_> = sccs.iter().filter(|scc| scc.len() > 1).collect();
        assert_eq!(loops.len(), 2, "should find two loops");
    }

    #[test]
    fn traced_tarjan_produces_same_sccs() {
        let adj = vec![vec![1], vec![2], vec![0]];
        let sccs = tarjan_scc(3, &adj);
        let traced = tarjan_scc_with_trace(3, &adj, None);
        assert_eq!(sccs, traced.sccs);
    }

    #[test]
    fn trace_contains_visit_for_each_node() {
        let adj = vec![vec![1], vec![], vec![]];
        let traced = tarjan_scc_with_trace(3, &adj, None);
        let visits: Vec<_> = traced.frames.iter()
            .filter(|f| matches!(f.step, TarjanStep::Visit(_)))
            .collect();
        assert_eq!(visits.len(), 3);
    }

    #[test]
    fn trace_records_scc_found() {
        let adj = vec![vec![1], vec![0]];
        let traced = tarjan_scc_with_trace(2, &adj, None);
        let scc_events: Vec<_> = traced.frames.iter()
            .filter(|f| matches!(f.step, TarjanStep::SccFound { .. }))
            .collect();
        assert_eq!(scc_events.len(), 1, "one SCC with both nodes");
    }

    #[test]
    fn trace_records_back_edge() {
        let adj = vec![vec![1], vec![0]];
        let traced = tarjan_scc_with_trace(2, &adj, None);
        let back_edges: Vec<_> = traced.frames.iter()
            .filter(|f| matches!(f.step, TarjanStep::BackEdge { .. }))
            .collect();
        assert!(!back_edges.is_empty(), "cycle should produce a back edge");
    }

    #[test]
    fn live_trace_receives_same_frames_as_returned() {
        let adj = vec![vec![1], vec![2], vec![0]];
        let lt = LiveTrace::new();
        let traced = tarjan_scc_with_trace(3, &adj, Some(&lt));
        let live_frames = lt.snapshot();
        assert_eq!(traced.frames.len(), live_frames.len());
        for (i, (ret, live)) in traced.frames.iter().zip(live_frames.iter()).enumerate() {
            assert_eq!(ret.step, live.step, "frame {i} step mismatch");
            assert_eq!(ret.stack, live.stack, "frame {i} stack mismatch");
        }
    }
}
