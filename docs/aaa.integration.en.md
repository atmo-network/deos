# AAA Integration in DEOS

## Purpose and Ownership

This document maps how the DEOS reference runtime composes reusable `pallet-deos-aaa` with deterministic System identities, TMCTOL execution-plan families, DEOS Router, Oracle, assets, staking, fee collection, XCM, governance, generated weights, and browser/control-plane surfaces.

The portable actor contract and crate implementation remain in [`template/pallets/aaa/docs/specification.en.md`](../template/pallets/aaa/docs/specification.en.md), [`template/pallets/aaa/docs/architecture.en.md`](../template/pallets/aaa/docs/architecture.en.md), and [`template/pallets/aaa/docs/embedding.md`](../template/pallets/aaa/docs/embedding.md). This document owns only concrete DEOS composition.

## Integration Code Map

| Surface | Anchor |
| --- | --- |
| Runtime adapters, actor builders, bounds, and origins | `template/runtime/src/configs/aaa_config.rs` |
| Runtime-generated AAA weights | `template/runtime/src/weights/pallet_aaa.rs` |
| DEOS Oracle publication hook | `template/runtime/src/configs/oracle_config.rs` |
| Router fee, quote, execution, and observation composition | `template/runtime/src/configs/axial_router_config.rs` |
| Asset and transaction-extension ingress | `template/runtime/src/configs/assets_config.rs`, `template/runtime/src/lib.rs` |
| Genesis System identities and ED anchors | `template/runtime/src/genesis_config_presets.rs` |
| Runtime integration and load evidence | `template/runtime/src/tests/aaa_integration_tests.rs`, `template/runtime/src/tests/load_testing.rs` |
| Off-chain artifacts and simulation | `docs/aaa-control-plane.contract.en.md`, `web-client/src/lib/automation/` |

## Namespace and Sovereign Accounts

The runtime binds package `pallet-deos-aaa`, Rust crate `pallet_aaa`, and `AaaPalletId = *b"aaactor0"`. The pallet account is `PalletId(*b"aaactor0").into_account_truncating()` under `AccountId32` and SS58 prefix `42`.

User actors derive sovereign accounts from `(PalletId, owner, owner_slot)`. System actors derive them from `(PalletId, "system", aaa_id)`. System actors consume no owner slot; the compatibility creation-event field remains sentinel `0`.

The complete DEOS deterministic System account map follows.

