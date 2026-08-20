# DEOS Actors Integration

## Purpose and Ownership

This document maps how the DEOS reference runtime composes reusable `pallet-deos-actors` with deterministic System identities, TMCTOL Actor Contract families, DEOS Router, Oracle, assets, staking, fee collection, XCM, governance, generated weights, and browser/control-plane surfaces.

The portable actor contract and crate implementation remain in [`template/pallets/actors/docs/specification.en.md`](../template/pallets/actors/docs/specification.en.md), [`template/pallets/actors/docs/architecture.en.md`](../template/pallets/actors/docs/architecture.en.md), and [`template/pallets/actors/docs/embedding.md`](../template/pallets/actors/docs/embedding.md). This document owns only concrete DEOS composition.

## Integration Code Map

| Surface | Anchor |
| --- | --- |
| Runtime adapters, actor builders, bounds, and origins | `template/runtime/src/configs/actor_config.rs` |
| Runtime-generated Actors weights | `template/runtime/src/weights/pallet_deos_actors.rs` |
| DEOS Oracle publication hook | `template/runtime/src/configs/oracle_config.rs` |
| Router fee, quote, execution, and observation composition | `template/runtime/src/configs/deos_router_config.rs` |
| Asset and transaction-extension ingress | `template/runtime/src/configs/assets_config.rs`, `template/runtime/src/lib.rs` |
| Genesis System identities and ED anchors | `template/runtime/src/genesis_config_presets.rs` |
| Runtime integration and load evidence | `template/runtime/src/tests/actor_integration_tests.rs`, `template/runtime/src/tests/load_testing.rs` |
| Off-chain artifacts and simulation | `docs/actors-control-plane.contract.en.md`, `web-client/src/lib/automation/` |

## Temporal Binding

The six-second DEOS slot binds block cooldown, retry, and window horizons through `ActorMaxExecutionDelayBlocks = 52_596_000`, exactly `ceil(10 × 365.25 days / 6 seconds)`. Cadence instead uses consensus timestamp through `ActorCadenceTickMillis = 500`; the same numeric admission cap bounds `every_ticks` without converting ticks into blocks.

## Namespace and Sovereign Accounts

The runtime binds package `pallet-deos-actors`, Rust crate `pallet_deos_actors`, and `ActorsPalletId = *b"actors00"`. The pallet account is `PalletId(*b"actors00").into_account_truncating()` under `AccountId32` and SS58 prefix `42`.

User actors derive sovereign accounts from `(PalletId, owner, owner_slot)`. System actors derive them from `(PalletId, "system", sovereign_id)`; `ActorClass::System { sovereign_id }` carries that custody locator independently from the actor-id key. Fresh creation assigns the new actor id as a new locator. `SystemSovereigns` retains every allocated locator as `Vacant | Occupied(actor_id)`; governance may attach a fresh identity to a vacant locator without changing its account or residual balances. `ActorCreated.actor_class` carries `ActorClass::User { owner_slot }` or `ActorClass::System { sovereign_id }`; no separate event field duplicates either value.

The complete DEOS deterministic System account map follows.

