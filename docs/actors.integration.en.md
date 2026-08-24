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

The six-second DEOS slot binds block cooldown and window horizons through `ActorMaxExecutionDelayBlocks = 52_596_000`, exactly `ceil(10 × 365.25 days / 6 seconds)`. AtTime and Cadenced use consensus timestamp through `ActorCadenceTickMillis = 500` and independent `ActorMaxTemporalDelayTicks = 631_152_000`, exactly `ceil(10 × 365.25 days / 500 milliseconds)`. These typed horizons are never converted or reused across clocks; retry backoff remains separately protocol-capped.

## Actor-State Holds and Capacity

The DEOS runtime maintains no Trigger-family bond, rent, or fee reserve. It binds `pallet-balances` as `StateHoldCurrency`, `RuntimeHoldReason::Actors(ActorState)` as the dedicated reason, one ED as each present component's fixed base, and one `MICRO_UNIT` per retained SCALE byte. `ActorStateHolds` separates identity, C6 header, actual cold chunks, detector topology, funding, and run capacity per User Actor; the run component reserves the type-derived maximum for the Active installed lifetime, costs `0.001805` native units per Actor under current DEOS pricing, and prevents owner-balance dependency at autonomous Opening. Dormant Actors release Active components, and System Actors are hold-exempt host capacity. Lifecycle, ingress, execution, cancellation, deactivation, and close reconcile owner hold deltas transactionally, and close never touches sovereign custody.

## Namespace and Sovereign Accounts

The runtime binds package `pallet-deos-actors`, Rust crate `pallet_deos_actors`, and `ActorsPalletId = *b"actors00"`. The pallet account is `PalletId(*b"actors00").into_account_truncating()` under `AccountId32` and SS58 prefix `42`.

User actors derive sovereign accounts from `(PalletId, owner, owner_slot)`. Close releases the slot without moving native or registered-asset custody; the same owner may install a fresh Mutable recovery Contract at that exact slot, receiving a fresh actor id and nonce while reusing and accessing the same sovereign account. No rescue subsystem or custody transfer is involved. System actors derive accounts from `(PalletId, "system", sovereign_id)`; `ActorClass::System { sovereign_id }` carries that custody locator independently from the actor-id key. Fresh creation assigns the new actor id as a new locator. `SystemSovereigns` retains every allocated locator as `Vacant | Occupied(actor_id)`; governance may attach a fresh identity to a vacant locator without changing its account or residual balances. `ActorCreated.actor_class` carries `ActorClass::User { owner_slot }` or `ActorClass::System { sovereign_id }`; no separate event field duplicates either value.

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
| `build_treasury_lp_unwind_contract_steps` | Treasuries B/C/D | Return typed failure unless the asset is a registered local LP; otherwise remove it into Treasury custody |
| `build_bldr_splitter_contract_steps` | BLDR Splitter | Split minted BLDR share between liquidity and treasury lanes |
| `build_bldr_liquidity_contract_steps` | BLDR Liquidity Actor | Add NTVE/BLDR liquidity → transfer LP to BLDR Bucket A |
| `build_treasury_b_buyback_contract_steps` | Treasury B | Optional NTVE buyback → burn acquired target |
| `build_native_staking_liquidity_contract_steps` | Native Staking Liquidity Actor | Donate balanced `NTVE/stNTVE` without minting LP |

These builders configure the reusable task language; they do not create pallet-level roles or Actors-id policy branches.

## System Activation DAG

The DEOS runtime owns one bounded System activation manifest over known ids `0..=14`. Its nodes and ranks are descriptive host metadata, not Actor Contract fields. The only declared edge effect is a successful certified Actor `Transfer` or `SplitTransfer` into a known System sovereign whose active Contract selects `AddressEvent`.

| Source | Certified activation targets |
| --- | --- |
| Fee Sink | Native staking LP provisioning actor |
| Liquidity Actor | TOL Buckets A/B/C/D |
| TOL Buckets B/C/D | Treasuries B/C/D respectively |
| BLDR Splitter | BLDR Liquidity Actor; BLDR Treasury |
| BLDR Liquidity Actor | BLDR Bucket A |
| BLDR Bucket A | BLDR Treasury |

`DeosSystemActorContractValidator` checks every Active System installation and replacement against this manifest before Contract mutation. The runtime integrity gate ranks all manifest nodes with bounded Kahn traversal, rejects a cycle, and validates every genesis System Contract. The derived projection scans only the bounded known catalog, includes edges whose target currently has an active `AddressEvent` Contract, and remains read-only; runtime tests require every projected edge to belong to the manifest and prove an undeclared back-edge is rejected without changing the stored Contract.