| aaa_id | Role or account | Hex | SS58 |
| ---: | --- | --- | --- |
| — | AAA pallet account | `0x6d6f646c61616163746f72300000000000000000000000000000000000000000` | `5EYCAe5fiK3ZpinaPEDXwvtT6tFp5gBL16S5vyt4TYmgLaT1` |
| 0 | Burn Actor | `0xeba61f8494ba498cb84ce3b771bc3c193dbd82f9a999153a55c383349f6e512e` | `5HPgTa8GLrmzMDktPEWmuC82WtipKSibwd9C2pUQnESn4nAv` |
| 1 | Fee Sink | `0xab373631522954b038699419fadc732893dff1230239bc30fbe17bf5fb12f084` | `5FwCSs6WuW2tTv7uQFRB1o4rjmPQsgE6PesjKUUbroxfzKKh` |
| 2 | Liquidity Actor | `0xb136dc3f6dba4aac24a8c9f8be3c7b20e26b08422803b6999b7cd019c4ca50ab` | `5G54dUVans8Rvnn1qdTea3fQ28osh8T7ijaWbi3gygm9sa7C` |
| 3 | TOL Bucket A | `0x6f9a5aa8cd9ba27b2e69f1bac1c521d2ffde543275ebd787da11dbd131c50d25` | `5Eb32Qkj9FpPMUXZMNreJzRESQRbYQWwiKXK4zf9VXifTEqX` |
| 4 | TOL Bucket B | `0x03699bb4549d77d91390fc161867ccd3ef97d4f305f01757708905c84cb7d882` | `5C9BNb4AoxDngwC6nzu8SEtAEbtGHiKeBjzJwgUewA9qDNL3` |
| 5 | TOL Bucket C | `0x313e7fb07ed6681741b54c3d421f8c261027048e2a9b0668e1058654d369de29` | `5DBGmawvmUvHAg9e2A4bcwZm3NiGX5KE5sPCKepN36SMJvfX` |
| 6 | TOL Bucket D | `0xd23baab9890a6990ff23e7ad7ab9d1ad34712d7add2344917d110e3cec5b9242` | `5GpMdwY6iMiA8LRUczsZH6p9WoxN4rX15U7FJWbeqTqTrPLX` |
| 7 | Treasury B | `0xa027809984f38031e61246efe8ad1f28ddacd9870f6bed081560089c15f9b966` | `5FghFeZDxtGWmvASpM4etxnYtreW9yamSx1Pwh1aGYkny2uv` |
| 8 | Treasury C | `0xcae77c85e5665e0cbe994898429478d3facf4c29a9b7539902f95ad7b3b4bf9b` | `5GekJ6zNwu6ABqhpcagnxbPmP6UtJ1gUKdvJywZKugWkCLhe` |
| 9 | Treasury D | `0xc81b0eb40aea260eb09b950cfbe2c43f9be1dc73bf62cf081c376cff4bdae0ca` | `5Gb5UKWyYyyttHG3GCsyEhN2Qtb92auewWLZzPaQCvp1RHaj` |
| 10 | BLDR Splitter | `0x8a420d09aa8842c9075deefab7791be5e9f9471bc68baa8c926128cfc29b6962` | `5FBz5y9kWN7ArW1w5TZiCLbszGmG3FmCSx6njj9w7VEuiK8N` |
| 11 | BLDR Liquidity Actor | `0x6324e98949d19dbe10162a939df82b28368bef743a14aa8ce0a3d9a02d567221` | `5EJhZc6rdqBKzZcJXfjeMwTaQvYsyTF9YJS39sWr1HEuEy17` |
| 12 | BLDR Bucket A | `0xb31a379c50afe1ba1ad65f1afafaf51df1c40ed2b6c08e9faf1a1ac2caf026de` | `5G7YDX7r2L8q5Wn73dNyhp8cnbpP3sTGUcRW6Eos5Urrxax8` |
| 13 | BLDR Treasury | `0x3a1bedf666c4852432a75dc0099fec586a02b813acb4457c9d4b150a03bdce45` | `5DNtvy5YymuvPBM6Wk8ADHs9ggLK2gjEZoaSoeM3aHLykNKG` |
| 14 | Native staking LP provisioning actor | `0xbb27f4956462189d16c7f9e207222ce9691308c6a55bb0141f139ebe071394d2` | `5GJ6gSae5dZhxJm6EuD82gaxiLkvokMeLFMNmtuSz8htoidu` |

## Genesis Topology

| Lane | Role | aaa_id | Genesis lifecycle |
| --- | --- | ---: | --- |
| Core | Burn Actor | 0 | Active burn plan |
| Core | Fee Sink | 1 | Active phase-one fee allocation |
| Core | Liquidity Actor | 2 | Dormant |
| TOL | Bucket A | 3 | Custody-only |
| TOL | Buckets B/C/D | 4–6 | Dormant |
| Treasury | Treasuries B/C/D | 7–9 | Dormant |
| BLDR | BLDR Splitter | 10 | Active 50/50 split |
| BLDR | BLDR Liquidity Actor | 11 | Dormant |
| BLDR | BLDR Bucket A | 12 | Custody-only |
| BLDR | BLDR Treasury | 13 | Dormant |
| Staking | Native staking LP provisioning actor | 14 | Dormant |

Active genesis actors use the runtime System cooldown, `AaaType::System`, `Mutability::Mutable`, and no schedule window. Dormant entries occupy `DormantAaaIdentities` and `SovereignIndex` without hot program, queue, wakeup, funding, fee, or cycle state. Custody-only accounts occupy neither identity store.

`ActorIdentityCount` covers thirteen active plus dormant identities. `NextAaaId = 15` preserves the reserved address range. Every expected small-native-flow System or custody account receives one persistent free-balance ED anchor because a provider or reserved balance alone does not make a zero-free account eligible for sub-ED native ingress under `pallet-balances` v50.

## Execution-Plan Families

