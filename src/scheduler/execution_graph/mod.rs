use std::{collections::{HashMap, HashSet}, hash::Hash, rc::Rc, sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering}};

use crate::scheduler::execution_graph::{node::Node, ordering::ExecutionOrdering};

pub mod ordering;
pub mod node;

pub struct ExecutionGraph<T> {
    finished: AtomicBool,
    nodes: HashMap<T, HashSet<T>>,
    // AtomicU8, 0 Not Done, 1 Pending, 2 Finished
    // Need Not Bool because async functions would be constantly thrashed around by threads trying to "mark_as_complete" it- only one thread should be able to and that thread has the components
    // so i can eliminate "pending" from "leaves"
    // cant just say its finished because 1. Doesnt make sense- its not finished, and 2. Would pre-emptively finish the graph
    current_flow: HashMap<T, (AtomicUsize, AtomicU8)>
}

impl<T> ExecutionGraph<T> {
    pub const INIT: u8 = 0;
    pub const PENDING: u8 = 1;
    pub const FINISHED: u8 = 2;

    pub fn new_empty() -> Self {
        Self {
            finished: AtomicBool::new(true),
            nodes: HashMap::new(),
            current_flow: HashMap::new()
        }
    }

    pub fn finished(&self) -> &AtomicBool {
        &self.finished
    }

    pub fn create_flow(nodes: &HashMap<T, HashSet<T>>) -> impl Iterator<Item = (&T, (AtomicUsize, AtomicU8))> {
        nodes.iter().map(|(node, after)| {
            (node, (AtomicUsize::new(after.len()), AtomicU8::new(Self::INIT)))
        })
    }

    pub fn leaves(&self) -> impl Iterator<Item = (&T, &AtomicU8)> {
        self.current_flow.iter().filter_map(|(id, (dependents, finished))| {
            if finished.load(Ordering::Acquire) != Self::FINISHED && dependents.load(Ordering::Acquire) == 0 {
                Some((id, finished))
            } else {
                None
            }
        })
    }
}

impl<T> ExecutionGraph<T> where T: Eq + Hash + Clone {
    pub fn new<Y>(nodes: &[(T, &Y)]) -> Self where Y: ExecutionOrdering<Item = T> {
        if nodes.len() == 0 {
            return Self::new_empty();
        }

        let node_list = nodes.iter().map(|(node, _)| node.clone()).collect::<HashSet<_>>();

        let subsumed_nodes = nodes.iter().map(|(node, ordering)| {
            (node, ordering.subsume(&node_list))
        });

        let mut new_nodes = HashMap::new();
        let mut conditional_afters = HashMap::new();
        for (node, ordering) in subsumed_nodes {
            let (
                mut befores,
                afters,
                priority
            ) = ordering.consume();

            for before in befores.drain() {
                conditional_afters.entry(before).or_insert(HashSet::new()).insert(node.clone());
            }

            new_nodes.insert(node, (afters, priority));
        }

        for (node, new_afters) in conditional_afters {
            new_nodes.get_mut(&node).unwrap().0.extend(new_afters);
        }

        Self::break_cycles(&mut new_nodes, &node_list);

        let nodes = new_nodes.into_iter().map(|(node, (after, _))| (node.clone(), after)).collect();
        let current_flow = Self::create_flow(&nodes).map(|(t, data)| (t.clone(), data)).collect();

        Self {
            finished: AtomicBool::new(false),
            nodes,
            current_flow
        }
    }

    pub fn break_cycles(
        nodes: &mut HashMap<&T, (HashSet<T>, f64)>,
        node_list: &HashSet<T>
    ) {
        let mut seen = HashSet::new();

        Self::construct_paths(nodes, None, &mut seen);

        while seen != *node_list {
            if let Some((node, _)) = {
                nodes
                    .iter()
                    .filter(|(node, _)| {
                        !seen.contains(&node)
                    })
                    .max_by(|(_, (_, p1)), (_, (_, p2))| {
                        p1.total_cmp(p2)
                    })
            } {
                let current_path = Node {
                    parent: None,
                    id: (*node).clone()
                };

                seen.insert((*node).clone());

                let current_path = Rc::new(current_path);

                Self::construct_paths(nodes, Some(Rc::clone(&current_path)), &mut seen);
            } else {
                unreachable!("this case implies `seen == node_list`");
            }
        }
    }