The guarantee is deliberately closed-world. External Oracle publishers, ordinary users, market counterparties, and uncertified balance movement are outside this graph. Oracle publication enters the separately bounded transition-ingress contract; User cycles remain permitted and paid; uncertified movement never fabricates AddressEvent activation.

User cycles use the ordinary FIFO rather than a graph lane. Runtime evidence covers a funded two-Actor cycle with repeated-signal coalescing and alternating ticket order, an externally closed self-cycle that reaches economic apoptosis, and an eight-Actor ring that starts under full queue pressure, deterministically coalesces to one circulating ticket, remains paid while solvent, and closes an underfunded member when FIFO service reaches it. No User path receives the System fee exemption or executes one Actor twice in a block.

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

System swaps read the exact directional Oracle feed with `MAX_SYSTEM_REFERENCE_AGE_BLOCKS = 100` and enforce `ActorMaxSystemPriceDeviation = 5%`. Fresh nonzero truth at the exact age boundary remains eligible. Unavailable, Uninitialized, Stale, or invalid truth falls back to direct reserves. That fallback uses the same checked widened scaled-ratio primitive as DEOS Router publication; zero denominator, unrepresentable narrowing, unavailable fallback, or excessive deviation fails Temporary before mutation.

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

All supported producers use one typed certified-movement protocol and literal read-only preflight; no event scan, compatibility ring, deferred correctness layer, or silent balance-only fallback exists. Ordinary runtime adapters notify after movement inside their storage transaction. Signed-extension producers notify after successful dispatch and reject the candidate block if that consequence fails. XCM alone precommits the Actors consequence before consuming its non-cloneable holding, then commits or rolls back the deposit, Actors state, events, and holding together.

| Producer family | DEOS integration |
| --- | --- |
| Signed Balances/Assets transfer | Transaction extension carries one bounded direct candidate and verified signer provenance |
| `transfer_all` | Candidate resolves actual movement from recipient balance delta |
| Actors Transfer/Mint | `TmctolAssetOps` submits sender or source-less typed ingress inside task execution |
| TMC distribution | Mint-output adapter submits once and preserves available source provenance |
| Router fee routing | Fee adapter submits once with fee-payer provenance |
| XCM asset deposit | `ActorAwareAssetTransactor` submits one converted or source-less candidate |
| Privileged/delegated producers | Transaction extension carries one source-less certified candidate |

XCM binds generated one-asset deposit weight and `MaxAssetsIntoHolding = 1`, preventing one instruction from multiplying synchronous Actors ingress work without a corresponding instruction-specific weigher.

User actors default to `OwnerOnly`; accepted verified owner or allowlist transfers may add to bounded funding accumulators. System actors default to denied `RuntimePolicy`. Source-less or rejected provenance still creates spendable ledger balance but does not gain tracked funding authority.

### Crediting-Producer Inventory

Every certified producer path that can credit an Actors sovereign account routes through `RuntimeAddressEventIngress`. The inventory names each path, typed protocol, credited surface, source/provenance semantics, preflight owner, consequence owner, rollback witness, and Weight owner. Paths outside this closed inventory are balance-only.

`SourceFilter::Any` accepts every certified source that passes the authored asset filter. Any such source may set the actor's pending latch, and a resulting User attempt spends that actor's Weight-derived fee budget even when the sender acts only to force evaluation. DEOS adds no hidden sender trust list, reimbursement, or anti-grief pricing policy; authors who cannot accept that exposure use `OwnerOnly` or an explicit bounded whitelist.