The runtime keeps TMCTOL policy declarative through builders in `aaa_config.rs`.

| Builder | Actor family | Composition |
| --- | --- | --- |
| `build_burn_execution_plan` | Burn Actor | Foreign balances → Native swap → burn |
| `build_zap_execution_plan` | Liquidity Actor | Add LP → surplus swap → split LP to buckets |
| `build_bucket_lp_transfer_execution_plan` | Buckets B/C/D | Transfer bounded LP fraction to paired Treasury |
| `build_treasury_lp_unwind_execution_plan` | Treasuries B/C/D | Remove configured LP into Treasury custody |
| `build_bldr_splitter_execution_plan` | BLDR Splitter | Split minted BLDR share between liquidity and treasury lanes |
| `build_bldr_zm_execution_plan` | BLDR Liquidity Actor | Add NTVE/BLDR liquidity → transfer LP to BLDR Bucket A |
| `build_treasury_b_buyback_execution_plan` | Treasury B | Optional NTVE buyback → burn acquired target |
| `build_native_staking_lp_farming_execution_plan` | Staking LP actor | Donate balanced `NTVE/stNTVE` without minting LP |

These builders configure the reusable task language; they do not create pallet-level roles or AAA-id policy branches.

## Governance Activation Flows

`Foreign asset + TOL lane`: register the foreign asset, create the Native/foreign pool, extend the Burn Actor, activate the Liquidity Actor, then optionally activate paired Bucket transfer and Treasury unwind plans.

`BLDR lane`: retain the BLDR Splitter at genesis, create the NTVE/BLDR pool, activate the BLDR Liquidity Actor, then optionally activate Treasury buyback/burn policy.

`Native staking LP lane`: register native staking, initialize `stNTVE`, create and seed the AMM, then call `activate_native_staking_lp_farming`. Activation fails until receipt asset, staking pool, actor, and nonempty AMM all exist.

Emergency policy pauses one actor through `pause_aaa` or stops cycle execution globally through the circuit breaker while bounded bookkeeping remains active.

## Market Adapter Composition

`TmctolDexOps` routes exact-input and exact-output swaps through DEOS Router with `ExecutionContext { actor, aaa_type }`. AAA supplies immutable actor authority; the adapter uses it only for typed market protection and never infers System status from the sovereign catalog.

Exact input derives `min_out` from the caller-aware quote and binds zero tolerance to that quote. Exact output obtains one reverse quote, adds authored tolerance with ceiling arithmetic, intersects it with live preservable input capacity, and executes under the explicit total-input cap.

DEOS Router evaluates the direct XYK candidate and at most one reverse-quoted Native-anchored path, selecting minimum required input. TMC remains exact-input only because it exposes no exact-recipient-output execution contract.

System swaps read the exact directional Oracle feed with `MAX_SYSTEM_REFERENCE_AGE_BLOCKS = 100` and enforce `AaaMaxSystemPriceDeviation = 5%`. Fresh nonzero truth at the exact age boundary remains eligible. Unavailable, Uninitialized, Stale, or invalid truth falls back to direct reserves; unavailable fallback or excessive deviation fails Temporary before mutation.

User swaps retain Router's ordinary direct-pair guard and do not fail solely because the standalone Oracle feed is absent or uninitialized. Native-anchored System routes without a pair reference fail closed.

The guard bounds authored execution loss; it does not prove external fair price, ordering safety, manipulation resistance, or MEV immunity.

## Integration Boundary

AAA invokes assets, swaps, liquidity, staking, fee collection, and direct ingress only through runtime adapters. Concrete ledger semantics, Router route selection, pool mechanics, staking representation, and fee destinations remain outside the pallet package.

Task-scoped storage transactions preserve committed earlier steps while rolling back a failing task's local effects. Runtime adapters classify only explicit Temporary market or infrastructure failures as retryable; unknown downstream errors remain Permanent.

## Runtime Adapter Bindings

`DeosFundingAuthority` receives only `RuntimePolicy` decisions after pallet-owned source-policy evaluation and defaults deny because the launch matrix authorizes no actor/source pair.