    pub fn construct_paths(
        nodes: &mut HashMap<&T, (HashSet<T>, f64)>, 
        current_path: Option<Rc<Node<T>>>, 
        seen_list: &mut HashSet<T>
    ) {
        let leaves: HashSet<T> = match &current_path {
            Some(current_path) => {
                nodes.iter().filter_map(|(node, (after, _))| {
                    if after.contains(&current_path.id) {
                        Some(*node)
                    } else {
                        None
                    }
                }).cloned().collect()
            },
            None => {
                nodes.iter().filter_map(|(node, (after, _))| {
                    if after.is_empty() {
                        Some(*node)
                    } else {
                        None
                    }
                }).cloned().collect()
            }
        };

        for leaf in leaves {
            seen_list.insert(leaf.clone());

            if let Some((max, child_of_max)) = Self::find_max_priority_in_cycle(nodes, &current_path, &leaf) {
                if let Some(child_of_max) = child_of_max {
                    nodes.get_mut(child_of_max).unwrap().0.remove(max);
                } else {
                    nodes.get_mut(&leaf).unwrap().0.remove(max);
                }
            } else {
                let new_current_path = if let Some(current_path) = &current_path {
                    Node {
                        parent: Some(Rc::clone(current_path)),
                        id: leaf
                    }
                } else {
                    Node {
                        parent: None,
                        id: leaf
                    }
                };

                Self::construct_paths(nodes, Some(Rc::new(new_current_path)), seen_list);
            }
        }
    }

    pub fn find_max_priority_in_cycle<'a>(
        priority_mapping: &HashMap<&T, (HashSet<T>, f64)>, 
        current_path: &'a Option<Rc<Node<T>>>,
        looking_for: &T
    ) -> Option<(&'a T, Option<&'a T>)> {
        if current_path.is_none() {
            return None
        }
        let current_path = current_path.as_ref().unwrap();

        let mut child_of_max = None;
        let mut max = current_path;

        #[allow(unused_assignments)]
        let mut child_of_current = None;
        
        let mut current = current_path;

        while current.parent.is_some() {
            child_of_current = Some(current);
            current = current.parent.as_ref().unwrap();

            let max_priority = priority_mapping.get(&max.id).unwrap().1;
            let current_priority = priority_mapping.get(&current.id).unwrap().1;

            if current_priority > max_priority {
                child_of_max = child_of_current;
                max = current;
            }

            if current.id == *looking_for {
                return Some((&max.id, child_of_max.map(|node| &node.id)))
            }
        }

        None
    }

    pub fn mark_as_complete(&mut self, marking: &T) {
        let previous_state = self.current_flow.get_mut(marking).unwrap().1.swap(Self::FINISHED, Ordering::Release);
        if previous_state == Self::FINISHED {
            return;
        }

        for (t, afters) in &self.nodes {
            if afters.contains(marking) {
                // Can't change between lines of code because &mut self
                let value = self.current_flow.get(t).unwrap().0.load(Ordering::Acquire);
                if value != 0 {
                    self.current_flow.get_mut(t).unwrap().0.store(value - 1, Ordering::Release);   
                }
            }
        }
    }

    pub fn mark_as_pending(&mut self, pending: &T) {
        self.current_flow.get_mut(pending).unwrap().1.compare_exchange(Self::INIT, Self::PENDING, Ordering::SeqCst, Ordering::Relaxed).unwrap();
    }
}

#[cfg(test)]
mod execution_graph {
    use std::collections::HashSet;

    use proptest::prelude::*;

    use crate::scheduler::execution_graph::{ordering::ExecutionOrdering, ExecutionGraph};

    type T = u8;

    #[allow(dead_code)]
    #[derive(Debug)]
    pub struct Ordering {
        pub before: HashSet<T>,
        pub after: HashSet<T>,
        pub priority: f64
    }

