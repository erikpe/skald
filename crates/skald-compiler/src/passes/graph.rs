//! Deterministic graph algorithms shared by target-independent passes.

/// Returns one component index per node for an adjacency list whose node and
/// edge order is already canonical.
///
/// The implementation is iterative so deeply connected source programs do
/// not consume the compiler's call stack.
pub(crate) fn strongly_connected_components(adjacency: &[Vec<usize>]) -> Vec<usize> {
    let mut visited = vec![false; adjacency.len()];
    let mut finish_order = Vec::with_capacity(adjacency.len());
    for node in 0..adjacency.len() {
        if visited[node] {
            continue;
        }
        visited[node] = true;
        let mut pending = vec![(node, 0)];
        while let Some((current, next_edge)) = pending.last_mut() {
            if let Some(target) = adjacency[*current].get(*next_edge).copied() {
                *next_edge += 1;
                if !std::mem::replace(&mut visited[target], true) {
                    pending.push((target, 0));
                }
            } else {
                finish_order.push(*current);
                pending.pop();
            }
        }
    }

    let mut reverse = vec![Vec::new(); adjacency.len()];
    for (source, targets) in adjacency.iter().enumerate() {
        for target in targets {
            reverse[*target].push(source);
        }
    }
    for edges in &mut reverse {
        edges.sort_unstable();
        edges.dedup();
    }

    let mut components = vec![usize::MAX; adjacency.len()];
    let mut component = 0;
    for node in finish_order.into_iter().rev() {
        if components[node] != usize::MAX {
            continue;
        }
        components[node] = component;
        let mut pending = vec![node];
        while let Some(current) = pending.pop() {
            for target in &reverse[current] {
                if components[*target] == usize::MAX {
                    components[*target] = component;
                    pending.push(*target);
                }
            }
        }
        component += 1;
    }
    components
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separates_self_mutually_recursive_and_acyclic_components() {
        let adjacency = vec![vec![0], vec![2], vec![1], vec![]];
        let components = strongly_connected_components(&adjacency);

        assert_eq!(components[1], components[2]);
        assert_ne!(components[0], components[1]);
        assert_ne!(components[3], components[1]);
    }
}