`TmctolAssetOps` maps Native to `pallet-balances` and Local/Foreign to `pallet-assets`. Its transfer preflight covers source withdrawal and recipient deposit consequences. Ordered `SplitTransfer` legs all preflight before mutation; task rollback forbids partial fan-out.

`pallet-balances` v50 rejects a new zero-free account below ED even when FRAME already holds a provider. DEOS therefore endows expected small-flow System, custody, and staking-ingress accounts with one persistent free ED anchor and preserves it through amount resolution.

`TmctolLiquidityOps` delegates add/remove/donation to Asset Conversion while retaining ratio, LP receipt, and native-special-case policy in the adapter. `TmctolStakingOps` maps native staking to `stake_native`, other assets to generic staking, and resolves stable share assets through the staking receipt index.

Runtime adapters use typed failure classification. Explicit route, liquidity, slippage, oracle, and temporary-capacity failures may retry; malformed, forbidden, funding, fee, and unknown downstream failures remain Permanent.

## Address and Funding Ingress

All successful supported producers call the fallible direct ingress boundary in their originating transaction. They preflight before value movement and propagate notification failure; no event scan, compatibility ring, or deferred correctness layer exists.

| Producer family | DEOS integration |
| --- | --- |
| Signed Balances/Assets transfer | Transaction extension carries one bounded direct candidate and verified signer provenance |
| `transfer_all` | Candidate resolves actual movement from recipient balance delta |
| AAA Transfer/Mint | `TmctolAssetOps` submits sender or source-less typed ingress inside task execution |
| TMC distribution | Mint-output adapter submits once and preserves available source provenance |
| Router fee routing | Fee adapter submits once with fee-payer provenance |
| XCM asset deposit | `AaaAwareAssetTransactor` submits one converted or source-less candidate |
| Privileged/delegated producers | Direct source-less candidate preserves signal delivery but remains balance-only |

XCM binds generated one-asset deposit weight and `MaxAssetsIntoHolding = 1`, preventing one instruction from multiplying synchronous AAA ingress work without a corresponding instruction-specific weigher.

User actors default to `OwnerOnly`; accepted verified owner or allowlist transfers may update funding batches. System actors default to denied `RuntimePolicy`. Source-less or rejected provenance still creates spendable ledger balance but does not gain tracked funding authority.

## Fee Composition

AAA fee admission derives each task and complete-plan reserve from runtime-generated task weights, `WeightToFee`, and configured step fees. One selected fee charges per executed or failed task attempt; skipped steps charge evaluation only.

`TmctolFeeCollector` transfers the complete charge into Fee Sink System AAA `1`. Collection does not depend on later Fee Sink execution and never pays the current author directly. The Fee Sink's current phase-one plan allocates available Native value equally between staking ingress and native liquidity provisioning while retaining indivisible remainder.

Pure lifecycle cleanup charges no execution fee and runs no plan. User fee admission reserves transient Native fees without consuming the persistent sovereign ED anchor.

## Runtime Bounds and Block Budget

| Bound | DEOS value and role |
| --- | --- |
| `MaxActiveActors` | 10,000 compile-time identities |
| `ActiveActorLimit` | Governance operational cap, never above actor or queue hard capacity |
| `QueuePageSize` | 64 active FIFO entries per physical page |
| `WakeupPageSize` | 32 temporal entries per page |
| `MaxQueueEntriesScannedPerBlock` | 10,000 physical inspections |
| `MaxExecutionsPerBlock` | 1,000 defense-in-depth attempt ceiling |
| `MaxUserExecutionPlanSteps` | 3 |
| `MaxSystemExecutionPlanSteps` | 10 |
| `MaxConsecutiveFailures` | 10 |
| `MaxAutoCloseNonceHorizon` | 10,000 |

Runtime block policy assigns 50% to dispatch and 50% guaranteed `on_idle` headroom. No dedicated Operational reserve exists while no concrete critical Operational call consumes one. `GuaranteedOnIdleWeight` binds directly to `1,000,000,000,000 / 2,500,000`.

System and User execution shares each reserve 20% of the actor-execution phase. Housekeeping, observation fanout, wakeup work, cleanup, and admission bookkeeping remain outside those service shares.