| actor_id | Role or account | Hex | SS58 |
| ---: | --- | --- | --- |
| — | Actors pallet account | `0x6d6f646c6163746f727330300000000000000000000000000000000000000000` | `5EYCAe5fiQWMqjyVakD96Nwxv8toW2XYiWaTHmnmop8X9u5J` |
| 0 | Burn Actor | `0xe5d2c431c880d0bfbad3663b09164d86a76696dc2f137eeb502359fd28363f42` | `5HG3S6PLHrykv65Vw8j19zRaEx2Bmb37iywfo2qK3cHosGKX` |
| 1 | Fee Sink | `0x7576c68c853f9f0427ae0c26043cd168ca5672bcdb221d9c0ad4ae7234d17e43` | `5Eiik51gjANLwbjZUXnVJv8pPpoTTVVic2x5sNwy8NaoVaJ9` |
| 2 | Liquidity Actor | `0x643d7f4212a9f0ad63071393bc9accbcc2eabb4d32e30ebbf546bb8c3f852b70` | `5EL8uyEoZA3JQkhCC3ackopXhdujtKjHHRYVSM1BVrf5x6LW` |
| 3 | TOL Bucket A | `0x35c4420572bfee8130a3ad5072f26d9b9ce0cf349bdb6fe1fb2c5b8fa99d4186` | `5DHChJzyAY9pz54d6PXLmScG5vhdiarfNY2VjhkP4pG8vqSs` |
| 4 | TOL Bucket B | `0x8667dc4e696df85145ff65005d50f842d4aa196b2b0481681d6086d38a98c263` | `5F6w8Jd8mHTPphhHgBdUJdkTaT2hQ8mKYojDhzCre5TJqGPg` |
| 5 | TOL Bucket C | `0x0c90365514a0e365f883e8f4a14f18b2090e77d952d3be055847a10ef7fc8b0e` | `5CMBGiT8bLjfecCBLf7jSeWXoHKwEXtF7epoFHaLSTmxPhyp` |
| 6 | TOL Bucket D | `0x7a2cdcdf546f84c94b2de0d2db31906a3872ece0f1604816a6ff16b2f292d459` | `5Epu2U8sJbpBH1AQhc2KW6yuPA62Hst9r3zSdEHx4vS386JW` |
| 7 | Treasury B | `0x25cca60a36d1458c32e01b8d6d70aa836a98d53e13c5c51b1f8566633677d72d` | `5CvGRScqAYFFZRymun1fNJogwgUZCigd2ncmxCGvpquWy4nM` |
| 8 | Treasury C | `0x9ab9d1e2aa163c1e0df8910b3f840824bde1c3be288be2d2c4a75910b68362fd` | `5FZaRybmQEh2eHXM95zB2tyty3vxBZPyrCYTekHu5YxuCKj8` |
| 9 | Treasury D | `0x1a01084c8c17375cf01299a8f492de6023bc29b78e56024510630be56b5c38f3` | `5CeoQfeA6zkG7yToYZm3L8g5gjR5aMikm4b1gVLK69CgYzsC` |
| 10 | BLDR Splitter | `0xdc201c83f1db632704da438c2fe7e6212c4a25921c48cd9294f6dde633ef1d85` | `5H3KvwhcEmU5QZNcXWjwwmtduXdrKTrR5WYZqjrJm23KK14u` |
| 11 | BLDR Liquidity Actor | `0x2e699b4acc26bcf078237dc13eda2470505c8bd99450269eeb7eb4c5f5472968` | `5D7ZRz4hMphgVdq9UYBA9Gtk1q2cBjKTgoDCqpBETQi6Ziq4` |
| 12 | BLDR Bucket A | `0x791ec3fe30f34d005232cdf3bb5abdc0ae14e51fe3caeb62914d35f7c81ae544` | `5EoWnoVuB925BHs9UwHUfLkcm5rSbmqzrHgFZRzY5nA4M5B6` |
| 13 | BLDR Treasury | `0x07297bfba697b7593a93b6bc2c52f7dc4452d968c1e2c3badb09f2fafb8d1709` | `5CE6WsJ12vyyjAPMuvaqf2cdSQMVzAAxVjZDvXZK99VswFGe` |
| 14 | Native staking LP provisioning actor | `0x14292af3e9e70acb4c39cfe83317039c1f2111b475b99e660d87b16948edc339` | `5CX93X5agA9cbvbv4JKpXmR8RF9ywdLbyg6WR9qY15evri5L` |

## Genesis Topology

