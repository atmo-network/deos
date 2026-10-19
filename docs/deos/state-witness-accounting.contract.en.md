# State Witness and Resource Accounting Contract

## Purpose and Ownership

This contract defines the project-wide distinction between logical execution, content-addressed state witnesses, measured proof increments and charged resources. It applies to DEOS runtime composition, pallet benchmarks, storage-layout decisions, performance reports and client explanations. RFC 2119/RFC 8174 uppercase key words are normative.

**Core principle: count repeated work by its execution owner; count required witness payloads by their distinct content identities within an explicitly bound scope. Neither count is a price for the other.**

This document owns that principle, vocabulary, matching-unit rules and design/review obligations. The [Actors resource policy](actors-resource-policy.specification.en.md) retains block ceilings, frozen allocations, admission and settlement; the [performance-assurance specification](actors-performance-assurance.specification.en.md) retains concrete workload/collector gates. Pallet specifications own semantics; code/tests own behavior; package architecture and DEOS integration documents own implementation mappings. No rule here installs a coefficient, activates reclaim, changes a fee or certifies capacity.

## Resource Coordinates

Every report or comparison MUST name its coordinate rather than use “cost”, “memory”, “proof” or “actual” without qualification.

| Coordinate | Unit / scope | Does not establish |
| --- | --- | --- |
| Logical work | Gets, decodes, predicate visits, encodes, puts; repeated per execution | CPU time or unique backend input |
| Logical value traffic | Encoded bytes returned/submitted across those operations | Physical I/O, copies, peak RAM or proof size |
| Required original payload mass | Distinct necessary external-value bytes at the declared original root(s) | Complete encoded proof, recorder increment or compressed PoV |
| Raw / compact state proof | Bytes in the named codec and witness scope | Equal byte length in another representation |
| Recorder increment | End minus start counter under the collector's exact semantics | A fresh complete witness or all block input |
| Database counters | Named SDK/backend counter semantics, tracking and verification mode | All logical accesses or physical database traffic |
| Net state delta | Final changed keys and values | Intermediate work, original inputs or trie repair |
| RefTime observation | Time in the named body/process/worker boundary | A conservative complete production owner |
| Declared / settled Weight | Independent RefTime and ProofSize components | Measured CPU, RAM or encoded network PoV |
| Runtime memory | Peak/live allocations, allocator and host scope | ProofSize or storage-value traffic |
| Network PoV | Named block-data/proof framing and compression boundary | Equality with any one charged Weight component |

Exact integers MUST survive collection and transformation. SDK u128 timing literals require integer-preserving handling; proof counters require their declared width. Floating-point conversion, overflow, saturation or fallback MUST NOT silently strengthen a bound.

## Content-Addressed Witness Sharing

A state witness supplies enough original information to authenticate the required reads and perform the claimed transition against its root. It is not a log containing one copy of every value for every access.

For a trie layout with separately hashed external values, two different storage keys can reference byte-identical encoded values with the same content hash. A compatible recorder/proof representation can retain one external payload for both references. The proof still needs the relevant key paths and authentication relationships. Shared payloads MUST NOT merge domains, owners, authorization, lifecycle, counters or storage-key identity.

```text
key/domain 1 -- authenticated path --> hash H --+
                                               +--> one encoded payload in the witness
key/domain 2 -- authenticated path --> hash H --+
```

Equality means exact encoded bytes under the selected format, not equal lengths, equal members in another order or equivalent application meaning. Different encoded values normally have different hashes under the declared collision-resistance assumption. They require their own payloads when full original values are needed, although their key paths and other nodes can still share structure.

This is not a promise that every host database deduplicates physical storage or that runtime objects share allocations. Proof-node retention by content identity, key-level recording and database-cache behavior are separate mechanisms. The actual layout/recorder/codec MUST support the claimed sharing; large-value behavior MUST NOT be generalized to inline values or another backend without checking it.

## Monolithic Values and Near-Equal Data

A large SCALE vector stored as one external value is opaque to this sharing mechanism. Changing one ID changes that value's content identity; a full original-value read can require the complete new vector payload, not just a four-byte patch. This is not per-element Merkleization or a cross-value delta codec.

