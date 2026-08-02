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

## Temporal Binding

The six-second DEOS slot binds `AaaMaxExecutionDelayBlocks` to `52_596_000`, exactly `ceil(10 × 365.25 days / 6 seconds)`. `ten_julian_year_horizon_matches_six_second_runtime_binding` derives the value from `SLOT_DURATION`; package boundary tests reject unrepresentable exact-next-block, cooldown, cadence-plus-jitter, retry, and window-terminal targets without mutation.

## Namespace and Sovereign Accounts

The runtime binds package `pallet-deos-aaa`, Rust crate `pallet_aaa`, and `AaaPalletId = *b"aaactor0"`. The pallet account is `PalletId(*b"aaactor0").into_account_truncating()` under `AccountId32` and SS58 prefix `42`.

User actors derive sovereign accounts from `(PalletId, owner, owner_slot)`. System actors derive them from `(PalletId, "system", sovereign_id)`; `ActorClass::System { sovereign_id }` carries that custody locator independently from the actor-id key. Fresh creation assigns the new actor id as a new locator. `SystemSovereigns` retains every allocated locator as `Vacant | Occupied(aaa_id)`; governance may attach a fresh identity to a vacant locator without changing its account or residual balances. `AaaCreated.actor_class` carries `ActorClass::User { owner_slot }` or `ActorClass::System { sovereign_id }`; no separate event field duplicates either value.

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

Active genesis actors use the runtime System cooldown, `AaaType::System`, `Mutability::Mutable`, and no schedule window. Dormant entries occupy `ActorIdentities` and `SovereignIndex` without hot program, queue, wakeup, funding, fee, or Active-epoch state. Custody-only accounts occupy no actor identity.

The reference runtime configures no System Immutable actor, so `0.7.11` has no actor-specific emergency migration or custody disposition to execute. A downstream runtime that admits an indefinite System Immutable actor must ship its migration-specific source/target actor set, bounded Close or Deactivate disposition, custody handling, terminal invariant, and Continuation policy with the same upgrade. The ordinary DEOS Governance path exposes 3-day lead-in, 7-day vote, 7-day protection, and 3-day enactment delay—20 days before bounded maturity/operational delay. Protocol `L1RootAction` can use the separately governed 24-hour urgent path only with unanimous raw protection-track `Pass`; AAA promises neither path completes within a finite time.

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

User actors default to `OwnerOnly`; accepted verified owner or allowlist transfers may add to bounded funding accumulators. System actors default to denied `RuntimePolicy`. Source-less or rejected provenance still creates spendable ledger balance but does not gain tracked funding authority.

### Crediting-Producer Inventory

Every producer path that can credit an AAA sovereign account uses one paired preflight/notify transaction through `RuntimeAddressEventIngress`. The inventory below names each path, its credited surface, source/provenance semantics, preflight owner, notification owner, rollback witness, and Weight owner; all movement is reverted on preflight or notification failure in the same transaction.