    impl ExecutionOrdering for Ordering {
        type Item = T;
        fn consume(self) -> ( HashSet<Self::Item>, HashSet<Self::Item>, f64 ) {
            (self.before, self.after, self.priority)
        }

        fn subsume(&self, superset: &HashSet<Self::Item>) -> Self {
            Self {
                before: self.before.intersection(superset).cloned().collect(),
                after: self.after.intersection(superset).cloned().collect(),
                priority: self.priority
            } 
        }
    }

    const SIZE: std::ops::Range<usize> = 0..45;
    
    fn ordering_set_strategy() -> impl Strategy<Value = HashSet<T>> {
        prop::collection::hash_set(any::<T>(), SIZE)
    }

    fn ordering_strategy() -> impl Strategy<Value = Ordering> {
        (ordering_set_strategy(), ordering_set_strategy(), any::<f64>()).prop_map(|(before, after, priority)| Ordering { before, after, priority })        
    }

    fn network_strategy() -> impl Strategy<Value = Vec<(T, Ordering)>> {
        prop::collection::vec((any::<T>(), ordering_strategy()), SIZE)
    }

    proptest! {
        #[test]
        fn graph(input in network_strategy()) {
            let should_see = input.iter().map(|(node, _)| { node.clone() }).collect::<HashSet<_>>();
            let input: Vec<(T, Ordering)> = input;
            let inputs = input.iter().map(|(node, ordering)| (node.clone(), ordering)).collect::<Vec<_>>();
            let graph = ExecutionGraph::new(&inputs);

            assert_eq!(graph.current_flow.len(), should_see.len());
            assert_eq!(graph.nodes.len(), should_see.len());
            
            can_end_check(graph, should_see);
        }
    }

    fn can_end_check(mut graph: ExecutionGraph<T>, mut should_see: HashSet<T>) {
        // check if can reach the end and see all nodes only once and that there are leaves at the start
        if graph.nodes.len() > 0 {
            assert!(graph.leaves().count() > 0);
        }

        while graph.leaves().count() > 0 {
            let leaf = graph.leaves().next().unwrap().0.clone();
            assert!(should_see.remove(&leaf));
            graph.mark_as_complete(&leaf);
        }

        assert!(should_see.is_empty());
    }

    #[test]
    fn priority_awareness() {
        let a = 1;
        let b = 2;
        let c = 3;
        let d = 4;

        let ao = Ordering {
            before: HashSet::from([b]),
            after: HashSet::from([d]),
            priority: 1.0
        };

        let bo = Ordering {
            before: HashSet::new(),
            after: HashSet::new(),
            priority: 2.0
        };

        let co = Ordering {
            before: HashSet::from([d]),
            after: HashSet::from([b]),
            priority: 3.0
        };

        let ddo = Ordering {
            before: HashSet::new(),
            after: HashSet::new(),
            priority: 2.5
        };

        let input = vec![
            (a, &ao),
            (b, &bo),
            (c, &co),
            (d, &ddo)
        ];

        let mut graph = ExecutionGraph::new(&input);
        let should_see = vec![d, a, b, c];
        let mut counting: HashSet<&u8> = HashSet::from_iter(should_see.iter());

        let mut index = 0;
        while graph.leaves().count() > 0 {
            let leaf = graph.leaves().next().unwrap().0.clone();
            assert_eq!(leaf, *should_see.get(index).unwrap());
            assert!(counting.remove(&leaf));
            graph.mark_as_complete(&leaf);
            index += 1;
        }
        assert_eq!(index, 4)
    }