For illustration, 16,384 four-byte IDs occupy 65,540 SCALE bytes including the compact length prefix. Two keys containing the same vector can require one such payload; equal-length vectors differing in one ID can require two. Those numbers describe selected payloads, not total proof, network PoV, RAM, CPU or an unconditional domain cap. The [test mapping](#executable-falsifiers) binds this example to executable checks.

An optimization argument MUST consider full equality, near equality and genuinely distinct data. It MUST NOT rely on accidental equality in a fixture unless protocol semantics guarantee that equality throughout the claimed input domain. Artificially synchronizing unrelated domain lists or omitting real peers to obtain a smaller proof is nonconforming.

## Scope, Root and Input Lineage

Every sharing or proof-bound claim MUST identify the base root(s), key/value layout, hash and proof codec, recorder lifetime, prior retained inputs, overlay lineage, included phases and relevant host/binary bindings. A cache hit is not by itself permission to omit data that the verifier still needs.

Equality of payloads does not authenticate their association with another key or root. Reuse across roots requires valid paths and an explicit input-lineage/retention argument; a recorder reset, new process or changed root MUST NOT inherit a discount merely because a previous run read equal bytes. Conversely, valid multi-root proofs can share identical content when their representation and authenticated paths support it.

Original backend values, values supplied by an earlier prefix and values rewritten in the overlay MUST remain distinguishable. After one original index read and put, later reads of its shrinking overlay version may perform full decode/filter/encode work without requiring a new original payload each time. Source values MUST NOT be rebound to an overlay cursor, later snapshot or newly computed trie node.

A write-only/delete path does not necessarily decode the removed record. It can nevertheless require original paths, absence information, siblings or deletion repair during root construction. Missing/absent values are not inherently free. Sorted delta execution does not eliminate those dependencies. Full transition witnesses MUST include the required original repair inputs, not present new output nodes as old input.

## Distinct Payload Mass and Matching Units

Let U be the set of distinct content hashes of external original values required in full, and v_h the corresponding encoded payload bytes. Under a collision-free consistent identity mapping:

```text
P = sum(len(v_h) for h in U)
```

P is materialized required payload mass. Counting each domain copy or each repeated access instead is wrong for this coordinate. For one fixed snapshot with one original value per queried key, the number of these classes is at most the number of such keys; this is not a bound on every node or value required by the complete workflow.

For a standalone uncompressed representation that must carry every required payload verbatim, their contribution is at least P; paths, nodes and framing add their own costs. A claim about an encoded compact proof MUST check that codec's treatment of external values. Compression, an external dictionary or a different inclusion boundary does not inherit this numerical floor automatically. Payload mass alone is never a complete-witness upper bound.

Raw proof, compact proof, a recorder's estimated/encoded size, recorded increment, declared ProofSize and network PoV are not interchangeable. Comparisons to a budget MUST prove that the budget bounds the same unit, scope and representation. A conditional witness mass exceeding a number does not establish public-state reachability, production impossibility or a new population/retention cap.

## Baseline and Recorder Increment

If a collector reports D=max(E−B,0), with complete end value E and start value B in the same valid unit, and E≥P is independently justified, only this implication follows:

```text
D >= max(P - B, 0)
```

It does not imply D≥P without an adequate baseline argument. Already recorded payloads can contribute zero new bytes to a later interval. The incremental byte count is also not generally the encoded size of a set difference, because encoding and framing have their own behavior.

Collectors MUST distinguish database discounts, whitelist prefill, recorder contents, read/write-counter reset, commit/reopen and root reset. A whitelist may affect counters or seed the start proof rather than make the witness intrinsically whitelist-aware. Neither an empty whitelist nor a counter reset alone proves B=0. Empty/fallback representations, width limits, missing recording and saturation MUST be classified before transferring any floor or credit.

Reports MUST say whether zero baseline was directly observed, established under source/host premises or remains unknown. A trace field missing from the selected dispatcher MUST NOT be invented from another SDK path. Unknown evidence narrows the claim rather than silently selecting the most favorable interpretation.

## Repeated Execution, Failure and Verification

Witness sharing does not remove per-key or per-operation work. Separate reads may still decode, filter and encode separate vectors; equality of proof payloads does not prove shared runtime buffers or CPU savings. Equal input counts do not imply equal physical time, and more unique payloads do not by themselves prove monotonic elapsed time.

Source operation bounds are inputs to a physical model, not accepted RefTime coefficients. Calibration requires a supported candidate family, applicable source/host transfer, collector boundary and falsifier; accepted numerical coefficients need not pre-exist the experiment. Raw maxima, modeled DB terms or a lower byte count alone MUST NOT become a conservative complete price.

A failed or rolled-back transition can incur work despite having no net state change. Proof retention through rollback depends on the actual recorder/transaction implementation. No generic zero-cost or retained-proof credit follows from rollback itself; failure, refusal, cleanup and retry paths keep explicit cost owners.

Setup, useful body, recording stop, optional verification, commit/root and persistence/validation-host work MUST be separately identified. Strict verification MUST remain effective when requested and excluded from optional-off footprints. Generated macro behavior, including unconditional Result tails, must be checked through the actual caller. A stop-callback snapshot, ordinary SDK returned row and whole-block/worker observation MUST NOT substitute for each other.

## Admission, Settlement and Fees

Safe admission MUST retain the applicable conservative maximum in both Weight axes until its owner authorizes settlement. Expected sharing, likely cache warmth, common Actor Contract shape or a small observed proof MUST NOT reduce that maximum without complete qualification. Uncertain sharing is no budget credit.

Naturally shared input can be accounted once within the owning full-workflow composition. Replacing an inclusive owner requires retiring its contained declarations, not adding the inclusive measurement on top. Safe upper-bound sums MAY deliberately overapproximate sharing; an optimization must justify a tighter owner rather than label every conservative sum an implementation defect.

Any actual-proof settlement/reclaim MUST use the existing qualified accounting authority, preserve still-outstanding work and avoid a second credit for the same saving. It MUST NOT create another allocator, rescan global state, expand a frozen budget or charge a new on-chain hash-class census solely to observe sharing. The [resource policy](actors-resource-policy.specification.en.md) remains authoritative for these decisions.

Resource observations do not change Actor fee ownership, payer rights, custody or governance authority. Sharing alone grants no fee refund, free Action, free destruction or permission to bypass Q1, B+1, FIFO or scheduler order. Batching/reordering independent work merely to improve sharing requires its own conforming architecture decision; it is not authorized here.

## Practical Design Rules

- Reuse naturally common canonical data where existing semantics already permit it; do not add a second proof-deduplication subsystem for behavior the SDK already supplies.
- Preserve stable canonical encoding, but do not alter domain meaning or add aliases merely to manufacture byte equality.
- Keep logical traffic and unique original inputs as independent model operands; a class count can be bounded by another operand only with an explicit conservative argument, not hidden in an unqualified coefficient.
- Evaluate immutable shared payloads versus owner-specific mutable data only when an actual use case earns the split. References, lifecycle/refcounts, access checks, cleanup and extra storage/proof costs remain owned.
- Treat paging/chunking or reverse-index changes as hypotheses, not a consequence of deduplication. They help only if the workload can reach the needed parts without replacing one full scan with another; measure the complete owner and both axes before retention.
- Preserve the smallest correct baseline and existing public capabilities; no artificial genesis funding, chronology, caps or weakened guards may repair an unfavorable resource result.
- Describe state roots and canonical values separately from materialized proof measurements and derived estimates. Clients MUST NOT present a measured or estimated witness as authoritative chain pricing or a RAM limit; the [read-model contract](read-model.contract.en.md) owns provenance classification.

## Review and Evidence Checklist

| Review question | Required evidence / smallest falsifier |
| --- | --- |
| Are the keys independent but payloads shared? | Actual distinct keys/paths, equal encoded values, one retained external payload and successful reads |
| Does one changed element create another class? | Equal-length near-distinct values, exact hashes and both required payloads |
| Are both shared uses authenticated? | Remove the shared payload: all its dependent reads fail; unaffected classes still work |
| Was logical work confused with sharing? | Keep per-domain decode/filter/encode or visit counts independent of unique payload count |
| Is the coordinate really an increment? | Bind actual start/end/root/reset/codec behavior; reject an unjustified full-mass floor |
| Is overlay input being called original? | Verify original first read, later overlay sequence and noninterference of earlier phases |
| Is transition/root work complete? | Replay original reads plus whole mixed delta/repair to the independent post-state root |
| Do verification and returned counters agree? | Test optional false/true behavior and the actual ordinary dispatcher return, not only stop |
| Does the fixture represent the claimed domain? | Real identities, records and lifecycle; state conformance separate from public ancestry |
| Is the physical/pricing claim stronger than the data? | Declare both-axis complete owner, applicable host/model and undercoverage falsifier; reject otherwise |

These are claim-directed obligations, not an instruction to rerun every closed corpus, create a new harness or require the most expensive gate for unrelated changes. Reuse unchanged semantic/source evidence; refresh only the affected coordinate. Optional missing evidence narrows capability; a missing required pricing layer fails closed.

## Executable Falsifiers

The following project-owned checks in [Governance runtime integration tests](../../template/runtime/src/tests/governance_integration_tests.rs) anchor the mechanism and its boundaries. They are executable behavior evidence, not production-price certificates or private Skill dependencies.

| Test symbol | Falsifies |
| --- | --- |
| `cleanup_equal_domain_indices_share_proof_value_not_logical_work` | Shared/near-distinct keys, raw/compact decoding, absence guards or unchanged per-domain visits regressing |
| `cleanup_required_external_value_mass_counts_hash_classes_not_domains` | Counting domain copies instead of required hash classes, or accepting missing required values |
| `cleanup_index_logical_work_bound_covers_domain_and_population_partitions` | Incorrect shrinking-vector/SCALE logical-work bounds |
| `independent_full_indices_original_input_replays_whole_delta` | Read-input/repair lineage failing to reconstruct the exact mixed transition |
| `full_index_value_classes_reach_native_sdk_body_recorder` | Prepared full-state input not reaching the actual native body recorder |
| `full_index_value_classes_v2_records_absolute_body_before_verification` | Incorrect start/stop/whitelist/payload ownership on the generated v2 boundary |
| `full_index_value_classes_sdk_return_separates_optional_verification` | Returning optional-verifier reads as the false native footprint, or changing saved proof through later verification |

The three `verification_switch_tests` in [Governance benchmarking](../../template/pallets/governance/src/benchmarking.rs) additionally require post-stop corruption to be skipped at verify=false and refused at true. SDK recorder/API/reset and root-inclusive controls retain their own test/implementation owners. Historical observations MUST remain bound to the source/artifact/scope executed; these links do not promote all native or prepared checks to default-Wasm, admitted-history or host-maximum evidence.
