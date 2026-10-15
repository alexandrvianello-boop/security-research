# HashMap iteration order breaks consensus determinism across off-chain nodes

| | |
|---|---|
| **Contest** | Code4rena — Swafe (Nov 2025) |
| **Submission** | S-1157 |
| **Outcome** | Valid — Medium. Grouped with primary S-525 into one finding, [M-03](https://code4rena.com/reports/2025-11-swafe), under the threshold root cause |
| **Submitted as** | High. Medium is the correct grade — see [Severity calibration](#severity-calibration). |

## Summary

Vote aggregation across off-chain nodes selects a winner by iterating a `HashMap`. Rust reseeds `RandomState` per map instance, so iteration order differs between nodes. When two candidates both meet the majority threshold, each node selects whichever it encounters first — meaning nodes reach different conclusions from **identical input**. Recovery requires agreement among these nodes.

## Affected code

`lib/src/association/v0.rs#L468-L477` (MSK record aggregation), same pattern at `#L551-L560` (VDRF evaluation aggregation).

```rust
let mut votes = HashMap::new();
for (_, record) in &v0_records {
    *votes.entry(record.fixed.clone()).or_insert(0) += 1;
}

let majority_fixed = votes
    .into_iter()
    .find(|(_, count)| *count >= majority_threshold)
    .map(|(fixed, _)| fixed)
```

## Root cause

`find()` returns the first element satisfying the predicate. Over a `HashMap`, "first" is defined by iteration order, and Rust randomizes that per instance by design.

The predicate is `count >= majority_threshold` — qualification, not maximality. It does not ask which candidate has the *most* votes; it returns any candidate at or above the bar. With a strict majority this is harmless, since only one candidate can exceed 50%. Here the threshold is `div_ceil(n, 2)`, which for even `n` evaluates to exactly 50% — so two candidates can qualify simultaneously, and the tiebreak falls through to iteration order.

The bug is therefore a composition of two decisions: a non-deterministic container, and a threshold that admits exact ties. Either alone is survivable. Together they make node agreement a coin flip.

## Impact

Four nodes, votes `{A: 2, B: 2}`, threshold `2` — both qualify:

- Node 1 iterates `[A, B]` → selects A
- Node 2 iterates `[B, A]` → selects B

Nodes disagree on which MSK record is authoritative while processing the same data. Recovery, which depends on node agreement, either stalls or proceeds against inconsistent state depending on which nodes happen to agree.

## Proof of concept

Standalone, no project dependencies. Compiled and run against `rustc 1.75.0`.

```rust
use std::collections::{BTreeMap, HashMap, HashSet};

const THRESHOLD: usize = 2;
const RUNS: usize = 1000;

/// Mirrors the selection pattern at v0.rs:468-477. A fresh map per call:
/// RandomState reseeds per instance, so iteration order varies within one
/// process the same way it varies across independent nodes.
fn select_with_hashmap() -> &'static str {
    let mut votes: HashMap<&'static str, usize> = HashMap::new();
    votes.insert("record_A", 2);
    votes.insert("record_B", 2);

    votes
        .into_iter()
        .find(|(_, count)| *count >= THRESHOLD)
        .map(|(record, _)| record)
        .unwrap()
}

/// Identical logic over BTreeMap, which iterates in key order.
fn select_with_btreemap() -> &'static str {
    let mut votes: BTreeMap<&'static str, usize> = BTreeMap::new();
    votes.insert("record_A", 2);
    votes.insert("record_B", 2);

    votes
        .into_iter()
        .find(|(_, count)| *count >= THRESHOLD)
        .map(|(record, _)| record)
        .unwrap()
}

#[test]
fn hashmap_selection_is_nondeterministic() {
    let mut seen = HashSet::new();
    for _ in 0..RUNS {
        seen.insert(select_with_hashmap());
    }
    assert_eq!(
        seen.len(),
        2,
        "identical input should have produced both winners; saw {:?}",
        seen
    );
}

#[test]
fn btreemap_selection_is_deterministic() {
    let mut seen = HashSet::new();
    for _ in 0..RUNS {
        seen.insert(select_with_btreemap());
    }
    assert_eq!(seen.len(), 1, "expected a single winner; saw {:?}", seen);
    assert_eq!(seen.into_iter().next().unwrap(), "record_A");
}
```

```
$ rustc --test poc.rs -o poc && ./poc

running 2 tests
test btreemap_selection_is_deterministic ... ok
test hashmap_selection_is_nondeterministic ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The first test asserts that identical input yields **both** winners across runs — it fails once the container is made deterministic, which is the property being demonstrated. The second confirms `BTreeMap` collapses the same input to a single stable winner.

## Recommendation

Replace `HashMap` with `BTreeMap` at both sites. Key ordering is total and identical across processes, so every node resolves a tie the same way.

```rust
let mut votes: BTreeMap<MskRecordFixed, usize> = BTreeMap::new();
for (_, record) in &v0_records {
    *votes.entry(record.fixed.clone()).or_insert(0) += 1;
}
```

This makes the tiebreak deterministic. It does not make it *correct* — a tie still resolves to whichever record sorts first, rather than being rejected as inconclusive. Fixing the threshold to require a strict majority removes the tie condition itself; the container change removes the divergence.

## Severity calibration

Judged Medium. That is the right grade, and my High was wrong.

High on this platform generally requires assets at direct risk or unauthorized access. This finding is neither: no attacker action is required to trigger it, nothing is stolen, and no privilege is escalated. What breaks is agreement — recovery stalls or advances on inconsistent state. That is an availability and consistency failure in an off-chain process, which is Medium territory.

It also carries a precondition I did not weigh at submission time: the tie is only reachable on an even node count with votes split exactly at the threshold. Real impact depends on deployment topology, not on the code alone.

The mistake was scoring the *class* of the bug — consensus divergence sounds severe — instead of its realised impact under stated preconditions.