    #[test]
    fn non_cycle() {
        let a = 1;
        let b = 2;
        let c = 3;
        let d = 4;
        let e = 5;

        let ao = Ordering {
            before: HashSet::from([c]),
            after: HashSet::from([]),
            priority: 0.0
        };

        let bo = Ordering {
            before: HashSet::from([]),
            after: HashSet::from([]),
            priority: 0.0
        };

        let co = Ordering {
            before: HashSet::from([]),
            after: HashSet::from([b]),
            priority: 0.0
        };

        let ddo = Ordering {
            before: HashSet::from([e]),
            after: HashSet::from([]),
            priority: 0.0
        };

        let eo = Ordering {
            before: HashSet::from([]),
            after: HashSet::from([c]),
            priority: 0.0
        };

        let input = vec![
            (a, &ao),
            (b, &bo),
            (c, &co),
            (d, &ddo),
            (e, &eo)
        ];

        let mut graph = ExecutionGraph::new(&input);

        let leaves = graph.leaves().map(|(a, _)| a).cloned().collect::<HashSet<_>>();
        assert_eq!(leaves, HashSet::from([a, b, d]));
        for l in leaves {
            graph.mark_as_complete(&l);
        }
        let leaves = graph.leaves().map(|(a, _)| a).cloned().collect::<HashSet<_>>();
        assert_eq!(leaves, HashSet::from([c]));
        graph.mark_as_complete(&c);
        let leaves = graph.leaves().map(|(a, _)| a).cloned().collect::<HashSet<_>>();
        assert_eq!(leaves, HashSet::from([e]));
        graph.mark_as_complete(&e);
    }

    #[test]
    fn closed_cycles_priority_last() {
        let a = 1;
        let b = 2;
        let c = 3;
        let d = 4;

        let ao = Ordering {
            before: HashSet::from([b]),
            after: HashSet::from([]),
            priority: 10.0
        };

        let bo = Ordering {
            before: HashSet::from([c]),
            after: HashSet::from([d]),
            priority: 2.0
        };

        let co = Ordering {
            before: HashSet::from([d]),
            after: HashSet::from([]),
            priority: 3.0
        };

        let ddo = Ordering {
            before: HashSet::from([]),
            after: HashSet::from([]),
            priority: 4.0
        };

        let input = vec![
            (a, &ao),
            (b, &bo),
            (c, &co),
            (d, &ddo)
        ];

        let mut graph = ExecutionGraph::new(&input);
        let should_see = vec![a, b, c, d];
        let mut counting: HashSet<&u8> = HashSet::from_iter(should_see.iter());

        let mut index = 0;
        while graph.leaves().count() > 0 {
            let leaf = graph.leaves().next().unwrap().0.clone();
            assert_eq!(leaf, *should_see.get(index).unwrap());
            assert!(counting.remove(&leaf));
            graph.mark_as_complete(&leaf);
            index += 1;
        }

        assert_eq!(index, 4);
    }

    #[test]
    fn closed_cycles_priority_penultimate() {
        let a = 1;
        let b = 2;
        let c = 3;
        let d = 4;

        let ao = Ordering {
            before: HashSet::from([b]),
            after: HashSet::from([]),
            priority: 10.0
        };

        let bo = Ordering {
            before: HashSet::from([c]),
            after: HashSet::from([d]),
            priority: 2.0
        };

        let co = Ordering {
            before: HashSet::from([d]),
            after: HashSet::from([]),
            priority: 4.0
        };

        let ddo = Ordering {
            before: HashSet::from([]),
            after: HashSet::from([]),
            priority: 3.0
        };

        let input = vec![
            (b, &bo),
            (c, &co),
            (a, &ao),
            (d, &ddo),
        ];

        let mut graph = ExecutionGraph::new(&input);

        let leaves = graph.leaves().map(|(a, _)| a).cloned().collect::<HashSet<_>>();
        // may seem odd, why d as well?
        // this seems odd because you imagine a -> (b -> c -> d -> b), by priority the `c->d` connection is cut, you may think the d connection should then be before b: a -> (d -> b -> c) but instead its (a, d) -> (b -> c). 
        // When the connection between C and D is cut there isn't a reason to bind D anymore except to the start of the cycle
        assert_eq!(leaves, HashSet::from([a, d]));
        graph.mark_as_complete(&a);
        graph.mark_as_complete(&d);
        let leaves = graph.leaves().map(|(a, _)| a).cloned().collect::<HashSet<_>>();
        assert_eq!(leaves, HashSet::from([b]));
        graph.mark_as_complete(&b);
        let leaves = graph.leaves().map(|(a, _)| a).cloned().collect::<HashSet<_>>();
        assert_eq!(leaves, HashSet::from([c]));
        graph.mark_as_complete(&c);
    }

