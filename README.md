# Security Research

Protocol and smart contract security — consensus logic and trust boundaries. Rust and Solidity.

Code4rena: [@XOMA](https://code4rena.com/@XOMA)

## Research

**[Determinism is not a property a node can verify](research/determinism-is-not-node-verifiable.md)**

A replicated state machine requires that all nodes agree; a node can only verify a specification that never mentions the other nodes. The gap is where chain halts come from. Covers why the property is not node-verifiable, why the chain's economic security model does not reach the class — the diverging node is honest and unslashable, and triggering needs no attacker — and what detection is achievable. Grounded in the Cosmos SDK's own `buildCommitInfo`, the Osmosis and Paloma halts, and error-path divergence in `packet-forward-middleware`.

## Contest findings

Each writeup states the finding, the affected code, the root cause, and a proof of concept that compiles and runs. Every PoC asserts the vulnerable property directly, so it fails once the bug is fixed. Each writeup also records the severity I submitted against the severity awarded — including where I got it wrong and why the judge was right.

| Submission | Contest | Submitted → judged |
|---|---|---|
| [Majority threshold admits exactly 50% on even node counts](contests/swafe-majority-threshold-off-by-one.md) | Swafe (Nov 2025) | Medium → Medium |
| [HashMap iteration order breaks consensus determinism across off-chain nodes](contests/swafe-hashmap-consensus-nondeterminism.md) | Swafe (Nov 2025) | High → Medium |

Both submissions were judged valid and grouped by the judges into a single finding, [M-03](https://code4rena.com/reports/2025-11-swafe), reported by 24 wardens in total.

The grouping is right, and the reason is an asymmetry. The threshold admits an exact tie on even node counts; the `HashMap` then resolves that tie differently on every node. Fixing the threshold removes the tie, and with it the divergence. A deterministic tie-break alone would make the nodes agree, but a coalition holding exactly half of them would still win. The threshold is the root cause; the divergence is its consequence.

## Proofs of concept

Standalone, no project dependencies. Verified against `rustc 1.75.0`.

```
rustc --test --edition 2021 poc/hashmap_nondeterminism.rs -o /tmp/poc && /tmp/poc
rustc --test --edition 2021 poc/majority_threshold.rs -o /tmp/poc && /tmp/poc
```

- [`poc/hashmap_nondeterminism.rs`](poc/hashmap_nondeterminism.rs) — asserts that identical input produces two different winners across runs, and that `BTreeMap` collapses it to one.
- [`poc/majority_threshold.rs`](poc/majority_threshold.rs) — asserts the threshold equals exactly half on even node counts, is correct on odd ones, and that two candidates can qualify simultaneously.
- [`poc/determinism_probe.rs`](poc/determinism_probe.rs) — reusable harness for the class above. Runs a state transition repeatedly against fresh state and counts distinct result digests, the way validators compare state. Includes a demonstration that a full set of functional assertions passes on non-deterministic code.