**Ingress producers (credit another actor's sovereign):**

| Producer path | Credited surface | Source / provenance | Preflight owner | Notify owner | Rollback witness | Weight owner |
| --- | --- | --- | --- | --- | --- | --- |
| Signed Balances/Assets transfer | Recipient sovereign | Signer / `Signed` | `TransactionExtension` candidate preflight | `post_dispatch_details` notify | Balances/Assets ledger revert | `transaction_extension_ingress_base` + `_notify` |
| `transfer_all` | Recipient sovereign | Signer / `Signed`, actual recipient delta | Same candidate preflight | Same notify | Balances ledger revert | Same extension weights |
| AAA Transfer/Mint task | Task `to` sovereign | Sender or source-less / typed | `TmctolAssetOps::transfer` preflight | Same adapter `on_internal_inbound` | Asset ops transaction | `task_dex_*`/`task_transfer` generated weights |
| TMC collateral distribution | Collateral/minted recipients | Mint source / `InternalProtocol` | `before_collateral_transfer`/`before_sink_mint` | `after_distribution` | TMC distribution transaction | TMC distribution generated weights |
| Router fee routing | Burn Actor sovereign | Fee payer / `InternalProtocol` | `route_fee` preflight | Same `on_internal_inbound` | Router fee transaction | Router fee routing weights |
| XCM asset deposit | Recipient sovereign | XCM origin / `Xcm` | `preflight_xcm_inbound` | `on_xcm_inbound` | `AaaAwareAssetTransactor` deposit revert | One-asset deposit generated weight |
| XCM without origin | Recipient sovereign | Source-less / none | Implicit source-less preflight | `on_inbound_without_source` | Same deposit revert | Same one-asset weight |
| Privileged/delegated producers | Recipient sovereign | Source-less candidate / none | Direct source-less preflight | Same source-less notify | Producer transaction revert | Producer-supplied weights |
| FeeCollector | Fee Sink sovereign | Payer / `InternalProtocol` | `on_internal_inbound` preflight (window/zero/auth/tracked) | Same `on_internal_inbound` notify | Fee Sink transfer + ingress transaction revert | Fee collection generated weights |

**Same-actor task/adapter outputs (intentionally excluded from the ingress boundary):** each executes inside the actor's own cycle and resolves against the actor's own balances, so it does not signal the actor or update its funding. They are named explicitly rather than claiming "all producers are covered":

| Output path | Signals current actor | Can update funding | Rationale |
| --- | --- | --- | --- |
| Swap output (exact-in/exact-out) | No | No | Debited from the actor's own sovereign; `SwapExecuted` event carries factual deltas |
| AddLiquidity LP issuance | No | No | LP minted to the actor's own sovereign; factual `amount_a`/`amount_b`/`lp_minted` event |
| RemoveLiquidity outputs | No | No | Underlying assets returned to the actor's own sovereign |
| DonateLiquidity debits | No | No | Factual debits within caps, own-sovereign movement |
| Staking shares / yield | No | No | `StakeExecuted`/`UnstakeExecuted` on the actor's own position; yield bridges via adapter |
| Unstake outputs | No | No | Own-sovereign return |
| StopCycle | No | No | No economic effect |

Movement to a non-AAA recipient and a task transfer to the actor's own sovereign remain explicit exclusions: `resolve_aaa` returns `None` for non-sovereign recipients and the pallet's recipient validation rejects self-transfers with `SelfTransferNotAllowed`.

Fee collection is now an explicit certified producer to the configured Fee Sink (not an accidental side effect of `AssetOps::transfer`): it moves exactly the fee-native amount and submits one paired AddressEvent with payer source and typed internal-protocol provenance in the same transaction, latching Fee Sink readiness. The inventory is certified by `AAA_ADDRESS_EVENT_PRODUCER_INVENTORY`, the generated ingress runtime evidence (which scans the complete runtime-config Rust tree for typed-boundary bypasses), the paired-executive tests, and the independent embedding fixture; any new crediting path must register here before acceptance.

## Fee Composition

The package-owned `attempt_fee_envelope` derives each task and complete-plan reserve from runtime-generated task weights, `WeightToFee`, and configured step fees. `settle_attempt_fee_step` releases the selected reservation before charging evaluation-only or attempted-step fees. DEOS runtime tests consume that exact envelope rather than reconstructing fee arithmetic.

Package-generated `aaa-fee-envelope-vectors.json` constrains browser forecast fee policy across User/System suffixes, reservation release, rollback pricing, and the direct User fee-native floor. One selected fee charges per executed or failed task attempt; skipped steps charge evaluation only.

`TmctolFeeCollector` is the explicit certified fee-ingress producer: it transfers the complete charge into Fee Sink System AAA `1` via a ledger-only movement primitive (no generic transfer/transaction-extension ingress, no staking bridge) and submits exactly one paired AddressEvent (payer source, internal-protocol provenance) in the same transaction so the Fee Sink actor latches readiness. One charge yields exactly one preflight, one movement, and one notification; zero/no-op collection emits no ingress; failure rolls back movement and all AAA state. Collection never pays the current author directly and never recursively charges an AAA fee. The Fee Sink's current phase-one plan allocates available Native value equally between staking ingress and native liquidity provisioning while retaining indivisible remainder.

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
| `MaxExecutionPlanSteps` | Configurable shared `1..=255` bound; DEOS baseline is 8 for both classes |
| `MaxRetryAttempts` | 10 cursor-local unsuccessful attempts |
| `MaxConsecutiveFailures` | 10 |
| `MaxAutoCloseNonceHorizon` | 10,000 |
| `MaxCacheRevalidationUnitsPerBlock` | 16 count ceiling for one bounded worker pass |
| `CacheRevalidationNoAdmitDisposition` | `None` until a concrete migration names its disposition |

Runtime block policy assigns 50% to dispatch and 50% guaranteed `on_idle` headroom. No dedicated Operational reserve exists while no concrete critical Operational call consumes one. `AaaOnIdleReserve` binds directly to `1,000,000,000,000 / 2,500,000`.

Cache revalidation is a global execution gate independent of the public breaker: the reference runtime starts at fresh epoch `0` with every genesis actor stamped and no gate, and no cache-affecting change can activate because no migration-specific no-admit disposition is configured (spec 6.4). `cache_revalidation_units` is the sole generated revalidation Weight class: fixed pass orchestration plus one worst-case actor unit each, measured at `20,303,784 + 20,776,516 × n` RefTime and `3,591 + 8,677 × n` ProofSize with `2 + 3n` reads and `1 + n` writes. One maximum unit fits `ActorServiceReserve` after the maximum fixed workers in both Weight dimensions; the runtime test `one_maximum_cache_revalidation_unit_fits_actor_service_reserve` and the package try-state assertion prove the admission bound against the measured classes.

System and User actors share one paged FIFO with a single global ticket order; no actor class reserves an execution share, weight slice, or service right (spec 8.1.8). Housekeeping, observation fanout, wakeup work, cleanup, and admission bookkeeping remain outside the actor-service pass; within that pass, strict head-of-line order and the shared cutoff alone bound service.

Creation, activation, fresh custody reattachment, and plan replacement reject any actor whose scheduler, complete attempt, and pure cleanup do not fit the guaranteed two-dimensional envelope.

## Reactive Delivery Evidence

The runtime binds `MaxObservationFanoutPagesPerBlock = 64` and `ObservationFanoutWeightLimit = 400,000,000,000 / 1,000,000`, and `WakeupWeightLimit = 400,000,000,000 / 1,000,000` for the overdue-wakeup worker. Production ProofSize admits five worst-case fanout units after base admission.

| Production topology after publication | Fanout service units | Blocks at five units |
| --- | ---: | ---: |
| One feed with 10,000 subscribers | 157 | 32 |
| One sparse subscriber at a historical high page id | 1 | 1 |
| Four dirty feeds with 10,000 subscribers each | 628 | 126 |
| One revision restart after quiescence | At most 314 | 63 |
| Persistently saturated queue | Unbounded | Unbounded |

A fanout service unit is one occupied-page attempt or one cursorless restart/cleanup transition; final-page completion may clear or restart within its page unit. The finite rows require stable active topology, no newer selected-feed revision, available configured RefTime/ProofSize budget, eventual queue capacity, and same-finalized-block runtime/code/metadata/constants matching generated client evidence. Mismatch withholds the rows while retaining factual chain topology. Estimates end at fanout completion, not queue admission, condition evaluation, or actor attempt.

Production fanout base is `31,565,000 / 1,543` (`6,565,000` benchmark RefTime plus one DbWeight read); the completing dense unit is `12,135,545,000 / 167,454` (`1,460,545,000` benchmark RefTime plus 139 reads and 72 writes). A saturated dense diagnostic is lower in RefTime and equal in ProofSize, so one conservative unit class remains sufficient.

The final corrective AAA production run used `frame-omni-bencher 0.22.0`, production Wasm, 50 steps, and 20 repeats. Existing-page enqueue is `59,925,000 / 6,474` with 7 reads and 5 writes; new-page enqueue is `60,483,000 / 8,890` with 8 reads and 5 writes. Preserve-page consume is `31,220,000 / 4,095` with 5 reads and 3 writes; delete-page consume is `32,895,000 / 4,077` with 5 reads and 5 writes.

Accepted wakeup values are `scheduler_wakeup_replace_exact = 64,814,000 / 6,664` with 6 reads and 7 writes; middle-page invalidation is `60,064,000 / 10,165` with 6 reads and 5 writes; exact cursor removal is `452,928,000 / 55,800` with 33 reads and 25 writes.

Tombstone drain is `7,057,781 + 2,132,168 × n` RefTime and `2,982 + 2,492 × n` ProofSize, with `5 + n` reads and 4 writes. Cycle orchestration is `44,699,000 / 9,667` with 3 reads and 2 writes; step orchestration is `44,555,323 + 215,321 × n` RefTime with 3 reads and 2 writes and covers the complete cycle execution over `n` inert steps. The scheduler base is `20,463,000 / 1,543` with 7 reads and 1 write. These values price measured bounded topology only; they imply no throughput promise.

## Generated Evidence and Artifacts

Current DEOS 0.7.11 candidate artifact identities:

| Artifact | SHA-256 |
| --- | --- |
| AAA runtime weights | `e46596b91c5d55112e6bb88360ac87078f6ae327680d48d972b54aa4a2b170cc` |
| DEOS Oracle runtime weights | `ffd422bd67a6b75c8bc4e76f7ace4aad5b40a352cf2b10a70547a241e261259e` |
| Compact compressed runtime Wasm | `71b5bf391632f599a059a0df3552fb308cdf729d93bd1f30c9cd9fc2608a0a7f` |
| SCALE-encoded V16 metadata | `bd38d1bd19b731ab9516faa374e315e53f006459b129a0fba1be8ccf56044d77` |
| AAA semantic manifest | `466a5b549b0f56a73dbffe2fcb76b02d040f9c623fa961da11f6b87aca066ba0` |
| AAA fee-envelope vectors | `6566d8d396ccbb00f074861c60fc90627fdb7f5550b64de7c3729cb07750eb32` |
| AAA ABI manifest | `eb5793f590c7ce10039856624d0f3fba49bf92f795dcce99a62e07dfcff37b7f` |

The metadata identity hashes the exact bytes at `web-client/.papi/metadata/deos.scale`. Fee-envelope `metadataSha256` and `weightSha256` equal the metadata and AAA-weight rows above. Observation evidence binds the same metadata SHA-256 and AAA-weight identity plus the rebuilt compact-Wasm runtime-code hash. No alternate compact-Wasm claim remains active.

`template/runtime/src/weights/pallet_aaa.rs` owns complete generated methods and storage annotations. Architecture records only load-bearing admission values and accepted identities; benchmark-host timing never becomes a chain-throughput claim.

## Control Plane and Read Surfaces

Canonical active projection joins `ActorIdentities`, `ActorHot`, and `ActorProgram` at one finalized block; funding details require the separately bounded `ActorFunding` value. Dormant identity, queue/wakeup membership, active-dirty topology, and bounded simulation results remain canonical-chain truth.

The read-only `AaaEligibilityApi::aaa_eligibility` projection reports current readiness (`ready`), the scheduler-owned phase explaining it, and `next_eligible_block` at one finalized block, reusing the exact scheduler arithmetic so the browser never reimplements cadence phase, cooldown, schedule window, retry backoff, breaker, or latch logic. It is canonical-chain truth at the queried block and never promises service, because queue position and available Weight decide actual admission.

The browser's authoring, artifact, matching-Wasm, simulation, observation, and governance-composition surfaces live under `web-client/src/lib/automation/` and `web-client/src/lib/observation/`. They bind metadata and runtime identity rather than recreating pallet semantics.

Unbounded history, archive search, forecasting records, governance preparation history, and longitudinal telemetry remain materialized-provider work under [`aaa-control-plane.contract.en.md`](./aaa-control-plane.contract.en.md) and [`read-model.contract.en.md`](./read-model.contract.en.md).

## Validation and Operations

Final reproducible validation used Rust/Cargo 1.96.1, Node 24.10.0, npm 11.7.0, and `frame-omni-bencher 0.22.0`. `scripts/aaa-release-gate.sh` passed its artifact-freshness, package, embedding, Clippy, and three of its five required scheduler stress profiles; the remaining `stress_10k_actors_queue_scheduler` and `checkpoint_a_s6_dense_10k_wakeups_converge_without_drops` profiles carry pre-existing 0.7.11 test-versus-scheduler-semantics mismatches (creation-time wakeup-first scheduling and a changed theoretical-max throughput assertion) that the weight-evidence pass reports separately. `npm run validate:all`, full package architecture, script-entrypoint, semantic-terminology, release self-test, Markdown, readability, backlog, Domain DAG, wiki, and completion audits passed against the artifact identities above.

Package tests own executable actor, scheduler, trigger, lifecycle, storage, and try-state behavior. Runtime tests own adapters, fees, ingress, genesis topology, Oracle/Router rollback, staking, XCM, generated-weight binding, block-budget partition, and full System/User composition.

The full `scripts/aaa-release-gate.sh` route covers package portability, external embedding, runtime integration, scheduler fairness, dense/sparse liveness, 10,000-actor queue stress, and the occupancy profile. The canonical script owns command syntax and profile selection.

`template/runtime/src/tests/fixtures/aaa-reactive-operations.v1.json` is the machine-readable reactive-operations corpus. Every scenario names initial state, ordered actions, checkpoints, terminal state, global invariants, production weight class, rollback boundary, runtime evidence identity, and an executable Rust test anchor. Seeded scenarios also name their seed.

`scripts/reactive-operations-corpus.sh` validates all or one family and emits selected seed/initial-state failure artifacts. `--execute` runs every selected Rust anchor in dev or release profile; those tests remain the behavioral evidence owner. The delivery-pressure family covers seeded revision races, newer-revision restart, queue-admission retry, three simultaneously dense feeds under one-unit round robin, cursor repair, dirty-age preservation/reset, and maximum subscriber density.

The runtime cross-pallet hook-rejection anchor fills AAA dirty capacity, attempts direct Oracle publication, and proves Oracle observation/revision, AAA feed/list state, and runtime events equal the captured pre-state. After capacity recovery, one producer retry commits Oracle revision `1` and AAA latest revision `1`; no replay state or Router publication path participates.

`AAA_OBSERVATION_PUBLISHER_INVENTORY` closes the reference-runtime publisher set to `DEOS Oracle::OnObservationChanged`. The Oracle hook reaches AAA through `ObservationChangeIngress`; no second runtime publisher owns revision progression. Generated observation evidence scans the complete runtime-config Rust tree, requires exactly one Oracle-owned typed ingress call, and rejects direct `AAA::note_observation_changed` bypasses.

Task rollback and lifecycle rollback remain distinct corpus boundaries. A DEX adapter that fails after input transfer restores actor/pool task writes while `ContinueNextStep` permits the following transfer and cycle summary to commit. Corrupt dirty-list linkage makes deactivation fail closed and restores actor, subscription, dirty-feed, list, and event pre-state; explicit linkage repair permits a fresh deactivation attempt to finish cleanup.

Operational-recovery fixtures bind exact runtime version identity, breaker deferral/recovery, Continuation wakeup, and bounded repair. Version drift yields `EvidenceMismatch` without changing factual reactive state. Breaker activation produces no partial cycle/close evidence; recovery admits the deferred close. A suspended retry wakes at cooldown without another signal, while one-actor permissionless repair remains available under the breaker and performs no hidden task work.

Full-corpus validation requires scenario coverage for revision linkage, dirty-feed uniqueness, subscriber-page reachability, queue/wakeup exclusivity, atomic pre-state restoration, and bounded weight ownership. Quick AAA acceptance validates this contract; full acceptance executes all 20 anchored Rust tests in the selected Cargo profile.

Operational observation uses canonical queue head/tail, exact queue occupancy, active limit, wakeup cursor/buckets, actor-local queue and wakeup pointers, starvation detection/recovery events, and sweep events. Weight and scan deferral remains silent and state-preserving. These surfaces expose pressure without promising an exact future execution block.

### Capacity Economics

The reference runtime binds `MaxActiveActors = MaxActorIdentities = MaxQueueLength = 10,000`, `MaxOwnerSlots = 255`, `MaxSweepBatch = 5`, and `AaaCreationFee = EXISTENTIAL_DEPOSIT = 0.001` fee-native units. Filling the identity cap with User actors therefore requires at least 10 fee-native units in nonrefundable creation fees and at least 40 distinct owner accounts. Active User liveness may additionally require balances above the protected minimum, but that balance remains actor custody and is not an anti-spam fee.

The same 10,000 ceiling bounds simultaneous active actors and physical scheduler occupancy. One maximum permissionless sweep call examines five explicit identities, so 2,000 full batches cover the complete identity cap. A hypothetical one-batch-per-block sequence spans 12,000 seconds, or 3 hours 20 minutes, at the six-second target; this is a latency illustration, not guaranteed throughput, because dispatch Weight, competing block demand, eligibility, and submitted transaction count control realized progress.

The handwritten AAA production core (`adapters.rs`, `contract.rs`, `execution.rs`, `lib.rs`, `reactions.rs`, `scheduler.rs`, `subscriptions.rs`, and `types.rs`) measures 13,440 lines against the 13,438-line accepted pre-cooling baseline: a net increase of two lines. Benchmarks, generated weights, tests, fixtures, and client artifacts are excluded from that source metric and require separate evidence identities.

Cooling metrics after the cache-revalidation rebenchmark pass: compact compressed production Wasm is 1,404,835 bytes (down from 1,405,148 before the pass, -313 bytes) with SHA-256 `71b5bf391632f599a059a0df3552fb308cdf729d93bd1f30c9cd9fc2608a0a7f`; compact Wasm is 8,621,381 bytes (down from 8,621,416) and the uncompressed Wasm is 9,135,448 bytes (down from 9,135,455). Generated storage dimensions moved from the provisional derivation to measured values: `scheduler_on_idle_base` reads 7 and writes 1 with 1,543 ProofSize (was 6 reads, 1 write, 1,543 ProofSize); `cache_revalidation_units` reads `2 + 3n` and writes `1 + n` with `3,591 + 8,677 × n` ProofSize (was `8 + 8n` reads/writes with `1,543 + 9,667 × n` estimated ProofSize). The measured revalidation unit is `20,303,784 + 20,776,516 × n` RefTime, replacing the derived `19,556,000 + 121,875,000 × n` provisional model; one maximum unit fits `ActorServiceReserve` after the maximum fixed workers.

The guaranteed saturated service model assumes only the one maximum actor attempt-or-cleanup admitted by `ActorServiceReserve`, not the configured count ceiling. With 10,000 eligible FIFO actors ahead of no bypass and one maximum attempt per block, a tail actor reaches an attempt within 10,000 conforming six-second blocks: 60,000 seconds, or 16 hours 40 minutes. One full FIFO traversal has the same conservative bound. Lighter measured plans may admit more work, but `MaxExecutionsPerBlock = 1,000` is only a count ceiling and creates no throughput promise. Cadence denotes temporal eligibility; queue position and available Weight determine actual service.

Governance can lower the active-admission limit, operate the breaker, and repair bounded scheduler state, but it cannot confiscate or forcibly close a healthy owner-controlled User actor merely to recover capacity. Permissionless sweep closes only actors meeting the specified liveness/expiry conditions. Consequently, a fully funded live identity-cap fill has no bounded governance-only eviction latency in the current contract. DEOS provides explicit finite cost and bounded repair, not adaptive creation pricing or a guarantee against economically funded saturation.

The zombie-spam regression compares the complete actor-cap creation-cost floor with bounded permissionless sweep fees. Governance changes to creation fee, fee conversion, or sweep bounds must preserve the measured dominance relation rather than relying on a copied ratio.

Package scheduler, trigger, lifecycle, storage, and extrinsic internals remain authoritative in the package architecture. This integration map owns only the concrete bindings and policies layered over those mechanisms.