    #[test]
    fn closed_cycles_priority_first() {
        let a = 1;
        let b = 2;
        let c = 3;
        let d = 4;

        let ao = Ordering {
            before: HashSet::from([b]),
            after: HashSet::from([]),
            priority: 10.0
        };

        let bo = Ordering {
            before: HashSet::from([c]),
            after: HashSet::from([d]),
            priority: 4.0
        };

        let co = Ordering {
            before: HashSet::from([d]),
            after: HashSet::from([]),
            priority: 2.0
        };

        let ddo = Ordering {
            before: HashSet::from([]),
            after: HashSet::from([]),
            priority: 3.0
        };

        let input = vec![
            (a, &ao),
            (d, &ddo),
            (c, &co),
            (b, &bo),
        ];

        let mut graph = ExecutionGraph::new(&input);

        let leaves = graph.leaves().map(|(a, _)| a).cloned().collect::<HashSet<_>>();
        assert_eq!(leaves, HashSet::from([a, c]));
        graph.mark_as_complete(&a);
        graph.mark_as_complete(&c);
        let leaves = graph.leaves().map(|(a, _)| a).cloned().collect::<HashSet<_>>();
        assert_eq!(leaves, HashSet::from([d]));
        graph.mark_as_complete(&d);
        let leaves = graph.leaves().map(|(a, _)| a).cloned().collect::<HashSet<_>>();
        assert_eq!(leaves, HashSet::from([b]));
        graph.mark_as_complete(&b);
    }

    #[test]
    fn closed_cycles_priority_first_two_equal() {
        let a = 1;
        let b = 2;
        let c = 3;
        let d = 4;

        let ao = Ordering {
            before: HashSet::from([b]),
            after: HashSet::from([]),
            priority: 10.0
        };

        let bo = Ordering {
            before: HashSet::from([c]),
            after: HashSet::from([d]),
            priority: 4.0
        };

        let co = Ordering {
            before: HashSet::from([d]),
            after: HashSet::from([]),
            priority: 4.0
        };

        let ddo = Ordering {
            before: HashSet::from([]),
            after: HashSet::from([]),
            priority: 2.0
        };

        let input = vec![
            (d, &ddo),
            (b, &bo),
            (a, &ao),
            (c, &co),
        ];

        let mut graph = ExecutionGraph::new(&input);

        let leaves = graph.leaves().map(|(a, _)| a).cloned().collect::<HashSet<_>>();
        assert_eq!(leaves, HashSet::from([a, d]));
        graph.mark_as_complete(&a);
        graph.mark_as_complete(&d);
        let leaves = graph.leaves().map(|(a, _)| a).cloned().collect::<HashSet<_>>();
        assert_eq!(leaves, HashSet::from([b]));
        graph.mark_as_complete(&b);
        let leaves = graph.leaves().map(|(a, _)| a).cloned().collect::<HashSet<_>>();
        assert_eq!(leaves, HashSet::from([c]));
        graph.mark_as_complete(&c);
    }

    #[test]
    fn contradictions() {
        let a = 1;
        let b = 2;
        let c = 3;

        let ao = Ordering {
            before: HashSet::from([]),
            after: HashSet::from([]),
            priority: 0.0
        };

        let bo = Ordering {
            before: HashSet::from([b, c]),
            after: HashSet::from([a]),
            priority: 0.0
        };

        let co = Ordering {
            before: HashSet::from([]),
            after: HashSet::from([]),
            priority: 0.0
        };

        let input = vec![
            (b, &bo),
            (a, &ao),
            (c, &co),
        ];

        let mut graph = ExecutionGraph::new(&input);

        let leaves = graph.leaves().map(|(a, _)| a).cloned().collect::<HashSet<_>>();
        assert_eq!(leaves, HashSet::from([a]));
        graph.mark_as_complete(&a);
        let leaves = graph.leaves().map(|(a, _)| a).cloned().collect::<HashSet<_>>();
        assert_eq!(leaves, HashSet::from([b]));
        graph.mark_as_complete(&b);
        let leaves = graph.leaves().map(|(a, _)| a).cloned().collect::<HashSet<_>>();
        assert_eq!(leaves, HashSet::from([c]));
        graph.mark_as_complete(&c);
    }
}