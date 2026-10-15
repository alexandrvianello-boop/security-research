# Determinism is not a property a node can verify

A replicated state machine requires that for all nodes `n`, `m`, given identical input, `output(n) == output(m)`. That statement quantifies over the node set.

What a node can check about itself quantifies over nothing. The output is non-empty. It is a member of the submitted set. It meets the threshold. It is within bounds.

These are different specifications, and the second is strictly weaker than the first. A node satisfying every clause of its local specification tells you nothing about whether the system specification holds, because the local specification never mentions the other nodes. The gap is not a gap in rigour. It is a gap in what the sentence being verified says.

This is the same limit that bounds formal verification generally: a proof establishes that an implementation matches a specification, and says nothing about whether the specification is the right one. A node-local specification that does not quantify over nodes will verify clean against code that diverges.

## The gap is executable

The clearest demonstration is not an argument. It is two tests over the same function, both correct, disagreeing.

```rust
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

#[test]
fn functional_assertions_hold_regardless_of_which_winner_is_returned() {
    for _ in 0..1000 {
        let winner = select_hashmap();
        assert!(winner == "record_A" || winner == "record_B");
        assert!(!winner.is_empty());
    }
}
```

That test asserts everything a reasonable suite asserts about the output, a thousand times, and passes. The same function under a probe that compares results across runs reports two distinct outcomes. Both tests are correct. One of them is checking a specification that quantifies over executions, and the other is not.

Verified output, `rustc 1.75.0`, edition 2021:

```
running 3 tests
test demo::functional_assertions_hold_regardless_of_which_winner_is_returned ... ok
test demo::probe_detects_hashmap_divergence ... ok
test demo::probe_passes_btreemap ... ok

test result: ok. 3 passed; 0 failed
```

Property-based and coverage-guided fuzzing inherit the same limit, for the same reason. A fuzzer checks invariants you wrote, per execution. Divergence violates no invariant in either branch — both branches are valid outputs. And fuzzers are normally pinned to a fixed seed for reproducibility, which fixes the exact variation the check would need to vary.

## The economics do not apply

The security model of a proof-of-stake chain rests on two assumptions: that faults are attributable, and that attackers act when the cost of an attack is below the value extracted. Neither holds for this class.

**The diverging node is honest and unslashable.** It ran the released binary against the canonical input and produced a valid result. There is no signed equivocation, no invalid state transition, no provable misbehaviour — nothing an on-chain protocol can point to. Slashing requires attributable fault; here the fault is a property of the ensemble, and no individual member is at fault. The chain's entire incentive layer is inapplicable to the class by construction, not by oversight.

Agoric's incident is the shape of this: validators were jailed after diverging. From the network's perspective, divergence is fault, and an honest node running unmodified software is penalised as though it were malicious. There is no observable difference between a node that iterated a map in a different order and one that lied.

**There is no attacker, and therefore no cost/value calculus.** The standard model — an adversary weighs attack cost against extractable value — offers no protection here, because triggering divergence requires no adversary and extracts nothing. An ordinary transaction that happens to create the condition halts the chain. The expected-value reasoning that governs reentrancy, oracle manipulation, and MEV has nothing to price: the cost of triggering may be one transaction fee or zero, and the value extracted is nil.

Where an adversary *does* want the halt, the asymmetry is the steepest available. The lever is not the iteration order, which no attacker controls — Go randomises the starting bucket, Rust reseeds per map instance. The lever is the **condition under which ordering becomes decisive**: two entries equally satisfying the selection predicate. Whether that is cheap depends on the instance. In vote aggregation it costs control of half the nodes. In a map keyed by user-submitted data and selected by a `>=` predicate, it may cost two transactions.

**Damage is not bounded by capital.** An exploit extracts at most what is in the pool. A halt costs the network its availability for as long as recovery takes, and recovery is coordinated human action rather than a transaction. Osmosis stopped at height 287087 on an AppHash error traced to ordering in end-block code. Paloma partitioned into groups that each agreed internally and none of which reached threshold, so rounds advanced and no blocks were produced.

The combination — unslashable, unpriced, unbounded — is why this class deserves separate treatment rather than a line in a checklist.

## Where it originates

The Cosmos SDK carried it in its own core: `store/rootmulti`'s `buildCommitInfo` iterated a Go map to assemble the commit info feeding the app hash ([cosmos-sdk#12429](https://github.com/cosmos/cosmos-sdk/issues/12429)). Go randomises map iteration deliberately, to stop code depending on it. The fix collected keys into a slice and sorted them. A [follow-up issue](https://github.com/cosmos/cosmos-sdk/issues/13039) tracks sweeping the pattern out of the codebase.