| Lane | Role | actor_id | Genesis lifecycle |
| --- | --- | ---: | --- |
| Core | Burn Actor | 0 | Active burn plan |
| Core | Fee Sink | 1 | Active 120-tick/60-second 10% buffer allocation |
| Core | Liquidity Actor | 2 | Dormant |
| TOL | Bucket A | 3 | Custody-only |
| TOL | Buckets B/C/D | 4–6 | Dormant |
| Treasury | Treasuries B/C/D | 7–9 | Dormant |
| BLDR | BLDR Splitter | 10 | Active 50/50 split |
| BLDR | BLDR Liquidity Actor | 11 | Dormant |
| BLDR | BLDR Bucket A | 12 | Custody-only |
| BLDR | BLDR Treasury | 13 | Dormant |
| Staking | Native staking LP provisioning actor | 14 | Dormant |

Active genesis actors use the runtime System cooldown, `ActorType::System`, `Mutability::Mutable`, and no schedule window. Fee Sink's tick-zero bootstrap wakeup anchors its 120-tick period from the first consensus timestamp without executing allocation. Dormant entries occupy `ActorIdentities` and `SovereignIndex` without hot program, queue, wakeup, funding, fee, or Active-epoch state. Custody-only accounts occupy no actor identity.

The reference runtime configures no System Immutable actor, so it has no actor-specific emergency migration or custody disposition to execute. A downstream runtime that admits an indefinite System Immutable actor must ship its migration-specific source/target actor set, bounded Close or Deactivate disposition, custody handling, terminal invariant, and Continuation policy with the same upgrade. The ordinary DEOS Governance path exposes 3-day lead-in, 7-day vote, 7-day protection, and 3-day enactment delay—20 days before bounded maturity/operational delay. Protocol `L1RootAction` can use the separately governed 24-hour urgent path only with unanimous raw protection-track `Pass`; Actors promises neither path completes within a finite time.

`ActorIdentityCount` covers thirteen active plus dormant identities. `NextActorId = 15` preserves the reserved address range. Every expected small-native-flow System or custody account receives one persistent free-balance ED anchor because a provider or reserved balance alone does not make a zero-free account eligible for sub-ED native ingress under `pallet-balances` v50.

## Actor Contract Families

The runtime keeps TMCTOL policy declarative through builders in `actor_config.rs`.

| Builder | Actor family | Composition |
| --- | --- | --- |
| `build_burn_contract_steps` | Burn Actor | Foreign balances → Native swap → burn |
| `build_fee_sink_contract_steps` | Fee Sink | Above the per-leg ED threshold, process 10% of spendable Native → phase-aware allocation |
| `build_zap_contract_steps` | Liquidity Actor | Add LP → surplus swap → split LP to buckets |
| `build_bucket_lp_transfer_contract_steps` | Buckets B/C/D | Transfer bounded LP fraction to paired Treasury |
| `build_treasury_lp_unwind_contract_steps` | Treasuries B/C/D | Remove configured LP into Treasury custody |
| `build_bldr_splitter_contract_steps` | BLDR Splitter | Split minted BLDR share between liquidity and treasury lanes |
| `build_bldr_liquidity_contract_steps` | BLDR Liquidity Actor | Add NTVE/BLDR liquidity → transfer LP to BLDR Bucket A |
| `build_treasury_b_buyback_contract_steps` | Treasury B | Optional NTVE buyback → burn acquired target |
| `build_native_staking_liquidity_contract_steps` | Native Staking Liquidity Actor | Donate balanced `NTVE/stNTVE` without minting LP |

These builders configure the reusable task language; they do not create pallet-level roles or Actors-id policy branches.

The package architecture owns the exhaustive public reachability matrix. DEOS production builders currently instantiate the reference topology subset, while typed creation/update calls and the independent embedding runtime keep the remaining portable variants executable. Constructor-free runtime-upgrade cancellation and context-free amount dependency placeholders are absent; adding any public variant requires its constructor, evaluator/adapter branch, and executable evidence in the same change.

## Governance Activation Flows

`Foreign asset + TOL lane`: register the foreign asset, create the Native/foreign pool, extend the Burn Actor, activate the Liquidity Actor, then optionally activate paired Bucket transfer and Treasury unwind plans.

`BLDR lane`: retain the BLDR Splitter at genesis, create the NTVE/BLDR pool, activate the BLDR Liquidity Actor, then optionally activate Treasury buyback/burn policy.

