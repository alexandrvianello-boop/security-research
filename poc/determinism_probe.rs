//! A local analogue of AppHash comparison.
//!
//! Replicated state machines fail when two nodes compute different results from
//! the same input. That divergence is not observable from inside a single run,
//! because one run is one execution path. This harness makes it observable by
//! executing the same transition repeatedly against fresh state and digesting
//! each result, the way validators digest their state and compare.
//!
//! Scope: this catches per-instance randomisation — Rust's `RandomState`, Go's
//! randomised map iteration, unseeded RNG. It does NOT catch divergence whose
//! source is outside the process: architecture, toolchain version, wall clock,
//! filesystem ordering, locale. Those require differential replay across
//! environments, not repetition within one.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

#[derive(Debug)]
pub struct DeterminismReport {
    pub runs: usize,
    pub distinct: usize,
    /// One formatted sample per distinct result, in order of first appearance.
    pub samples: Vec<String>,
}

impl DeterminismReport {
    pub fn is_deterministic(&self) -> bool {
        self.distinct <= 1
    }
}

/// Executes `f` `runs` times and reports how many distinct results were seen.
///
/// `f` must construct its own state on each call. Reusing state across calls
/// defeats the probe: a `HashMap` built once keeps one seed for its lifetime,
/// so it will iterate consistently and the divergence will not appear.
pub fn probe<T, F>(runs: usize, f: F) -> DeterminismReport
where
    T: Hash + Debug,
    F: Fn() -> T,
{
    let mut digests: HashSet<u64> = HashSet::new();
    let mut samples: Vec<String> = Vec::new();

    for _ in 0..runs {
        let out = f();
        // DefaultHasher::new() is fixed-key, so the digest itself is stable.
        let mut hasher = DefaultHasher::new();
        out.hash(&mut hasher);
        if digests.insert(hasher.finish()) {
            samples.push(format!("{:?}", out));
        }
    }

    DeterminismReport {
        runs,
        distinct: digests.len(),
        samples,
    }
}

#[macro_export]
macro_rules! assert_deterministic {
    ($runs:expr, $f:expr) => {{
        let report = probe($runs, $f);
        assert!(
            report.is_deterministic(),
            "non-deterministic: {} distinct results over {} runs; samples: {:?}",
            report.distinct,
            report.runs,
            report.samples
        );
    }};
}

#[cfg(test)]
mod demo {
    use super::*;
    use std::collections::{BTreeMap, HashMap};

    const THRESHOLD: usize = 2;

    /// Vote aggregation over a HashMap, tied at the threshold.
    fn select_hashmap() -> &'static str {
        let mut votes: HashMap<&'static str, usize> = HashMap::new();
        votes.insert("record_A", 2);
        votes.insert("record_B", 2);

        votes
            .into_iter()
            .find(|(_, count)| *count >= THRESHOLD)
            .map(|(record, _)| record)
            .unwrap()
    }

    /// Identical logic over BTreeMap.
    fn select_btreemap() -> &'static str {
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
    fn probe_detects_hashmap_divergence() {
        let report = probe(1000, select_hashmap);
        assert_eq!(
            report.distinct, 2,
            "expected the probe to observe both winners, got {:?}",
            report
        );
        assert!(!report.is_deterministic());
    }

    #[test]
    fn probe_passes_btreemap() {
        assert_deterministic!(1000, select_btreemap);
    }

    /// Each candidate is individually a valid majority winner. Every functional
    /// assertion a test suite would make about the result holds in both cases.
    /// Only the comparison across runs exposes the fault.
    #[test]
    fn functional_assertions_hold_regardless_of_which_winner_is_returned() {
        for _ in 0..1000 {
            let winner = select_hashmap();
            assert!(winner == "record_A" || winner == "record_B");
            assert!(!winner.is_empty());
        }
    }
}
