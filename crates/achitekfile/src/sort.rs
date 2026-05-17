//! Deterministic topological sorting for prompt dependency graphs.
//!
//! This module implements Kahn's algorithm for directed acyclic graphs (DAGs).
//! In this crate it is used to order prompts so that every prompt appears after
//! the prompts it depends on.
//!
//! Edges point from dependency to dependent:
//!
//! ```text
//! database -> orm
//! ```
//!
//! That edge means `orm` depends on `database`, so `database` must be sorted
//! first. Nodes that have no dependency relationship are emitted in the order
//! they appear in [`Graph::nodes`], giving callers deterministic output.
//!
//! Ported and modified from: <https://github.com/TheAlgorithms/Rust/blob/master/src/graph/topological_sort.rs>

use std::{
    backtrace::Backtrace,
    collections::{HashMap, HashSet, VecDeque},
};

/// A directed graph represented as an adjacency list of edges.
///
/// Each edge is a tuple `(source, destination)`. For prompt ordering, `source`
/// is the referenced dependency prompt and `destination` is the prompt that
/// depends on it.
pub type AdjacencyList<Node> = Vec<(Node, Node)>;

/// A graph data structure used for topological sorting.
///
/// `nodes` defines both the complete node set and the deterministic tie-breaker
/// order for independent nodes. `edges` defines ordering constraints between
/// those nodes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Graph<Node> {
    /// All nodes in the graph.
    pub nodes: Vec<Node>,
    /// Directed edges between nodes.
    pub edges: AdjacencyList<Node>,
}

/// Analysis result for a directed graph.
///
/// `sorted` is present only when the graph has no cycles. `cycles` contains
/// strongly connected components that make a topological order impossible.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphAnalysis<Node> {
    /// Valid topological order when the graph is acyclic.
    pub sorted: Option<Vec<Node>>,
    /// Cycles found in the graph.
    pub cycles: Vec<Cycle<Node>>,
}

/// A cyclic region of a graph.
///
/// The `nodes` field contains the nodes participating in the cycle. The `edges`
/// field contains the original graph edges where both endpoints are in
/// `nodes`. For a self-dependency, `nodes` contains one node and `edges`
/// contains the self edge.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cycle<Node> {
    /// Nodes participating in the cycle.
    pub nodes: Vec<Node>,
    /// Edges between cycle nodes.
    pub edges: Vec<(Node, Node)>,
}

/// Analyzes a graph for both topological order and cycle diagnostics.
///
/// This is the richer API for callers that need user-facing diagnostics. Use
/// [`sort_graph`] when all you need is the sorted order or an error.
pub fn analyze_graph<Node: std::hash::Hash + Eq + Clone>(
    graph: &Graph<Node>,
) -> GraphAnalysis<Node> {
    let nodes = normalized_nodes(graph);
    let sorted = topological_sort(&nodes, &graph.edges);
    let cycles = find_cycles(&nodes, &graph.edges);

    GraphAnalysis {
        sorted: if cycles.is_empty() {
            Some(sorted)
        } else {
            None
        },
        cycles,
    }
}

/// Sorts a graph with [Kahn's algorithm](https://en.wikipedia.org/wiki/Topological_sorting).
///
/// Given a graph, this function returns a vector of nodes in a valid
/// topological order or a [`SortError`] when the graph contains a cycle.
///
/// The implementation is deterministic: when multiple nodes are available to
/// emit, they are considered in the order they appear in [`Graph::nodes`].
///
/// # Errors
///
/// Returns [`SortError`] if the graph contains one or more cycles. Use
/// [`SortError::cycles`] to inspect the cyclic regions.
///
/// # Examples
///
/// ```ignore
/// use achitekfile::sort::{Graph, sort_graph};
///
/// let nodes: Vec<usize> = vec![2, 3, 5, 7, 8, 9, 10, 11];
/// let edges: Vec<(usize, usize)> = vec![
///     (5, 11),
///     (7, 8),
///     (7, 11),
///     (3, 8),
///     (3, 10),
///     (11, 2),
///     (11, 9),
///     (11, 10),
///     (8, 9),
/// ];
/// let graph: Graph<usize> = Graph { nodes, edges };
/// let sorted = sort_graph::<usize>(&graph)?;
///
/// assert_eq!(sorted.len(), 8);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn sort_graph<Node: std::hash::Hash + Eq + Clone>(
    graph: &Graph<Node>,
) -> Result<Vec<Node>, SortError<Node>> {
    let analysis = analyze_graph(graph);

    if let Some(sorted) = analysis.sorted {
        Ok(sorted)
    } else {
        Err(SortError::cycle_detected(analysis.cycles))
    }
}

