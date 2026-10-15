/// Mirrors the threshold computation at lib/src/association/v0.rs:473 and :556.
fn current_threshold(node_count: usize) -> usize {
    node_count.div_ceil(2)
}

/// Standard strict-majority threshold.
fn strict_majority_threshold(node_count: usize) -> usize {
    node_count / 2 + 1
}

#[test]
fn current_threshold_admits_exactly_half_on_even_node_counts() {
    for n in (2..=12).step_by(2) {
        let t = current_threshold(n);
        assert_eq!(
            2 * t,
            n,
            "n={n}: threshold {t} is exactly 50%, not a majority"
        );
    }
}

#[test]
fn current_threshold_is_correct_on_odd_node_counts() {
    for n in (3..=13).step_by(2) {
        let t = current_threshold(n);
        assert!(2 * t > n, "n={}: threshold {} should exceed 50%", n, t);
    }
}

#[test]
fn strict_majority_always_exceeds_half() {
    for n in 2..=13 {
        let t = strict_majority_threshold(n);
        assert!(2 * t > n, "n={}: threshold {} must exceed 50%", n, t);
    }
}

#[test]
fn two_candidates_can_qualify_simultaneously_on_even_counts() {
    // Four nodes, votes split 2/2. Both records meet the threshold,
    // so selection falls through to container iteration order.
    let node_count = 4;
    let threshold = current_threshold(node_count);
    let votes = [("attacker_record", 2usize), ("honest_record", 2usize)];

    let qualifying: Vec<_> = votes
        .iter()
        .filter(|(_, count)| *count >= threshold)
        .map(|(name, _)| *name)
        .collect();

    assert_eq!(
        qualifying.len(),
        2,
        "expected both records to qualify at threshold {threshold}"
    );
}