Creation, activation, reopening, and plan replacement reject any actor whose scheduler, complete attempt, and pure cleanup do not fit the guaranteed two-dimensional envelope.

## Reactive Delivery Evidence

The runtime binds `MaxObservationFanoutPagesPerBlock = 64` and `ObservationFanoutWeightLimit = 400,000,000,000 / 1,000,000`. Production ProofSize admits five worst-case units after base admission.

| Production topology after publication | Charged units | Blocks at five units |
| --- | ---: | ---: |
| One feed with 10,000 subscribers | 157 | 32 |
| One sparse subscriber at a historical high page id | 1 | 1 |
| Four dirty feeds with 10,000 subscribers each | 628 | 126 |
| One revision restart after quiescence | At most 314 | 63 |
| Persistently saturated queue | Unbounded | Unbounded |

The finite rows require quiescent revisions, available configured budget, and eventual queue capacity. They end at fanout completion, not condition evaluation or actor attempt.

Production fanout base is `31,565,000 / 1,543`; the completing dense unit is `10,082,557,000 / 172,190`. A saturated dense diagnostic is lower in RefTime and equal in ProofSize, so one conservative unit class remains sufficient.

## Generated Evidence and Artifacts

Accepted production identities:

| Artifact | SHA-256 |
| --- | --- |
| AAA runtime weights | `6688fe062147259f42666b477b5d1bfb43a339eb8ba2f14cd842352b4f631728` |
| DEOS Oracle runtime weights | `97d87f3d2bfd1fe43e2df3c86af07b13283a72a40de5862d676592eb863d658a` |
| Compact runtime Wasm | `d18815e9ca4e02918626274af822e09e7d8d5465c87a95d605f6725dd6dc01e6` |
| V16 metadata | `b6b7c3c2b2f55f64d8b15e5dfcc4f08cad6b645526d30e44b3a1f249f6ec688e` |
| AAA semantic manifest | `f7b952b6785d4c7e668a16b36fc92ce593d121424c2de319726756034e6bb190` |

`template/runtime/src/weights/pallet_aaa.rs` owns complete generated methods and storage annotations. Architecture records only load-bearing admission values and accepted identities; benchmark-host timing never becomes a chain-throughput claim.

## Control Plane and Read Surfaces

Canonical active projection joins `ActorHot` and `ActorProgram` at one finalized block; funding details require the separately bounded `ActorFunding` value. Dormant identity, queue/wakeup membership, active-dirty topology, and bounded simulation results remain canonical-chain truth.

The browser's authoring, artifact, matching-Wasm, simulation, observation, and governance-composition surfaces live under `web-client/src/lib/automation/` and `web-client/src/lib/observation/`. They bind metadata and runtime identity rather than recreating pallet semantics.

Unbounded history, archive search, forecasting records, governance preparation history, and longitudinal telemetry remain materialized-provider work under [`aaa-control-plane.contract.en.md`](./aaa-control-plane.contract.en.md) and [`read-model.contract.en.md`](./read-model.contract.en.md).

## Validation and Operations

Package tests own executable actor, scheduler, trigger, lifecycle, storage, and try-state behavior. Runtime tests own adapters, fees, ingress, genesis topology, Oracle/Router rollback, staking, XCM, generated-weight binding, block-budget partition, and full System/User composition.

The full `scripts/aaa-release-gate.sh` route covers package portability, external embedding, runtime integration, scheduler fairness, dense/sparse liveness, 10,000-actor queue stress, and the occupancy profile. The canonical script owns command syntax and profile selection.

Operational observation uses typed lane heads/tails, bounded occupancy, active limit, wakeup cursor/buckets, actor-local queue and wakeup pointers, `CycleDeferred`, starvation detection/recovery events, and sweep events. These surfaces expose pressure without promising an exact future execution block.

The zombie-spam regression compares the complete actor-cap creation-cost floor with bounded permissionless sweep fees. Governance changes to creation fee, fee conversion, or sweep bounds must preserve the measured dominance relation rather than relying on a copied ratio.

Package scheduler, trigger, lifecycle, storage, and extrinsic internals remain authoritative in the package architecture. This integration map owns only the concrete bindings and policies layered over those mechanisms.
