use std::collections::{BTreeMap, HashMap, HashSet};

const THRESHOLD: usize = 2;
const RUNS: usize = 1000;

/// Mirrors the vote-selection pattern at lib/src/association/v0.rs:468-477.
/// A fresh map is built per call: Rust reseeds RandomState per instance, so
/// iteration order varies within a single process the same way it varies
/// across independent nodes.
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
