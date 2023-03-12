#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    pub dependencies: Vec<String>,
}

pub fn sort_deps(nodes: Vec<Node>) -> Vec<String> {
    // To make this more deterministic, we sort the nodes by their id
    let mut nodes = nodes;
    nodes.sort_by(|a, b| a.id.cmp(&b.id));

    let mut sorted = Vec::new();
    // First, push all nodes with no dependencies
    // And remove them from the list
    // Just push the IDs, not the whole node
    let mut nodes = nodes
        .into_iter()
        .filter(|node| {
            if node.dependencies.is_empty() {
                sorted.push(node.id.clone());
                false
            } else {
                true
            }
        })
        .collect::<Vec<_>>();

    // Loop until nodes are empty
    // Remove any dependencies from every node that is in sorted
    // If a node has no dependencies left, push it to sorted
    // And remove it from nodes
    while !nodes.is_empty() {
        let mut nodes_changed_in_this_pass = 0;

        nodes = nodes
            .iter()
            .filter_map(|node| {
                let mut node = node.clone();
                node.dependencies.retain(|dep| !sorted.contains(dep));
                if node.dependencies.is_empty() {
                    sorted.push(node.id.clone());
                    nodes_changed_in_this_pass += 1;
                    None
                } else {
                    Some(node)
                }
            })
            .collect::<Vec<_>>();

        if nodes_changed_in_this_pass == 0 {
            tracing::warn!("There are circular dependencies in the graph");
            for node in nodes {
                tracing::warn!("Node {} depends on {:?}", node.id, node.dependencies);
            }
            break;
        }
    }

    sorted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_deps() {
        let nodes = vec![
            Node {
                id: "a".to_string(),
                dependencies: vec!["b".to_string(), "c".to_string()],
            },
            Node {
                id: "b".to_string(),
                dependencies: vec!["c".to_string()],
            },
            Node {
                id: "c".to_string(),
                dependencies: vec![],
            },
        ];

        let sorted = sort_deps(nodes);
        assert_eq!(sorted, vec!["c", "b", "a"]);
    }

    #[test]
    fn test_sort_deps_with_circular_deps() {
        let nodes = vec![
            Node {
                id: "a".to_string(),
                dependencies: vec!["b".to_string(), "c".to_string()],
            },
            Node {
                id: "b".to_string(),
                dependencies: vec!["c".to_string()],
            },
            Node {
                id: "c".to_string(),
                dependencies: vec!["a".to_string()],
            },
            Node {
                id: "d".to_string(),
                dependencies: vec!["e".to_string(), "f".to_string()],
            },
            Node {
                id: "e".to_string(),
                dependencies: vec!["f".to_string()],
            },
            Node {
                id: "f".to_string(),
                dependencies: vec![],
            },
            Node {
                id: "g".to_string(),
                dependencies: vec!["g".to_string()],
            },
        ];

        let sorted = sort_deps(nodes);
        assert_eq!(sorted, vec!["f", "e", "d"]);
    }
}
