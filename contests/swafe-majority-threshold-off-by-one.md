# Majority threshold admits exactly 50% on even node counts

| | |
|---|---|
| **Contest** | Code4rena — Swafe (Nov 2025) |
| **Submission** | S-1164 |
| **Outcome** | Valid — Medium. Grouped with primary S-525 into one finding, [M-03](https://code4rena.com/reports/2025-11-swafe) |
| **Submitted as** | Medium. Grade held — see [Severity calibration](#severity-calibration). |

## Summary

The consensus threshold is computed as `div_ceil(n, 2)`. For even node counts this evaluates to exactly `n/2` — half, not a majority. A coalition controlling exactly 50% of nodes therefore qualifies to dictate the aggregation outcome, and the stated Byzantine fault tolerance is one node weaker than intended on every even-sized deployment.

## Affected code

`lib/src/association/v0.rs#L473` (MSK record aggregation), same computation at `#L556` (VDRF evaluation aggregation).

```rust
let majority_threshold = v0_records.len().div_ceil(2);
```

## Root cause

A majority requires strictly more than half. `div_ceil(n, 2)` returns the smallest integer at or above `n/2`, which for even `n` is `n/2` itself.

| Nodes | `div_ceil(n, 2)` | Share | Strict majority |
|---|---|---|---|
| 4 | 2 | 50% | 3 |
| 5 | 3 | 60% | 3 |
| 6 | 3 | 50% | 4 |
| 7 | 4 | 57% | 4 |
| 8 | 4 | 50% | 5 |

The formula is correct for odd counts and off by one for even ones. Because it produces the right answer half the time, the error survives casual reading — and a deployment that started odd and added a node silently loses a tolerance guarantee it previously had.

## Impact

Four nodes, an adversary controlling two:

- Adversary nodes vote for `attacker_record` — 2 votes
- Honest nodes vote for `legitimate_record` — 2 votes
- Threshold is 2, so **both records qualify**

The selection code takes the first qualifying entry rather than the one with the most votes, so the outcome is decided by whichever record the aggregating container yields first. In this codebase that container is a `HashMap`, whose iteration order is randomized per instance — meaning a 50% coalition wins some nodes and loses others, and the nodes themselves disagree.

This finding and the [`HashMap` iteration order finding](./swafe-hashmap-consensus-nondeterminism.md) compose: the threshold creates the tie, and the container decides it. The relationship is not symmetric. Fixing the threshold removes the tie, and with it the divergence; a deterministic tie-break alone would make nodes agree but would still let a coalition holding exactly half the nodes win. That is why the judges grouped both under this root cause.

## Proof of concept

Standalone, no project dependencies. Compiled and run against `rustc 1.75.0`, edition 2021.

```rust
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
```

```
$ rustc --test --edition 2021 poc_threshold.rs -o poc_threshold && ./poc_threshold

running 4 tests
test current_threshold_admits_exactly_half_on_even_node_counts ... ok
test current_threshold_is_correct_on_odd_node_counts ... ok
test strict_majority_always_exceeds_half ... ok
test two_candidates_can_qualify_simultaneously_on_even_counts ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The first test asserts the threshold equals exactly half on even counts — it fails once the formula is corrected. The second confirms the current formula is already correct for odd counts, which is why the bug is easy to miss. The fourth shows both candidates qualifying at the same threshold, which is the precondition the container non-determinism then resolves arbitrarily.

## Recommendation

Use the standard strict-majority formula at both sites:

```rust
let majority_threshold = v0_records.len() / 2 + 1;
```

This yields 3 of 4, 3 of 5, 4 of 6 — strictly more than half in every case, and it eliminates the tie condition rather than only making its resolution deterministic.

## Severity calibration

Submitted Medium, judged Medium. Including this alongside the `HashMap` finding is deliberate: that one I over-graded as High, this one I called correctly, and the difference between them is instructive.

Medium rather than High: no funds are directly at risk, and exploitation requires an even node count *and* an adversary already holding exactly half of it. That last condition is close to the point where the trust model has failed anyway.

Medium rather than Low: the code states a majority requirement and does not enforce one. A deployment reading the source would reasonably conclude it tolerates `f` faults when on even counts it tolerates `f-1`. Silently weakening a stated fault-tolerance property is worth more than a QA note, and it is the precondition that makes the consensus-divergence issue reachable at all.