fn normalized_nodes<Node: std::hash::Hash + Eq + Clone>(graph: &Graph<Node>) -> Vec<Node> {
    let mut nodes = graph.nodes.clone();
    let mut seen = nodes.iter().cloned().collect::<HashSet<_>>();

    for (source, destination) in &graph.edges {
        if seen.insert(source.clone()) {
            nodes.push(source.clone());
        }
        if seen.insert(destination.clone()) {
            nodes.push(destination.clone());
        }
    }

    nodes
}

fn topological_sort<Node: std::hash::Hash + Eq + Clone>(
    nodes: &[Node],
    edges: &[(Node, Node)],
) -> Vec<Node> {
    let mut dependencies_to_dependents_map: HashMap<Node, Vec<Node>> = HashMap::default();
    let mut in_degree_map: HashMap<Node, usize> = HashMap::default();
    for node in nodes {
        in_degree_map.entry(node.clone()).or_insert(0);
    }
    for (src, dest) in edges {
        dependencies_to_dependents_map
            .entry(src.clone())
            .or_default()
            .push(dest.clone());

        *in_degree_map.entry(dest.clone()).or_insert(0) += 1;
    }

    let mut queue: VecDeque<Node> = VecDeque::default();

    for node in nodes {
        if in_degree_map.get(node).is_some_and(|count| *count == 0) {
            queue.push_back(node.clone());
        }
    }

    let mut sorted: Vec<Node> = Vec::default();

    // process nodes from the queue, ensuring that dependencies are handled.
    while let Some(node_without_incoming_edges) = queue.pop_front() {
        sorted.push(node_without_incoming_edges.clone());

        in_degree_map.remove(&node_without_incoming_edges);

        if let Some(neighbors) = dependencies_to_dependents_map.get(&node_without_incoming_edges) {
            for neighbor in neighbors {
                if let Some(count) = in_degree_map.get_mut(neighbor) {
                    *count -= 1;

                    if *count == 0 {
                        in_degree_map.remove(neighbor);

                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }
    }

    sorted
}

fn find_cycles<Node: std::hash::Hash + Eq + Clone>(
    nodes: &[Node],
    edges: &[(Node, Node)],
) -> Vec<Cycle<Node>> {
    let mut adjacency = HashMap::<Node, Vec<Node>>::new();
    for node in nodes {
        adjacency.entry(node.clone()).or_default();
    }
    for (source, destination) in edges {
        adjacency
            .entry(source.clone())
            .or_default()
            .push(destination.clone());
    }

    let mut state = TarjanState {
        adjacency: &adjacency,
        index: 0,
        indexes: HashMap::new(),
        lowlinks: HashMap::new(),
        stack: Vec::new(),
        on_stack: HashSet::new(),
        components: Vec::new(),
    };

    for node in nodes {
        if !state.indexes.contains_key(node) {
            state.strong_connect(node.clone());
        }
    }

    state
        .components
        .into_iter()
        .filter_map(|component| component_to_cycle(component, edges))
        .collect()
}

struct TarjanState<'a, Node> {
    adjacency: &'a HashMap<Node, Vec<Node>>,
    index: usize,
    indexes: HashMap<Node, usize>,
    lowlinks: HashMap<Node, usize>,
    stack: Vec<Node>,
    on_stack: HashSet<Node>,
    components: Vec<Vec<Node>>,
}

impl<Node: std::hash::Hash + Eq + Clone> TarjanState<'_, Node> {
    fn strong_connect(&mut self, node: Node) {
        self.indexes.insert(node.clone(), self.index);
        self.lowlinks.insert(node.clone(), self.index);
        self.index += 1;
        self.stack.push(node.clone());
        self.on_stack.insert(node.clone());

        for neighbor in self.adjacency.get(&node).into_iter().flatten() {
            if !self.indexes.contains_key(neighbor) {
                self.strong_connect(neighbor.clone());
                let neighbor_lowlink = self.lowlinks[neighbor];
                let node_lowlink = self.lowlinks[&node];
                self.lowlinks
                    .insert(node.clone(), node_lowlink.min(neighbor_lowlink));
            } else if self.on_stack.contains(neighbor) {
                let neighbor_index = self.indexes[neighbor];
                let node_lowlink = self.lowlinks[&node];
                self.lowlinks
                    .insert(node.clone(), node_lowlink.min(neighbor_index));
            }
        }

        if self.indexes[&node] == self.lowlinks[&node] {
            let mut component = Vec::new();
            while let Some(item) = self.stack.pop() {
                self.on_stack.remove(&item);
                component.push(item.clone());
                if item == node {
                    break;
                }
            }
            component.reverse();
            self.components.push(component);
        }
    }
}

fn component_to_cycle<Node: std::hash::Hash + Eq + Clone>(
    nodes: Vec<Node>,
    edges: &[(Node, Node)],
) -> Option<Cycle<Node>> {
    let node_set = nodes.iter().cloned().collect::<HashSet<_>>();
    let cycle_edges = edges
        .iter()
        .filter(|(source, destination)| node_set.contains(source) && node_set.contains(destination))
        .cloned()
        .collect::<Vec<_>>();

    if nodes.len() > 1
        || cycle_edges
            .iter()
            .any(|(source, destination)| source == destination)
    {
        Some(Cycle {
            nodes,
            edges: cycle_edges,
        })
    } else {
        None
    }
}

/// Error returned when prompt dependencies cannot be topologically sorted.
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SortError<Node> {
    cycles: Vec<Cycle<Node>>,
    #[cfg_attr(
        feature = "serde",
        serde(skip, default = "std::backtrace::Backtrace::capture")
    )]
    backtrace: Backtrace,
}

