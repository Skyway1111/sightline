//! Iterative Tarjan over string nodes, shared by effect summaries, hotness and
//! #35. The Python source sorts the starts and every neighbour list, so the
//! map is a `BTreeMap` here and the order is the sort's, not a hash's.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, btree_set};

/// The adjacency `tarjan_scc` reads, from any rows of (node, its out-edges):
/// the one place a qname graph becomes the sorted string map.
pub fn edges<N, E>(rows: impl IntoIterator<Item = (N, E)>) -> BTreeMap<String, BTreeSet<String>>
where
    N: ToString,
    E: IntoIterator,
    E::Item: ToString,
{
    rows.into_iter()
        .map(|(node, outs)| {
            (
                node.to_string(),
                outs.into_iter().map(|o| o.to_string()).collect(),
            )
        })
        .collect()
}

/// Every edge target must be a key of `edges`. Components emit callees first
/// (reverse topological), each component's members in stack-pop order;
/// `comp_of` maps a node to its component index.
pub fn tarjan_scc(
    edges: &BTreeMap<String, BTreeSet<String>>,
) -> (Vec<Vec<String>>, HashMap<String, usize>) {
    let mut run = Run {
        edges,
        index: HashMap::new(),
        low: HashMap::new(),
        on_stack: HashSet::new(),
        stack: Vec::new(),
        components: Vec::new(),
        comp_of: HashMap::new(),
        counter: 0,
        work: Vec::new(),
    };
    for start in edges.keys() {
        if run.index.contains_key(start.as_str()) {
            continue;
        }
        run.push(start);
        run.drain();
    }
    (run.components, run.comp_of)
}

struct Run<'e> {
    edges: &'e BTreeMap<String, BTreeSet<String>>,
    index: HashMap<&'e str, usize>,
    low: HashMap<&'e str, usize>,
    on_stack: HashSet<&'e str>,
    stack: Vec<&'e str>,
    components: Vec<Vec<String>>,
    comp_of: HashMap<String, usize>,
    counter: usize,
    work: Vec<(&'e str, btree_set::Iter<'e, String>)>,
}

impl<'e> Run<'e> {
    fn push(&mut self, node: &'e str) {
        let neighbours = self
            .edges
            .get(node)
            .expect("tarjan_scc: every edge target must be a key")
            .iter();
        self.index.insert(node, self.counter);
        self.low.insert(node, self.counter);
        self.counter += 1;
        self.stack.push(node);
        self.on_stack.insert(node);
        self.work.push((node, neighbours));
    }

    fn drain(&mut self) {
        while let Some((node, _)) = self.work.last() {
            let node = *node;
            let mut advance = None;
            {
                let (_, it) = self
                    .work
                    .last_mut()
                    .expect("the frame is still on the stack");
                for next in it.by_ref() {
                    let next = next.as_str();
                    match self.index.get(next) {
                        None => {
                            advance = Some(next);
                            break;
                        }
                        Some(&seen) if self.on_stack.contains(next) => {
                            let low = self.low.get_mut(node).expect("node was pushed");
                            *low = (*low).min(seen);
                        }
                        Some(_) => {}
                    }
                }
            }
            if let Some(next) = advance {
                self.push(next);
                continue;
            }
            self.work.pop();
            if let Some((parent, _)) = self.work.last() {
                let below = self.low[node];
                let up = self.low.get_mut(*parent).expect("the parent was pushed");
                *up = (*up).min(below);
            }
            if self.low[node] == self.index[node] {
                let mut comp = Vec::new();
                loop {
                    let w = self.stack.pop().expect("the root is on the stack");
                    self.on_stack.remove(w);
                    comp.push(w.to_string());
                    self.comp_of.insert(w.to_string(), self.components.len());
                    if w == node {
                        break;
                    }
                }
                self.components.push(comp);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(rows: &[(&str, &[&str])]) -> BTreeMap<String, BTreeSet<String>> {
        edges(rows.iter().map(|(n, outs)| (n, outs.iter())))
    }

    #[test]
    fn components_emit_callees_first_with_members_in_pop_order() {
        let edges = graph(&[
            ("a", &["b"]),
            ("b", &["c"]),
            ("c", &["a", "d"]),
            ("d", &["e"]),
            ("e", &["d"]),
            ("f", &["a"]),
            ("g", &[]),
        ]);
        let (components, comp_of) = tarjan_scc(&edges);
        assert_eq!(
            components,
            vec![vec!["e", "d"], vec!["c", "b", "a"], vec!["f"], vec!["g"]]
        );
        assert_eq!(comp_of["d"], 0);
        assert_eq!(comp_of["a"], 1);
        assert_eq!(comp_of["f"], 2);
        assert_eq!(comp_of["g"], 3);
    }

    #[test]
    fn a_self_loop_is_its_own_component() {
        let (components, _) = tarjan_scc(&graph(&[("a", &["a"])]));
        assert_eq!(components, vec![vec!["a"]]);
        assert_eq!(tarjan_scc(&graph(&[])).0, Vec::<Vec<String>>::new());
    }
}