`Native staking LP lane`: register native staking, initialize `stNTVE`, create and seed the AMM, then call `activate_native_staking_liquidity_actor`. Activation fails until receipt asset, staking pool, actor, and nonempty AMM all exist.

Emergency policy pauses one actor through `pause_actor` or stops cycle execution globally through the circuit breaker while bounded bookkeeping remains active.

## Market Adapter Composition

`TmctolDexOps` routes exact-input and exact-output swaps through DEOS Router with `ExecutionContext { actor, actor_type }` and returns actual `DexSwapOutcome { total_amount_in, recipient_amount_out }` facts to Actors. The accepted full production generation measures the Native-anchored maximum at `561,393,000 / 19,253` for exact-input and `563,139,000 / 19,253` for exact-output. Actors supplies immutable actor authority; the adapter uses it only for typed market protection and never infers System status from the sovereign catalog.

Exact input derives `min_out` from the caller-aware quote and binds zero tolerance to that quote. Exact output obtains one reverse quote, adds authored tolerance with ceiling arithmetic, intersects it with live preservable input capacity, and executes under the explicit total-input cap.

DEOS Router evaluates the direct XYK candidate and at most one reverse-quoted Native-anchored path, selecting minimum required input. TMC remains exact-input only because it exposes no exact-recipient-output execution contract.

System swaps read the exact directional Oracle feed with `MAX_SYSTEM_REFERENCE_AGE_BLOCKS = 100` and enforce `ActorMaxSystemPriceDeviation = 5%`. Fresh nonzero truth at the exact age boundary remains eligible. Unavailable, Uninitialized, Stale, or invalid truth falls back to direct reserves; unavailable fallback or excessive deviation fails Temporary before mutation.

User swaps retain Router's ordinary direct-pair guard and do not fail solely because the standalone Oracle feed is absent or uninitialized. Native-anchored System routes without a pair reference fail closed.

Every reference-runtime System swap is Native-anchored: Burn and Liquidity Actors convert foreign assets to Native, and Treasury buyback converts Native to a target asset. A direct pool therefore always supplies the reserve fallback and the guard never runs dry. Configuring a System actor on a pair holding neither a direct pool nor a published feed leaves it retrying `SystemReferencePriceUnavailable` indefinitely, because Temporary failure alone never terminates; such a pair needs an Oracle feed before activation.

The guard bounds authored execution loss; it does not prove external fair price, ordering safety, manipulation resistance, or MEV immunity.

## Integration Boundary

Actors invokes assets, swaps, liquidity, staking, fee collection, and direct ingress only through runtime adapters. Concrete ledger semantics, Router route selection, pool mechanics, staking representation, and fee destinations remain outside the pallet package.

Task-scoped storage transactions preserve committed earlier steps while rolling back a failing task's local effects. Runtime adapters classify only explicit Temporary market or infrastructure failures as retryable; unknown downstream errors remain Permanent.

## Runtime Adapter Bindings

`DeosFundingAuthority` receives only `RuntimePolicy` decisions after pallet-owned source-policy evaluation and defaults deny because the launch matrix authorizes no actor/source pair.

`TmctolAssetOps` maps Native to `pallet-balances` and Local/Foreign to `pallet-assets`. Its transfer preflight covers source withdrawal and recipient deposit consequences. Ordered `SplitTransfer` legs all preflight before mutation; task rollback forbids partial fan-out.

`pallet-balances` v50 rejects a new zero-free account below ED even when FRAME already holds a provider. DEOS therefore endows expected small-flow System, custody, and staking-ingress accounts with one persistent free ED anchor and preserves it through amount resolution.

`TmctolLiquidityOps` delegates add/remove/donation to Asset Conversion while retaining ratio, LP receipt, and native-special-case policy in the adapter. `TmctolStakingOps` maps every Actor staking asset to the generic `stake(asset_id, amount)` call and resolves stable share assets through the staking receipt index.