impl<Node> SortError<Node> {
    fn cycle_detected(cycles: Vec<Cycle<Node>>) -> Self {
        Self {
            cycles,
            backtrace: Backtrace::capture(),
        }
    }

    /// Returns the cyclic regions that prevented topological sorting.
    ///
    /// See [`crate::model::ValidAchitekFile::prompts_in`] for an example.
    pub fn cycles(&self) -> &[Cycle<Node>] {
        &self.cycles
    }

    /// Returns the backtrace captured when the error was created.
    ///
    /// See [`crate::model::ValidAchitekFile::prompts_in`] for an example.
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }
}

impl<Node> std::error::Error for SortError<Node> where Node: core::fmt::Debug + core::fmt::Display {}

impl<Node: std::fmt::Display> std::fmt::Display for SortError<Node> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "cycle detected in graph")?;
        for cycle in &self.cycles {
            writeln!(f, "cycle nodes:")?;
            for node in &cycle.nodes {
                write!(f, "{} ", node)?;
            }
            writeln!(f, "\nedges:")?;
            for (src, dest) in &cycle.edges {
                writeln!(f, "  {} -> {}", src, dest)?;
            }
        }

        write!(f, "backtrace:\n{}", self.backtrace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_graph_is_ok_integer() {
        let nodes: Vec<usize> = vec![2, 3, 5, 7, 8, 9, 10, 11];
        let edges: Vec<(usize, usize)> = vec![
            (5, 11),
            (7, 8),
            (7, 11),
            (3, 8),
            (3, 10),
            (11, 2),
            (11, 9),
            (11, 10),
            (8, 9),
        ];
        let graph: Graph<usize> = Graph { nodes, edges };
        let sorted = sort_graph::<usize>(&graph);

        assert!(sorted.is_ok());
    }

    #[test]
    fn test_sort_graph_is_err_integer() {
        let nodes: Vec<usize> = vec![2, 3, 5, 7, 8, 9, 10, 11];
        let edges: Vec<(usize, usize)> = vec![
            (5, 11),
            (7, 8),
            (7, 11),
            (3, 8),
            (3, 10),
            (11, 2),
            (11, 9),
            (11, 10),
            (8, 9),
            (9, 11), // <-- cycle introduced
        ];
        let graph: Graph<usize> = Graph { nodes, edges };
        let sorted = sort_graph::<usize>(&graph);

        assert!(sorted.is_err());
    }

    #[test]
    fn test_sort_graph_is_ok_strings() {
        let nodes = vec![
            "shirt",
            "hoodie",
            "socks",
            "underwear",
            "pants",
            "shoes",
            "glasses",
            "watch",
            "school",
        ];
        let edges = vec![
            ("shirt", "hoodie"),
            ("hoodie", "school"),
            ("underwear", "pants"),
            ("pants", "shoes"),
            ("socks", "shoes"),
            ("shoes", "school"),
        ];
        let graph: Graph<&str> = Graph { nodes, edges };
        let sorted = sort_graph::<&str>(&graph);

        assert!(sorted.is_ok());
    }

    #[test]
    fn test_sort_graph_keeps_node_order_when_independent() {
        let graph = Graph {
            nodes: vec!["first", "second", "third"],
            edges: Vec::new(),
        };

        assert_eq!(
            sort_graph::<&str>(&graph).expect("Expected graph to sort"),
            vec!["first", "second", "third"]
        );
    }

    #[test]
    fn test_is_err_strings() {
        let nodes = vec![
            "shirt",
            "hoodie",
            "socks",
            "underwear",
            "pants",
            "shoes",
            "glasses",
            "watch",
            "school",
        ];
        let edges = vec![
            ("shirt", "hoodie"),
            ("hoodie", "school"),
            ("school", "shirt"), // <-- cycle introduced
            ("underwear", "pants"),
            ("pants", "shoes"),
            ("socks", "shoes"),
            ("shoes", "school"),
        ];
        let graph: Graph<&str> = Graph { nodes, edges };
        let sorted = sort_graph::<&str>(&graph);

        assert!(sorted.is_err());
    }

    #[test]
    fn analyze_graph_returns_cycle_participants() {
        let graph = Graph {
            nodes: vec!["database", "orm", "api", "frontend"],
            edges: vec![
                ("database", "orm"),
                ("orm", "api"),
                ("api", "database"),
                ("api", "frontend"),
            ],
        };

        let analysis = analyze_graph(&graph);

        assert_eq!(analysis.sorted, None);
        assert_eq!(analysis.cycles.len(), 1);
        assert_eq!(analysis.cycles[0].nodes, vec!["database", "orm", "api"]);
        assert_eq!(
            analysis.cycles[0].edges,
            vec![("database", "orm"), ("orm", "api"), ("api", "database")]
        );
    }

    #[test]
    fn analyze_graph_returns_self_cycle() {
        let graph = Graph {
            nodes: vec!["project", "author"],
            edges: vec![("project", "project"), ("project", "author")],
        };

        let analysis = analyze_graph(&graph);

        assert_eq!(analysis.sorted, None);
        assert_eq!(analysis.cycles.len(), 1);
        assert_eq!(analysis.cycles[0].nodes, vec!["project"]);
        assert_eq!(analysis.cycles[0].edges, vec![("project", "project")]);
    }
}