**Ingress producers (credit another actor's sovereign):**

| Producer path | Protocol | Source / provenance | Preflight owner | Consequence owner | Rollback witness | Weight owner |
| --- | --- | --- | --- | --- | --- | --- |
| Signed Balances/Assets transfer | `BlockAtomicPostDispatch` | Signer / `Signed` | Extension `prepare` | `post_dispatch_details` | Block author/import transaction | Extension base + notify |
| `transfer_all` | `BlockAtomicPostDispatch` | Signer / `Signed`, actual delta | Extension `prepare` | `post_dispatch_details` | Block author/import transaction | Extension base + notify |
| Privileged/delegated movement | `BlockAtomicPostDispatch` | Source-less / none | `prepare_dynamic_producer` | `post_dispatch_details` | Block author/import transaction | Extension base + notify |
| Actors Transfer | `PostMovementNotify` | Sender / `InternalProtocol` | `TmctolAssetOps::transfer` | `on_internal_inbound` | Asset ops transaction | Transfer/split generated weights |
| Actors Mint | `PostMovementNotify` | Source-less / none | `TmctolAssetOps::mint` | `on_inbound_without_source` | Asset ops transaction | Mint generated weight |
| TMC distribution | `PostMovementNotify` | Mint source / `InternalProtocol` | Distribution preflight hooks | `after_distribution` | TMC transaction | Distribution generated weights |
| Router fee routing | `PostMovementNotify` | Fee payer / `InternalProtocol` | `route_fee` | `on_internal_inbound` | Router transaction | Router fee weights |
| XCM asset deposit | `XcmTransactionalPrecommit` | XCM origin / `Xcm` | `preflight_xcm_inbound` | `precommit_ingress` | Asset transactor transaction | One-asset deposit weight |
| XCM without origin | `XcmTransactionalPrecommit` | Source-less / none | `preflight_inbound_without_source` | `precommit_ingress` | Asset transactor transaction | One-asset deposit weight |

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

Fee-collector ledger movements are intentionally excluded from certified AddressEvent ingress. Actors now charges generated Manual, matching AddressEvent, affected-Actor ObservationChange, fired-Actor ObservationCrossing, due AtTime, and due Cadenced occurrence owners, but that collector credit—like transaction-payment, governance-opening, and XCM fee credits—reaches the Fee Sink ledger without recursively creating trigger or funding state; its single cadence owns allocation. `ACTORS_ADDRESS_EVENT_PRODUCER_INVENTORY`, generated ingress evidence, paired-executive tests, and the embedding fixture continue to cover paths that actually signal an actor.

## Fee Composition

Manual, AddressEvent, ObservationChange, ObservationCrossing fire, AtTime, and Cadenced are the implemented Trigger-fee families. Each family performs Actor-specific Trigger work and charges its generated occurrence owner only for a useful `pending_signal: false -> true` transition. Redundant latched activity performs no Actor-specific evaluation, fee, event, activation, or causal-history accumulation. Source-owned movement, funding accumulation, and authoritative Observation state may continue independently. Opening re-arms stateful Trigger families from current authority; AtTime remains one-shot consumed; Cadenced resumes from the first deadline strictly after the current authoritative tick. Underfunding or collector failure creates no readiness or apoptosis. A homogeneous Crossing batch whose collection cannot complete rolls back its aggregate attempt and advances through the already-admitted scalar owner, preventing a free retry loop without charging the publisher. The package-owned `PipelineMachineEnvelope` binds complete bounded control/cleanup pricing in the certified Contract head. When Idle paid readiness is consumable, the scheduler charges that total before Opening; one-unit shortfall selects `CycleAdmissionInsufficient` process cleanup without refunding prior Trigger fees. Running/Suspended service performs no machine affordability or collection. Current-Step resource admission still validates post-dispatch control/effect evidence component-wise, but `StepFeeBreakdown` reserves and settles only the current Action effect. Non-invoked effects, false predicates, skipped resolution, `FundingUnavailable`, and `StopCycle` produce no Action collection.

Package-generated `actors-fee-envelope-vectors.json` constrains the browser's Action-only suffix reservation and protected-floor behavior. Runtime-generated `actors-cost-vectors.json` binds metadata and Actors Weight hashes to separate `ActorCostApi` owners across Manual `0/1/4/8/32` geometry, every Trigger family at one Step, dormant User absence, and explicit System exemption. `runtime/examples/actor_cost_vectors.rs`, `automation/cost-vectors.ts`, Actors assurance, and full regeneration own generation, fail-closed parsing, freshness, and drift evidence. Visible presentation remains open.

Runtime Pipeline projection binds the generated zero-Step owner (`44,211,000 / 4,952`, five reads and three writes), every C6 Step-control branch across authored retry counts, and exact generated `close_actor` cleanup (`736,628,000 / 81,886`, 64 reads and 63 writes) rather than the broader lifecycle maximum. Runtime settlement charges a Pipeline once per admitted Cycle and charges every invoked success or typed adapter failure from valid actual effect Weight. Each committed Action-bearing attempt appends `ActionFeeCharged` with `(actor_id, cycle_nonce, step_index)`, actual effect Weight, and the exact User charge or System zero after semantic boundary events; non-invocation and `StopCycle` emit no Action receipt. Pipeline, Action, collection, or evidence failure rolls back queue consumption, Step state, effects, placement, close, fees, and events within the owning current-Step transaction.

`TmctolFeeCollector` transfers the complete charge into Fee Sink System Actor `1` through a ledger-only movement, matching transaction-payment and XCM-trader collection without fabricating AddressEvent readiness. One 120-tick cadence under the 500 ms consensus clock owns a stable 60-second allocation period. Its plan processes 10% of the current spendable Native buffer only when each configured split leg receives at least one ED, allocates the processed amount under the current phase policy, and retains the unprocessed buffer plus indivisible remainder. Runtime regressions prove collection-only custody, threshold skips, timestamp readiness, and no early execution.

The runtime-upgrade integrity gate re-derives the complete Fee Sink/native-security topology. It requires the exact Mutable persistent RuntimePolicy cadence contract and native 10% split; mode-shaped `50/50` staking/liquidity or `34/33/33` security/staking/liquidity legs with total share one; a retained liquidity System locator; one shared collector/actor custody account; distinct Fee Sink, staking ingress, security reward, liquidity, and LP-lock accounts; and ED anchors for every endpoint admitting arbitrarily small native flow.

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
| Crossing worker | 10% of maximum block Weight; admits one complete generated worst-case unit |
| Wakeup worker | 14% contribution to the shared materialization envelope; block-keyed Pipeline service and tick-keyed Trigger detection use independent Actor pointers |
| ObservationChange fanout | 20% contribution to the shared materialization envelope |
| `MaxCrossingMembersPerFeed` | 10,000 total memberships |
| `MaxUserCrossingMembersPerFeed` | 9,000 User memberships; remaining 1,000 positions are System-only |
| `MaxCrossingActorsPerBlock` | Four candidates through two reachable homogeneous pair cohorts |
| `MaxContractSteps` | Configured maximum remains within `0..=255`; each User or System Contract admits `0..=32` Steps under the DEOS production binding |
| `MaxRetryAttempts` | 10 cursor-local unsuccessful attempts |
| `MaxConsecutiveFailures` | 10 |
| `MaxAutoCloseNonceHorizon` | 10,000 |

The fixed context owner uses measured DMP plus the smallest component-wise XCMP residual and outer enqueue bookkeeping whose sum dominates the complete maximum-context benchmark; it does not retain the superseded 25% fictitious handler reserve.

The production resource algebra partitions the Normal block envelope component-wise into Actor Control `304,686,077,576 / 676,150` and Shared Economic `609,372,155,152 / 1,352,300`. Shared Economic grants equal base turns of `304,686,077,576 / 676,150` to Actor effects and signed user dispatch. Either side may borrow unused capacity from the other; neither may consume the other's guaranteed base while both are saturated. RefTime and ProofSize fragment independently and unused capacity in one component cannot compensate for exhaustion in the other.

The block sequence is `context inherents -> Mandatory Actor Prepass -> signed user dispatch -> Actor Drain -> on_finalize`. The node supplies the payload-free versioned prepass inherent exactly once after Timestamp and parachain context; runtime phase guards reject absence, duplication, staleness, or ordering after signed dispatch. Prepass freezes one FIFO cutoff and reaches `ExternalPhase`; Drain preserves that cutoff through `FreshDrain`, then generated finalization reaches `Finalizable`. `on_finalize` consumes only a matching current-block marker with zero outstanding reservations.

Prepass and Drain reserve maximum Actor Control before mutation and settle generated actual control afterward. Actor Actions reserve Shared Economic maxima from the Actor base turn and reclaim valid actual Weight transactionally. `BlockResourceMeterExtension` performs the equivalent maximum reservation and valid-actual reclaim for signed calls from the user base turn. Internal accounting and FRAME block Weight are reconciled fail-closed; uncertainty, overflow, stale state, and inconsistent post-dispatch evidence halt resource service rather than fabricate capacity.

`ActorResourceApi` exposes the configured two-dimensional budget, optional current authoritative block state, and latest bounded finalized non-authoritative snapshot as named SCALE projections. `ActorEligibilityApi` v6 replaces tuple-shaped fault/capacity output with `MaterializationFaults` and `CrossingCapacity`; clients do not reconstruct domain usage, phase, semantic faults, or capacity dimensions from events and tuple positions.

Generated `BlockResourceMeterExtension` execution (`10,896,000 / 1,560`, one read and one write) reserves each signed extrinsic's complete declared maximum from Shared Economic state during prepare, settles valid `PostDispatchInfo` actual Weight after dispatch, and rejects missing, stale, over-capacity, or inconsistent state with distinct transaction-invalidity evidence. Runtime block policy assigns 50% to dispatch and 50% guaranteed `on_idle` headroom. No dedicated Operational reserve exists while no concrete critical Operational call consumes one. `ActorOnIdleReserve` binds directly to `1,000,000,000,000 / 2,500,000`.

Resource acceptance is layered rather than implemented as a second execution engine. `types::resource::tests` exhausts empty, partial, saturated, borrowing, component-fragmented, settlement, and corruption algebra under the protocol-fixed equal-thirds budget selected by `actors/EXP-0022`. `production_full_block_resource_harness_records_actor_and_user_contention` composes the real mandatory prepass, one Actor effect, one signed external dispatch, Actor Drain, and finalized telemetry in one runtime block.

Wakeups, Crossing, and broad fanout contribute `14%`, `10%`, and `20%` of maximum block Weight to one `44%` shared materialization envelope. Generated maximum-unit minima for all three families fit together with positive RefTime and ProofSize remainder. A persisted cursor rotates the first family, reserves every later minimum, lends the remainder to the first family, and may return trailing unused reservations through one counter-preserving second grant. The `25,283,000 / 5,982` coordinator owner includes ten reads and one write for cursor state and bounded remaining-work classification. Fixed scheduler base, coordinator, mandatory cleanup, and the shared envelope are subtracted once; the residual is the exact Actor execution floor.

`ActorFunding` stores bounded tracked-asset funding only; it carries no Trigger-family amount or machine-fee ledger. Fresh-genesis Actors storage version `15` includes C6 fragment-local resources, compact activation authority, the control-only `PipelineMachineEnvelope`, split run head/payload state, exact User Crossing counts, the materialization-family cursor, one transient block-resource state, and one latest finalized non-authoritative telemetry snapshot; the pre-`1.0` reference line defines no deployed-state migration.

System and User actors share one paged FIFO with a single global ticket order; no actor class reserves an execution share, weight slice, or service right (spec 8.1.8). Housekeeping, observation fanout, wakeup work, cleanup, and admission bookkeeping remain outside the actor-service pass; within that pass, strict head-of-line order and the shared cutoff alone bound service.

Creation, activation, fresh custody reattachment, and plan replacement reject any Actor Contract whose generated scheduler, complete bounded Pipeline Machine, and pure cleanup resources do not fit the guaranteed two-dimensional envelope. User sovereign service funding is not required until a ready Opening.

## Reactive Delivery Evidence

The runtime binds `MaxObservationFanoutPagesPerBlock = 64` with a `400,000,000,000 / 1,000,000` fanout contribution and `MaxWakeupsPerBlock = 512` with a `280,000,000,000 / 700,000` wakeup contribution to the shared envelope. Compact activation authority keeps ordinary fanout independent of unused maximum Contract resources. Base plus branch probe, the component-wise queue/wakeup/coalesced/blocked maximum, and reserved fault-record Weight admit two complete ordinary pages before RefTime binds.

| Production topology after publication | Fanout service units | Blocks at one unit |
| --- | ---: | ---: |
| One feed with 10,000 subscribers | 157 | 79 |
| One sparse subscriber at a historical high page id | 1 | 1 |
| Four dirty feeds with 10,000 subscribers each | 628 | 314 |
| One revision restart after quiescence | At most 314 | 157 |
| Persistently saturated queue | Unbounded | Unbounded |

An ordinary fanout service unit is one exact subscriber-page turn. Capacity pressure retains the exact page and subscriber position for next-block retry; terminal cleanup uses a separate scalar turn. The finite rows require stable active topology, no newer selected-feed revision, available configured RefTime/ProofSize budget, eventual queue or wakeup capacity, and same-finalized-block runtime/code/metadata/constants matching generated client evidence. Estimates end at fanout completion, not Actor execution.

Production P32 fanout base is `56,565,000 / 1,629`; the branch probe is `63,759,000 / 3,587`; and the component-wise ordinary maximum is `155,840,020,000 / 304,734`. Reserved fault record is `197,000,000 / 4,106`. One admitted ordinary page owns `156,100,779,000 / 312,427` after the base. Two pages plus base consume `312,258,123,000 / 626,483`; a third exceeds the `400,000,000,000` RefTime envelope, so RefTime binds before ProofSize.

The current Actors production generation used `frame-omni-bencher 0.22.0`, benchmark Wasm, 50 steps, and 20 repeats. `template/runtime/src/weights/pallet_deos_actors.rs` owns the complete current scheduler, wakeup, ingress, task, predicate, and orchestration coefficients. Those values price measured bounded topology only; they imply no throughput promise.

## Generated Evidence and Artifacts

`scripts/actors-assurance.sh` owns freshness checks for Actors semantic, fee-envelope, ABI, observation, ingress, weight, and metadata evidence. Production Wasm, metadata, descriptors, and generated client evidence remain owned by their build/export commands and checked directly by the `full` validation profile.

`template/runtime/src/weights/pallet_deos_actors.rs` owns complete generated methods and storage annotations. The converged handoff binds generated Actors Weight `ac206ec06b3f2c2789da23540ca1ae87d343c8d2196f77b1c12c43569c0d3b9e`, production Wasm `484d7f9aa9eb4d767bb7ecaefce05d6c50c7d25b9b381454872e50ee35272fb6`, and metadata `27984891721c42acbce79d4e458e9b40dd6b9a046228438a072f4f2c1bd0f74e`. `scripts/actors-assurance.sh` reports and preserves those identities while running exact named heavy profiles. Architecture records only load-bearing admission values; benchmark-host timing never becomes a chain-throughput claim.

### Generated Event Trace Corpus

These non-normative traces project the package event-order fixtures; Section 8 of the Actors specification and runtime metadata remain authoritative for semantics and fields.

| Scenario | Ordered event trace | Falsification anchor |
| --- | --- | --- |
| Fresh transfer cycle | `CycleStarted -> TransferExecuted -> CycleSummary(Completed) -> ActionFeeCharged` | Fresh simulation and package cycle-order tests |
| Temporary retry attempt | `CycleContinued -> StepFailed(Temporary) -> CycleSuspended(Temporary) -> ActionFeeCharged` | `continuation_*` and retry-bound package tests |
| Cancel on semantic update | `CycleCancelled -> CycleSummary(Cancelled) -> ContractUpdated` | Semantic replacement cancellation tests |
| Close with Continuation | `CycleCancelled(Closing) -> CycleSummary(Cancelled) -> ActorClosed` | Continuation close-order tests |
| Expiry during suspension | `CycleCancelled(Closing(WindowExpired)) -> CycleSummary(Cancelled) -> ActorClosed(WindowExpired)` | Window-expiry Continuation tests |

The corpus intentionally omits block numbers, balances, and exhaustive step fields. It illustrates ordering only and creates no history or indexer promise.

## Control Plane and Read Surfaces

Canonical active projection joins `ActorIdentities`, `ActorHot`, certified C6 Contract fragments, compact admission identity, bounded funding, and optional split run head/payload at one finalized block. Dormant identity, queue/wakeup membership, active-dirty topology, and bounded simulation results remain canonical-chain truth.

The runtime simulation API executes the same package evaluator and finalizer used by scheduler service. Its bounded records carry canonical `StepOutcome` values, including concrete failure cause plus retry disposition, and its status is the shared `AttemptDisposition`; DEOS adds no adapter-side simulation model.

Version 6 `ActorEligibilityApi` reports `NotRegistered`, `Dormant`, or the canonical Active classification at one finalized block, preserving terminal reason plus exact retry/block/timestamp-tick payloads; Crossing phase, installation revision, pending/processing revisions, latch, and placement are semantic fields. Its companion `materialization_faults` method projects only the bounded current Crossing, broad-fanout, and wakeup faults without pages, radix geometry, or fault history, while `crossing_capacity` returns the runtime's 9,000 User / 10,000 total per-feed policy and exact current semantic counts. The browser therefore never reimplements cadence, cooldown, schedule window, retry backoff, breaker, latch, or detector topology logic. It is canonical-chain truth at the queried block and never promises service, because queue position and available Weight decide actual admission.

`ActorCostApi` binds the DEOS `Balance` and returns one bounded canonical quote. User Creation Fee, current family-specific Trigger Weight/fee, upfront Pipeline Machine/cleanup amounts, current maximum Action-effect Weight/fee, and geometry-bound state hold remain separately named. Trigger provenance hashes the six generated occurrence owners; Pipeline provenance carries the admitted runtime/Weight identities; state-hold provenance exposes DEOS pricing of one ED per present component plus one `MICRO_UNIT` per retained SCALE byte. System Actors report zero Actors fees and explicit hold exemption without resource privilege.

`automation/cost.ts` projects `ActorCostApi` without recombining owners and maps committed `ActionFeeCharged` receipts by exact Actor/Cycle/Step coordinates; `adapters/blockchain/actor-cost.ts` invokes the typed API at one caller-supplied finalized hash and returns explicit unavailability instead of local fee inference. `automation/cost-vectors.ts` fail-closes malformed identities, totals, strategies, family/geometry coverage, System exemption, and dormant semantics before generated runtime vectors enter browser validation.

The browser Trigger editor rejects invalid hysteresis, discloses no-retrofire/rearm semantics, distinguishes typed User-capacity versus total-capacity atomic failures, directs authors to same-block `crossing_capacity`, warns that broad-fanout service scales with subscribed pages, and explains latched-fire coalescing plus FIFO backpressure. It exposes no Trigger-family bond quote because dispatch owns no such economic surface. The browser's authoring, artifact, matching-Wasm, simulation, observation, and governance-composition surfaces live under `web-client/src/lib/automation/` and `web-client/src/lib/observation/`. They bind metadata and runtime identity rather than recreating pallet semantics.

Unbounded history, archive search, forecasting records, governance preparation history, and longitudinal telemetry remain materialized-provider work under [`actors-control-plane.contract.en.md`](./actors-control-plane.contract.en.md) and [`read-model.contract.en.md`](./read-model.contract.en.md).

## Validation and Operations

Package tests own executable actor, scheduler, trigger, lifecycle, storage, and try-state behavior. Runtime tests own adapters, fees, ingress, genesis topology, Oracle/Router rollback, staking, XCM, generated-weight binding, block-budget partition, and full System/User composition.

Actors try-state reconciles all canonical partitions and their stored cardinalities, Continuation ownership, global FIFO pages/tickets/occupancy, both typed wakeup heaps and bucket reverse indexes, actor-local queue/wakeup pointers, owner slots, System sovereign locators, observation slot/page/feed ownership, occupied-page links, dirty-feed links/cursor/count, and revision baselines. Lazy queue and wakeup tombstones remain the only admitted physical records without live actor ownership.

The reference Router binds `RuntimeLpPairIntegrity`, so try-state also requires every bounded LP reverse entry to resolve to the exact Asset Conversion pool and LP token, requires that LP asset to exist, and requires complete pool/index cardinality equality. Missing indexes, wrong LP identities, orphan pairs, and deleted LP assets fail before Actors liquidity or Treasury unwind can consume the binding.

| Actors late-failure surface | Injected checkpoint | Restoration evidence |
| --- | --- | --- |
| Transfer / SplitTransfer | Certified placement rejection; second-leg adapter failure | Exact runtime root for direct transfer; all leg balances and no success event for split |
| Mint | Native issuance overflow after read-only preflight | Exact runtime root, including ledger, events, and Actors state |
| SwapIn / SwapOut | Adapter failure after input debit | Actor and pool custody restored; no swap event; only ordinary failed-step accounting may commit |
| Add / Remove Liquidity | First asset debit/credit fault; post-call minimum-output rejection | Package custody rollback plus exact runtime root across ledgers, pool, LP index, issuance, and events |
| Stake / Unstake | Adapter failure after asset/share burn | Custody and staking representations restored; no success event |
| Donate Liquidity | Failure after first asset burn | Both custody legs and donation accounting restored; no success event |
| FeeCollector | Ledger failure after admission | Exact runtime root; no fee receipt, sink movement, attempt mutation, or scheduler drift |

Each task executes inside one task-owned storage transaction. A rejected task restores its adapter writes and task success event; the surrounding attempt may still commit its specified `StepFailed`, counters, policy transition, and later independent steps. Tests distinguish that expected failure envelope from leaked partial adapter state.

`scripts/actors-assurance.sh` owns package portability, external embedding, runtime integration, scheduler fairness, dense/sparse liveness, 10,000-actor queue stress, Crossing relevance, and occupancy proof commands. Optimized-profile regressions prove that the 10,000-actor fairness/occupancy matrix serves every actor with nonce spread at most four, that 10,000 wakeups due at one block drain completely, and that a 10,000-member Crossing feed performs no placement on a zero-match transition before activating only an eight-actor crossed cohort with unique tickets. The same profile then converges all 10,000 memberships on one threshold, crosses and oscillates the maximum 625-page herd in exactly 10,002 atomic service units per transition, and proves exactly one unchanged live queue or deferred wakeup placement per actor under the 1,024-ticket package fixture. A 17-leaf range spaced across 113 binary orders preserves its suffix cursor after the mock Weight cap admits three leaves and converges in seven worker passes. These are service-unit and configured-fixture measurements, not user-facing time promises. A standard 512-actor mixed-clock regression advances timestamp by 99 ticks at once and proves per-unit clock round-robin, within-clock FIFO, one execution and one canonical placement per actor, bounded completion, and no cadence catch-up burst.

Benchmark-Wasm generation at 50 steps and 20 repeats owns every branch coefficient and exact database term. The current System cheap-cycle slope is `126,316,005 / 3,171` with seven reads and three writes; the alternating System/User slope is `192,451,310 / 3,361` with 11 reads and five writes. The mixed `9,500` Transfer, `400` SwapOut, and `100` control-Actor profile completes starvation-free first traversal in block `679` under continuous signed user dispatch, with zero failed Steps inside the measured `1,301`-block horizon. This is a reproducible production profile, not an execution SLA; congestion and fragmented component capacity may delay any individual Actor.

## Reactive Capacity Ledger

The current production policy has a `44%` shared materialization envelope, exact generated minima for all three families, four Crossing candidates per block, 9,000 User memberships per feed, 10,000 total memberships per feed, a 10,000-entry FIFO, 512 wakeup scans, and 64 broad-fanout pages. Crossing branch billing selects one mutually exclusive generated execution owner after the common and branch probe; the component rows below are not summed as independent whole-branch maxima.

| Crossing path | Generated owner RefTime / ProofSize | Candidate ceiling | 10,000-candidate reference drain |
| --- | ---: | ---: | ---: |
| No-match transition | `32,895,000 / 6,060` transition; staged probes remain separate generated owners | 0 | Not a candidate drain |
| Post-installation skip pair | `52,522,000 / 6,156` probe; `74,033,000 / 6,156` execution | 4/block | 2,500 blocks |
| Rearm pair | `86,815,000 / 10,482` probe; `476,185,000 / 162,782` execution | 4/block | 2,500 blocks |
| Coalesced fire pair | `334,335,000 / 10,482` fire-pair probe; `657,217,000 / 162,782` execution | 4/block | 2,500 blocks |
| Placed fire pair | `334,335,000 / 10,482` fire-pair probe; `652,398,000 / 162,782` execution | 4/block | 2,500 blocks |
| Maximum placed batch | `998,886,000 / 81,886` execution after staged probes | 4/block | 2,500 blocks |
| Non-tail placed batch | Component-wise maximum of `795,994,000 / 81,886` emptied-tail and `801,232,000 / 81,886` trimmed-tail execution | 4/block | 2,500 blocks |
| Terminal fire | Generated fire probe; `693,745,000 / 162,782` cleanup execution | 4/block component cap | 2,500 blocks |
| One same-threshold page | Funded scalar owner `560,485,000 / 162,782`; homogeneous batches select their dedicated owner | 4/block | 2,500 blocks |
| Maximum homogeneous herd | Maximum placed-batch path under the shared envelope | 4/block | 2,500 blocks, or 4h10m at six-second blocks |

The ledger separates zero-match transition setup, sparse occupied-leaf search, source-page traversal, rearm, coalesced fire, placed fire, and terminal cleanup. The current search owner still couples a newly found sparse threshold to its first candidate; larger candidate-count cohorts and measured sparse-search amortization remain open, so these values must be reselected before final release evidence.

Every drain figure assumes one ready homogeneous branch, solvent Actors, available queue positions and tickets, no materialization fault, production block reserve, and no competing family consumption beyond its protected minimum. Producer rate, queue occupancy, mixed thresholds, paused or already-latched Actors, insolvency, branch changes, and the rotated lending order can change observed completion. These are bounded capacity facts, not a publication-rate, activation-latency, task-success, wall-clock, or economic-success SLA.

The runtime Pool Index extension fault anchor admits a pool-creation call, injects failure only at post-dispatch LP/Oracle indexing, rejects the block candidate, and proves exact storage-root restoration across the pool, LP asset and reverse index, Oracle feeds, balances, events, and signer nonce. Package Router faults separately prove exact-root rollback when the second market leg or second directional Oracle publication fails after earlier pool, fee, and publication work.

The runtime cross-pallet hook-rejection anchor fills Actors dirty capacity, attempts direct Oracle publication, and proves Oracle observation/revision, Actors feed/list state, and runtime events equal the captured pre-state. After capacity recovery, one producer retry commits Oracle revision `1` and Actors latest revision `1`; no replay state or Router publication path participates.

`ACTORS_OBSERVATION_PUBLISHER_INVENTORY` closes the reference-runtime publisher set to `DEOS Oracle::OnObservationChanged`. The Oracle hook reaches Actors through `ObservationTransitionIngress` with the exact revision and previous/current scalar values; no second runtime publisher owns transition progression. Generated observation evidence scans the complete runtime-config Rust tree, requires exactly one Oracle-owned typed ingress call, and rejects direct broad-fanout bypasses.

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