Runtime adapters use typed failure classification. Explicit route, liquidity, slippage, oracle, and temporary-capacity failures may retry; malformed, forbidden, funding, fee, and unknown downstream failures remain Permanent.

## Address and Funding Ingress

All successful supported producers call the fallible direct ingress boundary in their originating transaction. They preflight before value movement and propagate notification failure; no event scan, compatibility ring, or deferred correctness layer exists.

| Producer family | DEOS integration |
| --- | --- |
| Signed Balances/Assets transfer | Transaction extension carries one bounded direct candidate and verified signer provenance |
| `transfer_all` | Candidate resolves actual movement from recipient balance delta |
| Actors Transfer/Mint | `TmctolAssetOps` submits sender or source-less typed ingress inside task execution |
| TMC distribution | Mint-output adapter submits once and preserves available source provenance |
| Router fee routing | Fee adapter submits once with fee-payer provenance |
| XCM asset deposit | `ActorAwareAssetTransactor` submits one converted or source-less candidate |
| Privileged/delegated producers | Direct source-less candidate preserves signal delivery but remains balance-only |

XCM binds generated one-asset deposit weight and `MaxAssetsIntoHolding = 1`, preventing one instruction from multiplying synchronous Actors ingress work without a corresponding instruction-specific weigher.

User actors default to `OwnerOnly`; accepted verified owner or allowlist transfers may add to bounded funding accumulators. System actors default to denied `RuntimePolicy`. Source-less or rejected provenance still creates spendable ledger balance but does not gain tracked funding authority.

### Crediting-Producer Inventory

Every producer path that can credit an Actors sovereign account uses one paired preflight/notify transaction through `RuntimeAddressEventIngress`. The inventory below names each path, its credited surface, source/provenance semantics, preflight owner, notification owner, rollback witness, and Weight owner; all movement is reverted on preflight or notification failure in the same transaction.

`SourceFilter::Any` accepts every certified source that passes the authored asset filter. Any such source may set the actor's pending latch, and a resulting User attempt spends that actor's Weight-derived fee budget even when the sender acts only to force evaluation. DEOS adds no hidden sender trust list, reimbursement, or anti-grief pricing policy; authors who cannot accept that exposure use `OwnerOnly` or an explicit bounded whitelist.