Container iteration is the best-known source and not the only one:

- **Error values.** `packet-forward-middleware` was patched for chain-halt risk from non-determinism in *errors* ([GHSA-w6rp-vxj2-fjhr](https://github.com/advisories/GHSA-w6rp-vxj2-fjhr)). An error embedding a map key, an address, or a wrapped message whose text differs between nodes puts divergence into consensus state through a path nobody audits as state-mutating. The happy path can be deterministic while the failure path is not.
- **Floating point.** Results vary with compiler, optimisation level, and architecture. CosmWasm rejects float operations at the VM boundary rather than auditing for misuse — eliminating the source rather than detecting the symptom.
- **Clocks and scheduling.** System time, thread or goroutine interleaving, anything derived from allocation order or address values.
- **Environment.** Divergence depending on how a node was started rather than on the code. Provenance saw nodes synced from current snapshots progress while nodes replaying from older snapshots failed on app hash — same binary, same chain, different histories.

## What detection is achievable

Since the property quantifies over executions, the check must too: run the transition repeatedly against fresh state, digest each result, count distinct digests. That is AppHash comparison performed locally.

```rust
pub fn probe<T, F>(runs: usize, f: F) -> DeterminismReport
where
    T: Hash + Debug,
    F: Fn() -> T,
{
    let mut digests: HashSet<u64> = HashSet::new();
    let mut samples: Vec<String> = Vec::new();

    for _ in 0..runs {
        let out = f();
        let mut hasher = DefaultHasher::new();
        out.hash(&mut hasher);
        if digests.insert(hasher.finish()) {
            samples.push(format!("{:?}", out));
        }
    }

    DeterminismReport { runs, distinct: digests.len(), samples }
}
```

One detail decides whether it works: `f` must construct its own state on every call. A map built once holds a single seed for its lifetime and iterates consistently, hiding exactly what the probe looks for.

The bound is worth stating plainly, because it follows from the same argument as everything above. In-process repetition varies per-instance randomisation — `RandomState`, Go's map iteration, unseeded RNG. It does not vary architecture, toolchain version, clock, or filesystem ordering, so it cannot observe divergence sourced there. Those require differential replay across deliberately different environments; the Provenance case was found by replaying from an old snapshot, not by running the suite again. A probe reporting clean is evidence about one source, not a determinism proof. Extending the quantifier from executions to environments is a different harness.

Ranked by durability, elimination beats detection: deterministic containers in state-mutating paths, or float rejected at the VM boundary, remove sources that then need no test. Linting for map iteration in consensus paths scales to a codebase where a probe scales to a function. Replay differentials in CI cover the environmental class. Probes are precise and apply only where someone already suspected something.

The ordering matters because the class does not degrade gracefully. The chain agrees or it stops.

## Case study

Vote aggregation across off-chain nodes selected a winner by `HashMap` iteration, and a separate off-by-one in the threshold made ties reachable at all. The threshold admitted an exact 50% tie on even node counts; the container resolved that tie differently on each node. Every node's answer was individually valid, and every functional assertion about it held.

- [HashMap iteration order breaks consensus determinism across off-chain nodes](../contests/swafe-hashmap-consensus-nondeterminism.md)
- [Majority threshold admits exactly 50% on even node counts](../contests/swafe-majority-threshold-off-by-one.md)

Reviewing consensus code, the question that finds the class is not whether a line is correct. It is whether the specification being checked quantifies over the thing that has to agree.

## References

- [cosmos-sdk#12429](https://github.com/cosmos/cosmos-sdk/issues/12429) — non-determinism in `buildCommitInfo` via map iteration
- [cosmos-sdk#13039](https://github.com/cosmos/cosmos-sdk/issues/13039) — tracking map determinism across the SDK
- [cosmos-sdk#20106](https://github.com/cosmos/cosmos-sdk/issues/20106) — Paloma partition and the difficulty of localising divergence after the fact
- [GHSA-w6rp-vxj2-fjhr](https://github.com/advisories/GHSA-w6rp-vxj2-fjhr) — packet-forward-middleware chain-halt via error non-determinism
- [DSRV, Validator's Note — Frequent AppHash Error](https://medium.com/dsrv/validators-note-frequent-apphash-error-12a2e3ccfa8e) — Osmosis and Agoric incidents
- [Provenance, Discovering Non-Deterministic Behavior](https://medium.com/provenanceblockchain/discovering-non-deterministic-behavior-in-provenance-blockchain-and-cosmos-sdk-3b81b87b8698) — divergence found by replay from old snapshots
- [OtterSec, Cosmos Security: An Otter's Guide](https://osec.io/blog/2025-06-10-cosmos-security/) — map determinism among recurring Cosmos issue classes