**Ingress producers (credit another actor's sovereign):**

| Producer path | Credited surface | Source / provenance | Preflight owner | Notify owner | Rollback witness | Weight owner |
| --- | --- | --- | --- | --- | --- | --- |
| Signed Balances/Assets transfer | Recipient sovereign | Signer / `Signed` | `TransactionExtension` candidate preflight | `post_dispatch_details` notify | Balances/Assets ledger revert | `transaction_extension_ingress_base` + `_notify` |
| `transfer_all` | Recipient sovereign | Signer / `Signed`, actual recipient delta | Same candidate preflight | Same notify | Balances ledger revert | Same extension weights |
| Actors Transfer/Mint task | Task `to` sovereign | Sender or source-less / typed | `TmctolAssetOps::transfer` preflight | Same adapter `on_internal_inbound` | Asset ops transaction | `task_dex_*`/`task_transfer` generated weights |
| TMC collateral distribution | Collateral/minted recipients | Mint source / `InternalProtocol` | `before_collateral_transfer`/`before_sink_mint` | `after_distribution` | TMC distribution transaction | TMC distribution generated weights |
| Router fee routing | Burn Actor sovereign | Fee payer / `InternalProtocol` | `route_fee` preflight | Same `on_internal_inbound` | Router fee transaction | Router fee routing weights |
| XCM asset deposit | Recipient sovereign | XCM origin / `Xcm` | `preflight_xcm_inbound` | `on_xcm_inbound` | `ActorAwareAssetTransactor` deposit revert | One-asset deposit generated weight |
| XCM without origin | Recipient sovereign | Source-less / none | Implicit source-less preflight | `on_inbound_without_source` | Same deposit revert | Same one-asset weight |
| Privileged/delegated producers | Recipient sovereign | Source-less candidate / none | Direct source-less preflight | Same source-less notify | Producer transaction revert | Producer-supplied weights |

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

Movement to a non-Actors recipient and a task transfer to the actor's own sovereign remain explicit exclusions: `resolve_actor` returns `None` for non-sovereign recipients and the pallet's recipient validation rejects self-transfers with `SelfTransferNotAllowed`.

Fee collection is intentionally excluded from certified AddressEvent ingress. Actor, transaction-payment, governance-opening, and XCM fee collectors credit the Fee Sink ledger without creating trigger or funding state; its single cadence owns allocation. `ACTORS_ADDRESS_EVENT_PRODUCER_INVENTORY`, generated ingress evidence, paired-executive tests, and the embedding fixture continue to cover paths that actually signal an actor.

## Fee Composition

The package-owned `attempt_fee_envelope` derives each task and complete-plan reserve from runtime-generated task weights, `WeightToFee`, and configured step fees. `settle_attempt_fee_step` releases the selected reservation before charging evaluation-only or attempted-step fees. DEOS runtime tests consume that exact envelope rather than reconstructing fee arithmetic.

Package-generated `actors-fee-envelope-vectors.json` constrains browser forecast fee policy across User/System suffixes, reservation release, rollback pricing, and the direct User fee-native floor. One selected fee charges per executed or failed task attempt; skipped steps charge evaluation only.

`TmctolFeeCollector` transfers the complete charge into Fee Sink System Actor `1` through a ledger-only movement, matching transaction-payment and XCM-trader collection without fabricating AddressEvent readiness. One 120-tick cadence under the 500 ms consensus clock owns a stable 60-second allocation period. Its plan processes 10% of the current spendable Native buffer only when each configured split leg receives at least one ED, allocates the processed amount under the current phase policy, and retains the unprocessed buffer plus indivisible remainder. Runtime regressions prove collection-only custody, threshold skips, timestamp readiness, and no early execution.

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
| `MaxContractSteps` | Configurable shared `1..=255` bound; DEOS baseline is 8 for both classes |
| `MaxRetryAttempts` | 10 cursor-local unsuccessful attempts |
| `MaxConsecutiveFailures` | 10 |
| `MaxAutoCloseNonceHorizon` | 10,000 |

Runtime block policy assigns 50% to dispatch and 50% guaranteed `on_idle` headroom. No dedicated Operational reserve exists while no concrete critical Operational call consumes one. `ActorOnIdleReserve` binds directly to `1,000,000,000,000 / 2,500,000`.

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

A fanout service unit is one occupied-page attempt or one cursorless restart/cleanup transition; final-page completion may clear or restart within its page unit. The finite rows require stable active topology, no newer selected-feed revision, available configured RefTime/ProofSize budget, eventual queue capacity, and same-finalized-block runtime/code/metadata/constants matching generated client evidence. Mismatch withholds the rows while retaining factual chain topology. Estimates end at fanout completion, not queue admission, Predicate evaluation, or actor attempt.

Production fanout base is `31,565,000 / 1,543` (`6,565,000` benchmark RefTime plus one DbWeight read); the completing dense unit is `12,136,381,000 / 166,430` (`1,461,381,000` benchmark RefTime plus 139 reads and 72 writes). ProofSize therefore remains the active five-unit limit under the configured budget.

The accepted Actors production run used `frame-omni-bencher 0.22.0`, production Wasm, 50 steps, and 20 repeats. `template/runtime/src/weights/pallet_deos_actors.rs` owns the complete current scheduler, wakeup, ingress, task, predicate, and orchestration coefficients. Those values price measured bounded topology only; they imply no throughput promise.

## Generated Evidence and Artifacts

`scripts/actors-assurance.sh` owns freshness checks for Actors semantic, fee-envelope, ABI, observation, ingress, weight, and metadata evidence. Production Wasm, metadata, descriptors, and generated client evidence remain owned by their build/export commands and checked directly by the `full` validation profile.

`template/runtime/src/weights/pallet_deos_actors.rs` owns complete generated methods and storage annotations. Architecture records only load-bearing admission values; benchmark-host timing never becomes a chain-throughput claim.

### Generated Event Trace Corpus

These non-normative traces project the package event-order fixtures; Section 8 of the Actors specification and runtime metadata remain authoritative for semantics and fields.

| Scenario | Ordered event trace | Falsification anchor |
| --- | --- | --- |
| Fresh transfer cycle | `CycleStarted -> TransferExecuted -> CycleSummary(Completed)` | Fresh simulation and package cycle-order tests |
| Temporary retry attempt | `CycleContinued -> StepFailed(Temporary) -> CycleSuspended(Temporary)` | `continuation_*` and retry-bound package tests |
| Cancel on semantic update | `CycleCancelled -> CycleSummary(Cancelled) -> ContractUpdated` | Semantic replacement cancellation tests |
| Close with Continuation | `CycleCancelled(Closing) -> CycleSummary(Cancelled) -> ActorClosed` | Continuation close-order tests |
| Expiry during suspension | `CycleCancelled(Closing(WindowExpired)) -> CycleSummary(Cancelled) -> ActorClosed(WindowExpired)` | Window-expiry Continuation tests |

The corpus intentionally omits block numbers, balances, and exhaustive step fields. It illustrates ordering only and creates no history or indexer promise.

## Control Plane and Read Surfaces

Canonical active projection joins `ActorIdentities`, `ActorHot`, and `ActorContract` at one finalized block; funding details require the separately bounded `ActorFunding` value. Dormant identity, queue/wakeup membership, active-dirty topology, and bounded simulation results remain canonical-chain truth.

The runtime simulation API executes the same package evaluator and finalizer used by scheduler service. Its bounded records carry canonical `StepOutcome` values, including concrete failure cause plus retry disposition, and its status is the shared `AttemptDisposition`; DEOS adds no adapter-side simulation model.

The read-only `ActorEligibilityApi::actor_eligibility` projection reports `NotRegistered`, `Dormant`, or the canonical Active classification at one finalized block, preserving terminal reason plus exact retry/block/timestamp-tick payloads so the browser never reimplements cadence, cooldown, schedule window, retry backoff, breaker, or latch logic. It is canonical-chain truth at the queried block and never promises service, because queue position and available Weight decide actual admission.

The browser's authoring, artifact, matching-Wasm, simulation, observation, and governance-composition surfaces live under `web-client/src/lib/automation/` and `web-client/src/lib/observation/`. They bind metadata and runtime identity rather than recreating pallet semantics.

Unbounded history, archive search, forecasting records, governance preparation history, and longitudinal telemetry remain materialized-provider work under [`actors-control-plane.contract.en.md`](./actors-control-plane.contract.en.md) and [`read-model.contract.en.md`](./read-model.contract.en.md).

## Validation and Operations

Package tests own executable actor, scheduler, trigger, lifecycle, storage, and try-state behavior. Runtime tests own adapters, fees, ingress, genesis topology, Oracle/Router rollback, staking, XCM, generated-weight binding, block-budget partition, and full System/User composition.

`scripts/actors-assurance.sh` owns package portability, external embedding, runtime integration, scheduler fairness, dense/sparse liveness, 10,000-actor queue stress, and occupancy proof commands. Repository toolchain authorities and canonical release profiles own environment and profile selection; this integration document does not duplicate release results or version pins.

The runtime cross-pallet hook-rejection anchor fills Actors dirty capacity, attempts direct Oracle publication, and proves Oracle observation/revision, Actors feed/list state, and runtime events equal the captured pre-state. After capacity recovery, one producer retry commits Oracle revision `1` and Actors latest revision `1`; no replay state or Router publication path participates.

`ACTORS_OBSERVATION_PUBLISHER_INVENTORY` closes the reference-runtime publisher set to `DEOS Oracle::OnObservationChanged`. The Oracle hook reaches Actors through `ObservationChangeIngress`; no second runtime publisher owns revision progression. Generated observation evidence scans the complete runtime-config Rust tree, requires exactly one Oracle-owned typed ingress call, and rejects direct `Actors::note_observation_changed` bypasses.

Task rollback and lifecycle rollback remain distinct corpus boundaries. A DEX adapter that fails after input transfer restores actor/pool task writes while `ContinueNextStep` permits the following transfer and cycle summary to commit. Corrupt dirty-list linkage makes deactivation fail closed and restores actor, subscription, dirty-feed, list, and event pre-state; explicit linkage repair permits a fresh deactivation attempt to finish cleanup.

Operational-recovery fixtures bind exact runtime version identity, breaker deferral/recovery, Continuation wakeup, and bounded repair. Version drift yields `EvidenceMismatch` without changing factual reactive state. Breaker activation produces no partial cycle/close evidence; recovery admits the deferred close. A suspended retry wakes at cooldown without another signal, while one-actor permissionless repair remains available under the breaker and performs no hidden task work.

Full-corpus validation requires scenario coverage for revision linkage, dirty-feed uniqueness, subscriber-page reachability, queue/wakeup exclusivity, atomic pre-state restoration, and bounded weight ownership. Quick Actors acceptance validates this contract; full acceptance executes all 20 anchored Rust tests in the selected Cargo profile.

Operational observation uses canonical queue head/tail, exact queue occupancy, active limit, wakeup cursor/buckets, actor-local queue and wakeup pointers, starvation detection/recovery events, and sweep events. Weight and scan deferral remains silent and state-preserving. These surfaces expose pressure without promising an exact future execution block.

### Capacity Economics

The reference runtime binds `MaxActiveActors = MaxActorIdentities = MaxQueueLength = 10,000`, `MaxOwnerSlots = 255`, `MaxSweepBatch = 5`, and `ActorCreationFee = EXISTENTIAL_DEPOSIT = 0.001` fee-native units. Filling the identity cap with User actors therefore requires at least 10 fee-native units in nonrefundable creation fees and at least 40 distinct owner accounts. Active User liveness may additionally require balances above the protected minimum, but that balance remains actor custody and is not an anti-spam fee.

The same 10,000 ceiling bounds simultaneous active actors and physical scheduler occupancy. One maximum permissionless sweep call examines five explicit identities, so 2,000 full batches cover the complete identity cap. A hypothetical one-batch-per-block sequence spans 12,000 seconds, or 3 hours 20 minutes, at the six-second target; this is a latency illustration, not guaranteed throughput, because dispatch Weight, competing block demand, eligibility, and submitted transaction count control realized progress.

The guaranteed saturated service model assumes only the one maximum actor attempt-or-cleanup admitted by `ActorServiceReserve`, not the configured count ceiling. With 10,000 eligible FIFO actors ahead of no bypass and one maximum attempt per block, a tail actor reaches an attempt within 10,000 conforming six-second blocks: 60,000 seconds, or 16 hours 40 minutes. One full FIFO traversal has the same conservative bound. Lighter measured plans may admit more work, but `MaxExecutionsPerBlock = 1,000` is only a count ceiling and creates no throughput promise. Cadence denotes temporal eligibility; queue position and available Weight determine actual service.

Governance can lower the active-admission limit, operate the breaker, and repair bounded scheduler state, but it cannot confiscate or forcibly close a healthy owner-controlled User actor merely to recover capacity. Permissionless sweep closes only actors meeting the specified liveness/expiry conditions. Consequently, a fully funded live identity-cap fill has no bounded governance-only eviction latency in the current contract. DEOS provides explicit finite cost and bounded repair, not adaptive creation pricing or a guarantee against economically funded saturation.

The zombie-spam regression compares the complete actor-cap creation-cost floor with bounded permissionless sweep fees. Governance changes to creation fee, fee conversion, or sweep bounds must preserve the measured dominance relation rather than relying on a copied ratio.

Package scheduler, trigger, lifecycle, storage, and extrinsic internals remain authoritative in the package architecture. This integration map owns only the concrete bindings and policies layered over those mechanisms.
