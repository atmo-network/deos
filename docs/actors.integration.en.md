# DEOS Actors Integration

## Purpose and Ownership

This document maps how the DEOS reference runtime composes reusable `pallet-deos-actors` with deterministic System identities, TMCTOL Actor Contract families, DEOS Router, Oracle, assets, staking, fee collection, XCM, governance, generated weights, and browser/control-plane surfaces.

The portable actor contract and crate implementation remain in [`template/pallets/actors/docs/specification.en.md`](../template/pallets/actors/docs/specification.en.md), [`template/pallets/actors/docs/architecture.en.md`](../template/pallets/actors/docs/architecture.en.md), and [`template/pallets/actors/docs/embedding.md`](../template/pallets/actors/docs/embedding.md). This document owns only concrete DEOS composition.

## Integration Code Map

| Surface | Anchor |
| --- | --- |
| Runtime adapters, actor builders, bounds, and origins | `template/runtime/src/configs/actor_config.rs` |
| Runtime-generated Actors weights | `template/runtime/src/weights/pallet_deos_actors.rs` |
| DEOS Oracle observation provider (hooks bound to unit) | `template/runtime/src/configs/oracle_config.rs` |
| Router fee, quote, execution, and observation composition | `template/runtime/src/configs/deos_router_config.rs` |
| Asset and transaction-extension ingress | `template/runtime/src/configs/assets_config.rs`, `template/runtime/src/lib.rs` |
| Genesis System identities and ED anchors | `template/runtime/src/genesis_config_presets.rs` |
| Runtime integration and load evidence | `template/runtime/src/tests/actor_integration_tests.rs`, `template/runtime/src/tests/load_testing.rs` |
| Off-chain artifacts and simulation | `docs/actors-control-plane.contract.en.md`, `web-client/src/lib/automation/` |

## Temporal Binding

The six-second DEOS slot binds block cooldown and window horizons through `ActorMaxExecutionDelayBlocks = 52_596_000`, exactly `ceil(10 × 365.25 days / 6 seconds)`. AtTime and Cadenced use consensus timestamp through `ActorCadenceTickMillis = 500` and independent `ActorMaxTemporalDelayTicks = 631_152_000`, exactly `ceil(10 × 365.25 days / 500 milliseconds)`. These typed horizons are never converted or reused across clocks; retry backoff remains separately protocol-capped.

## Actor-State Holds and Capacity

The DEOS runtime maintains no Trigger-family bond, rent, or fee reserve. It binds `pallet-balances` as `StateHoldCurrency`, `RuntimeHoldReason::Actors(ActorState)` as the dedicated reason, one ED as each priced component's fixed base, and one `MICRO_UNIT` per priced SCALE byte, including reserved control and Run capacity. `ActorStateHolds` separates identity, Contract head/body, detector topology, and Run capacity per User Actor. Dormant Actors release Active components, while System Actors are hold-exempt host capacity. Lifecycle changes reconcile owner hold deltas transactionally, and close never touches sovereign custody. Active User head pricing reserves one 36-byte resource slot independently of Step count; actual head/admission and tail bytes are additional owners. Runtime cost vectors bind that quote, while System Actors remain exempt.

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
| 12 | BLDR Anchor | `0x791ec3fe30f34d005232cdf3bb5abdc0ae14e51fe3caeb62914d35f7c81ae544` | `5EoWnoVuB925BHs9UwHUfLkcm5rSbmqzrHgFZRzY5nA4M5B6` |
| 13 | BLDR Treasury | `0x07297bfba697b7593a93b6bc2c52f7dc4452d968c1e2c3badb09f2fafb8d1709` | `5CE6WsJ12vyyjAPMuvaqf2cdSQMVzAAxVjZDvXZK99VswFGe` |
| 14 | Native staking LP provisioning actor | `0x14292af3e9e70acb4c39cfe83317039c1f2111b475b99e660d87b16948edc339` | `5CX93X5agA9cbvbv4JKpXmR8RF9ywdLbyg6WR9qY15evri5L` |

## Genesis Topology

| Lane | Role | actor_id | Genesis lifecycle |
| --- | --- | ---: | --- |
| Core | Burn Actor | 0 | Active burn plan |
| Core | Fee Sink | 1 | Active 120-tick/60-second 10% buffer allocation |
| Core | Liquidity Actor | 2 | Dormant |
| TOL | Bucket A | 3 | Dormant Immutable Anchor |
| TOL | Buckets B/C/D | 4–6 | Dormant |
| Treasury | Treasuries B/C/D | 7–9 | Dormant |
| `$BLDR` | BLDR Splitter | 10 | Active 50/50 split |
| `$BLDR` | BLDR Liquidity Actor | 11 | Dormant |
| `$BLDR` | BLDR Anchor | 12 | Dormant Immutable Anchor |
| `$BLDR` | BLDR Treasury | 13 | Dormant |
| Staking | Native staking LP provisioning actor | 14 | Dormant |

Active genesis Actors use the runtime System cooldown, `ActorType::System`, `Mutability::Mutable`, and no schedule window. Fee Sink's tick-zero Trigger deadline anchors its 120-tick period from the first consensus timestamp without executing allocation. Twelve dormant entries occupy `ActorIdentities` and `SovereignIndex` without Contract, process residence, detector, fee, or Active-epoch state: ten are Mutable activation candidates, while Bucket A and BLDR Anchor are sealed Immutable identities that reject activation, mutation, deactivation, and close. The runtime LP freezer admits incoming LP to both Anchors, exposes zero reducible LP balance to ordinary, admin-forced, and internal transfer/burn paths, and blocks destruction of every LP asset class.

The reference runtime configures two sealed dormant System Immutable identities and no active System Immutable Contract. Bucket A and BLDR Anchor therefore have no Actor-level emergency close, deactivation, or custody disposition: changing their identity or LP-freeze contract requires an explicit runtime upgrade or fork. A downstream runtime that admits an indefinite active System Immutable actor must ship its migration-specific source/target actor set, bounded Close or Deactivate disposition, custody handling, terminal invariant, and Continuation policy with the same upgrade. The ordinary DEOS Governance path exposes 3-day lead-in, 7-day vote, 7-day protection, and 3-day enactment delay—20 days before bounded maturity/operational delay. Protocol `L1RootAction` can use the separately governed 24-hour urgent path only with unanimous raw protection-track `Pass`; Actors promises neither path completes within a finite time.

`ActorIdentityCount` covers all fifteen active plus dormant identities. `NextActorId = 15` preserves the reserved address range. Every expected small-native-flow System sovereign account receives one persistent free-balance ED anchor because a provider or reserved balance alone does not make a zero-free account eligible for sub-ED native ingress under `pallet-balances` v50.

## Actor Contract Families

The runtime keeps TMCTOL policy declarative through builders in `actor_config.rs`.

| Builder | Actor family | Composition |
| --- | --- | --- |
| `build_burn_contract_steps` | Burn Actor | Foreign balances → Native swap → burn |
| `build_fee_sink_contract_steps` | Fee Sink | Above the per-leg ED threshold, process 10% of spendable Native → phase-aware allocation |
| `build_zap_contract_steps` | Liquidity Actor | Add LP → surplus swap → split LP to buckets |
| `build_bucket_lp_transfer_contract_steps` | Buckets B/C/D | Transfer bounded LP fraction to paired Treasury |
| `build_treasury_lp_unwind_contract_steps` | Treasuries B/C/D | Return typed failure unless the asset is a registered local LP; otherwise remove it into Treasury custody |
| `build_bldr_splitter_contract_steps` | BLDR Splitter | Split minted `$BLDR` share between liquidity and treasury lanes |
| `build_bldr_liquidity_contract_steps` | BLDR Liquidity Actor | Add `$NTVE/$BLDR` liquidity → transfer LP to BLDR Anchor |
| `build_treasury_b_buyback_contract_steps` | Treasury B | Optional `$NTVE` buyback → burn acquired target |
| `build_native_staking_liquidity_contract_steps` | Native Staking Liquidity Actor | Donate balanced `$NTVE/stNTVE` without minting LP |

These builders configure the reusable task language; they do not create pallet-level roles or Actors-id policy branches.

The Builder Economy contract classifies BLDR Treasury as a Mutable System Actor treasury whose sovereign account may be debited by domain governance without mutating its Actor Contract. Genesis currently retains actor id `13` as a Dormant System identity, so a later activation may install an independent bounded treasury plan while governance and Actor execution remain serialized consumers of the same custody. Governance architecture owns the current invoice-settlement convergence gap.

The current Treasury B builder is narrower than the [Builder Economy contract](./builder-economy.contract.en.md): it consumes only a bounded percentage of Native and burns all target output. The target composition preserves gradual Bucket B LP transfer and paired Treasury unwind, then routes both resulting reserve assets into `$BLDR` and divides recipient output equally between burn and BLDR Treasury. Because existing Steps commit independently, the runtime change must specify route order, retained-prefix custody, retry behavior, liveness inspection, and Weight before this integration document can describe that target as shipped.

## System Activation DAG

The DEOS runtime owns one bounded System activation manifest over known ids `0..=14`. Its nodes and ranks are descriptive host metadata, not Actor Contract fields. The only declared edge effect is a successful certified Actor `Transfer` or `SplitTransfer` into a known System sovereign whose active Contract selects `AddressEvent`.

| Source | Certified activation targets |
| --- | --- |
| Fee Sink | Native staking LP provisioning actor |
| Liquidity Actor | TOL Buckets A/B/C/D |
| TOL Buckets B/C/D | Treasuries B/C/D respectively |
| BLDR Splitter | BLDR Liquidity Actor; BLDR Treasury |
| BLDR Liquidity Actor | BLDR Anchor |

`DeosSystemActorContractValidator` checks every Active System installation and replacement against this manifest before Contract mutation. The runtime integrity gate ranks all manifest nodes with bounded Kahn traversal, rejects a cycle, and validates every genesis System Contract. The derived projection scans only the bounded known catalog, includes edges whose target currently has an active `AddressEvent` Contract, and remains read-only; runtime tests require every projected edge to belong to the manifest and prove an undeclared back-edge is rejected without changing the stored Contract.

The guarantee is deliberately closed-world. External Oracle publishers, ordinary users, market counterparties, and uncertified balance movement are outside this graph. Oracle publication has no Actors consequence; User cycles remain permitted and paid; uncertified movement never fabricates AddressEvent activation.

User cycles use the same persistent Service ring rather than a graph lane. Runtime evidence covers a funded two-Actor cycle with repeated-signal coalescing and cyclic encounter order, an externally closed self-cycle that reaches economic apoptosis, and an eight-Actor ring that coalesces to one residence per Actor, remains paid while solvent, and closes an underfunded member when Service reaches it. No User path receives the System fee exemption or executes twice in one block.

The package architecture owns the exhaustive public reachability matrix. DEOS production builders currently instantiate the reference topology subset, while typed creation/update calls and the independent embedding runtime keep the remaining portable variants executable. Constructor-free runtime-upgrade cancellation and context-free amount dependency placeholders are absent; adding any public variant requires its constructor, evaluator/adapter branch, and executable evidence in the same change.

## Governance Activation Flows

`Foreign asset + TOL lane`: register the foreign asset, create the Native/foreign pool, extend the Burn Actor, activate the Liquidity Actor, then optionally activate paired Bucket transfer and Treasury unwind plans.

`$BLDR lane`: retain the BLDR Splitter at genesis, create the `$NTVE/$BLDR` pool, activate the BLDR Liquidity Actor, then optionally activate the current Native-only Treasury B buyback/burn policy. The dual-reserve burn/treasury bridge remains an explicit runtime convergence item.

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

`TmctolAssetOps` maps Native to `pallet-balances` and Local/Foreign to `pallet-assets`. Its transfer preflight covers source withdrawal and recipient deposit consequences. The reference runtime caps `SplitTransfer` at four recipients. Ordered legs all preflight before mutation; task rollback forbids partial fan-out. Certified ingress for each active Actor recipient belongs to the invoking Task effect even at this narrower cap.

`pallet-balances` v50 rejects a new zero-free account below ED even when FRAME already holds a provider. DEOS therefore endows expected small-flow System, custody, and staking-ingress accounts with one persistent free ED anchor and preserves it through amount resolution.

`TmctolLiquidityOps` delegates add/remove/donation to Asset Conversion while retaining ratio, LP receipt, and native-special-case policy in the adapter. `TmctolStakingOps` maps every Actor staking asset to the generic `stake(asset_id, amount)` call and resolves stable share assets through the staking receipt index.

Runtime adapters use typed failure classification. Explicit route, liquidity, slippage, oracle, and temporary-capacity failures may retry; malformed, forbidden, funding, fee, and unknown downstream failures remain Permanent.

`cadenced_module_failure_reaches_current_run_encoding_bound` in `template/runtime/src/tests/actors_integration_tests.rs` creates a Cadenced User, commits a skipped prefix, then reaches `RecipientDepositUnavailable` through real SplitTransfer recipient preflight. The retained Temporary module error populates the legal maximum Run shape. An in-memory comparison proves `FundingUnavailable` is smaller. This reference-runtime witness establishes reachable encoding geometry, not complete temporal Weight or artifact acceptance.

`cadenced_running_rearm`, `cadenced_suspended_service_rearm`, and `cadenced_suspended_deadline_rearm` reproduce the phase-specific maximum Run through the same real native SplitTransfer refusal. The Tick classifier selects a distinct non-useful busy owner; temporal rearm preserves the paid Run and its Service or Block-Deadline residence. Complete generated owners calibrate all three profiles, and the selector admits their component-wise maximum before mutation. Final artifact identities belong to generated evidence and release assurance for the exact source tree.

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

User actors default to `OwnerOnly`; verified owner or allowlist provenance may authorize useful AddressEvent readiness. System actors default to denied `RuntimePolicy`. Source-less or rejected provenance may still create ordinary spendable ledger balance but grants no readiness authority.

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

For Actors Transfer/SplitTransfer, the invoking Task effect Weight covers recipient preflight, ledger movement and ingress notification; do not add a separate `AddressEvent` source Weight for the same internal movement. The recipient's useful-readiness Trigger fee and the invoker's Action/Pipeline fees remain separate economic owners. `certified_ingress_inventory_is_closed_and_typed` binds this runtime attribution; it does not prove physical sufficiency. A selected four-User witness invokes the exact prepared SplitTransfer Task body, including the all-leg preflight, balance check, transfers, event and transaction; it exceeds the retained four-leg effect coefficient in both dimensions. The benchmark excludes surrounding Step control, and this selected effect witness is not a universal Task selector or a Weight/Wasm/ABI binding.

Certified ingress to an active `WindowExpired` recipient may synchronously finalize it inside the invoking Task effect. `four_leg_split_transfer_closes_expired_recipient_during_prepass_service` confirms this is reachable in the reference runtime even with four recipients and an admitted mandatory-Prepass Service turn: at least one recipient closes after its own ledger transfer. This selected test neither counts a universal maximum of inline closes nor prices terminal cleanup. The reference `TmctolTaskEffectWeight` chooses one maximum by `legs.len()` and settles that same reservation when invoked; recipient class, expiry, and native adapter side effects do not select a cheaper Task envelope. A matched prepared-Task diagnostic with one expired maximum-header User and three healthy ones exceeds its all-healthy comparator in both RefTime including DB and ProofSize. That selected comparison is not a universal close maximum or production coefficient, and it does not prove that its exact maximum-header geometry occurs in the tested scheduler interleaving.

**Same-actor task/adapter outputs (intentionally excluded from the ingress boundary):** each executes inside the actor's own Cycle and resolves against current sovereign state, so it does not signal the same Actor. They are named explicitly rather than claiming "all producers are covered":

| Output path | Signals current Actor | Certified ingress | Rationale |
| --- | --- | --- | --- |
| Swap output (exact-in/exact-out) | No | No | Debited from the actor's own sovereign; `SwapExecuted` event carries factual deltas |
| AddLiquidity LP issuance | No | No | LP minted to the actor's own sovereign; factual `amount_a`/`amount_b`/`lp_minted` event |
| RemoveLiquidity outputs | No | No | Underlying assets returned to the actor's own sovereign |
| DonateLiquidity debits | No | No | Factual debits within caps, own-sovereign movement |
| Staking shares / yield | No | No | `StakeExecuted`/`UnstakeExecuted` on the actor's own position; yield bridges via adapter |
| Unstake outputs | No | No | Own-sovereign return |
| StopCycle | No | No | No economic effect |

Movement to a non-Actors recipient and a task transfer to the actor's own sovereign remain explicit exclusions: `resolve_actor` returns `None` for non-sovereign recipients and the pallet's recipient validation rejects self-transfers with `SelfTransferNotAllowed`.

Fee-collector ledger movements are intentionally excluded from certified AddressEvent ingress. Actors charges generated Manual, matching AddressEvent, due AtTime, and due Cadenced occurrence owners, but that collector credit—like transaction-payment, governance-opening, and XCM fee credits—reaches Fee Sink without recursively creating Trigger readiness; its single cadence owns allocation. `ACTORS_ADDRESS_EVENT_PRODUCER_INVENTORY`, generated ingress evidence, paired-executive tests, and the embedding fixture continue to cover paths that actually signal an actor.

## Fee Composition

`RuntimeStepControlWeight` reserves one generated `fee_collection` allowance in Actor Control for each non-StopCycle Step, independently of Actor class. Opening, Running and Suspended actual control settle it only when a nonzero User Action fee must be collected; System and non-invoked attempts reclaim it. The existing bounded Pipeline machine envelope prepays this control work, while Action fees still derive solely from Task-effect Weight. This is separate from Pipeline admission collection. Base-selector Opening identity and component-wise reservation checks remain fail-closed.

`RuntimeStepControlWeight::user_opening_control_weight` supplies the same zero-tail and positive-tail User header profiles to maximum/actual selection and the production Control identity. The identity fingerprints every legal tail count; `control_weight_identity_binds_every_consumed_user_opening_profile` in `runtime/src/tests/actor_control_weight_identity.rs` rejects an omitted profile or either unbound Weight dimension. These are binding-integrity checks, not physical containment evidence. Complete generated ownership and final Weight/Wasm rebinding remain separate release gates.

Manual, AddressEvent, AtTime, and Cadenced are the implemented Trigger-fee families. DEOS Oracle publication is never a Trigger source; observation-reactive Actors pair a Cadenced or AddressEvent Trigger with a fresh-only observation predicate. Each family charges its generated occurrence owner only for a useful `pending_signal: false -> true` transition. Redundant latched activity performs no Actor-specific evaluation, fee, event, activation, or causal-history accumulation. Source-owned movement and authoritative Observation state may continue independently.

Opening re-arms temporal Trigger families from current authority; AtTime remains one-shot consumed; Cadenced resumes from the first deadline strictly after the current tick. Underfunding creates no readiness or apoptosis. Collector rejection instead rolls back exact source authority.

`PipelineMachineEnvelope` stores the Pipeline/cleanup quote from current host bindings in the certified Contract head. Idle readiness consumption charges that total before Opening; one-unit shortfall selects `CycleAdmissionInsufficient` cleanup without refunding the prior Trigger fee. Running or Suspended service performs no machine affordability or collection. Current-Step admission validates control and effect evidence independently; `StepFeeBreakdown` reserves and settles only the current Action effect. Non-invocation, false predicates, skipped resolution, `FundingUnavailable`, and `StopCycle` produce no Action collection.

Package-generated `actors-fee-envelope-vectors.json` constrains the browser's Action-only suffix reservation and protected-floor behavior. Runtime-generated `actors-cost-vectors.json` binds metadata and Actors Weight hashes to separate `ActorCostApi` owners across Manual `0/1/4/8/32` geometry, every Trigger family at one Step, dormant User absence, and explicit System exemption. `runtime/examples/actor_cost_vectors.rs`, `automation/cost-vectors.ts`, Actors assurance, and full regeneration own generation, fail-closed parsing, freshness, and drift evidence. Visible presentation remains open.

Runtime Pipeline projection binds the generated zero-Step owner, every certified Step-control branch across authored retry counts, and exact close cleanup rather than a broader lifecycle maximum. Runtime settlement charges Pipeline Machine control once per admitted Cycle and charges every invoked success or typed adapter failure from valid actual effect Weight.

Each committed Action-bearing attempt appends `ActionFeeCharged` with `(actor_id, cycle_nonce, step_index)`, actual effect Weight, and the exact User charge or System zero after semantic boundary events. Non-invocation and `StopCycle` emit no Action receipt. Pipeline, Action, collection, or evidence failure rolls back process residence, Step state, effects, close, fees, and events within the owning current-Step transaction.

`TmctolFeeCollector` transfers the complete charge into Fee Sink System Actor `1` through a ledger-only movement, matching transaction-payment and XCM-trader collection without fabricating AddressEvent readiness. One 120-tick cadence under the 500 ms consensus clock owns a stable 60-second allocation period. Its plan processes 10% of the current spendable Native buffer only when each configured split leg receives at least one ED, allocates the processed amount under the current phase policy, and retains the unprocessed buffer plus indivisible remainder. Runtime regressions prove collection-only custody, threshold skips, timestamp readiness, and no early execution.

The runtime-upgrade integrity gate re-derives the complete Fee Sink/native-security topology. It requires the exact Mutable persistent RuntimePolicy cadence contract and native 10% split; mode-shaped `50/50` staking/liquidity or `34/33/33` security/staking/liquidity legs with total share one; a retained liquidity System locator; one shared collector/actor custody account; distinct Fee Sink, staking ingress, security reward, liquidity, and LP-lock accounts; and ED anchors for every endpoint admitting arbitrarily small native flow.

Outside `runtime-benchmarks`, `RuntimeNativeSecurityModeProvider` fixes `TrustedSet`: successful security-reward funding is unavailable, while Fee Sink's active two-leg Native split sends one leg to the staking pool and one to the liquidity System Actor. When the native staking asset is registered, `TmctolAssetOps::transfer` converts the latter leg by slashing the received Native and minting the local staking asset before certified ingress; its conversion and ingress remain inside that SplitTransfer Task effect. The Fee Sink integration regression exercises this branch in TrustedSet. A matched-Wasm bridge diagnostic prepares an active liquidity Actor and invokes the complete prepared two-leg pool/bridge Task effect with certified ingress. Its measured source is not the Fee Sink, so the benchmark-only LP-backed security-reward route is **not invoked**: the pool/bridge effect branch matches the TrustedSet plan, while Fee Sink Step control and failure behavior remain unmeasured. Its selected value is below the stale projection, but neither this fixture nor an ordinary System-recipient comparator is a universal production bound.

A frozen registered native staking asset supplies a reachable late-failure witness in the TrustedSet reference composition. After the first staking-pool Native leg executes, the second leg's bridge rejects local minting with `AssetNotLive`; the enclosing Task rolls back **both** Native legs and emits `StepFailed`. `fee_sink_actor_splits_trusted_set_native_flow_to_staking_and_lp_ingress` tests that consequence. Neither the successful LP-backed benchmark nor the reverted final balances bound its physical RefTime or ProofSize.

Pure lifecycle cleanup charges no execution fee and runs no plan. User fee admission reserves transient Native fees without consuming the persistent sovereign ED anchor.

The reference precondition limits are four clauses, four predicates per clause, and four total predicates per Step. `RuntimeStepControlWeight` selects the component-wise maximum of generated asset-heavy, pure-asset, and observation-heavy current-evaluation owners for the certified current Step. Opening evaluates only Step 0 from current authoritative state; later Steps evaluate when reached. Amount resolution likewise reads current spendable balance or staking shares at each attempt. No predicate-result or amount snapshot is captured, stored, or priced.

## Runtime Bounds and Block Budget

| Bound | DEOS value and role |
| --- | --- |
| `MaxActiveActors` | 10,000 compile-time identities |
| `ActiveActorLimit` | Governance operational cap, never above active or Service hard capacity |
| Service ring | One actor-keyed persistent node per process residence |
| Deadline page size | 32 generation-bound temporal entries per retained page |
| `MaxQueueEntriesScannedPerBlock` | 10,000 bounded Service inspections |
| `MaxExecutionsPerBlock` | 1,000 defense-in-depth attempt ceiling |
| Deadline service | One pre-admitted Block-then-Tick quantum in mandatory Prepass, before the parked-balance dependency scan and Service; no repeated Drain quantum |
| `MaxContractSteps` | Configured maximum remains within `0..=255`; each User or System Contract admits `0..=12` Steps under the DEOS production binding, while independent hosts may select another bounded value |
| `MaxRetryAttempts` | 10 cursor-local unsuccessful attempts |
| `MaxConsecutiveFailures` | 10 |
| `MaxAutoCloseNonceHorizon` | 10,000 |

The fixed context owner uses measured DMP plus the smallest component-wise XCMP residual and outer scheduler bookkeeping whose component-wise sum dominates the complete maximum-context benchmark.

The current runtime composes `BlockResourceBudgetValue` from the global `MAXIMUM_BLOCK_WEIGHT` ceiling minus the fixed/context envelope, assigns Actor Control exactly one third of that schedulable remainder and assigns Shared Economic the complement. Shared Economic divides floor/remainder base turns between Actor effects and signed user dispatch. Either side may borrow unused capacity from the other, without consuming the other's guaranteed base under simultaneous saturation; both Weight dimensions fragment independently. The configured share now matches the resource-policy specification's **one-third** maximum. The live reference cap from the current fixed envelope is `303,433,283,909 RefTime / 2,504,736 ProofSize`; these are domain ceilings, not proof that a mandatory Prepass fits.

The approved [resource policy](./actors-resource-policy.specification.en.md) requires one post-prefix budget integrated with the Economic Zipper. Prepass now freezes the configured budget in `CurrentBlockResourceState`; Drain and `configs/resource_meter.rs::BlockResourceMeterExtension` use that snapshot, and finalization cannot substitute another budget. The finalized projection retains the same allocation. `prepare_uses_frozen_state_not_the_configured_budget` rejects a call fitting configuration but not its stored budget. Numerical limits remain static: prefix settlement, fixed-tail completion and a common FRAME/economic ceiling are not implemented; `BACKLOG.md` owns that gap. Resource-state encoding changed and requires coordinated Weight/metadata/Wasm evidence before release.

`actor_prepass` now declares the configured maximum block Weight rather than the smaller static Control-plus-base allocation, covering future quiet-prefix budgets without granting extra execution authority. `tests/production_block_replay.rs::full_executive_prepass_prefix_excludes_its_provisional_reservation` witnesses empty/one-Actor dispatch with that maximum temporarily booked above FRAME's ceiling, plus base/encoded overhead. Subtracting only the declaration preserves overhead; actual settlement and repeated SDK reclaim match ordinary Executive application's complete storage root. This certifies provisional accounting, not permission to execute beyond the frozen domains, a dynamic budget, a complete outstanding tail or production Wasm.

The prefix counter is not by itself a complete physical-cost certificate. The following source owners constrain normalization and the outstanding fixed allowance; rows distinguish returned Weight from side-effect registration and existing measured replacements.

| Fixed owner | Current accounting evidence | Unified-budget consequence |
| --- | --- | --- |
| Timestamp finalization | SDK `pallet-timestamp` `on_initialize` returns `WeightInfo::on_finalize` | Already prepaid in the prefix; no second tail reservation |
| Parachain finalization | SDK `cumulus-pallet-parachain-system` `on_initialize` prepays finalization DB work and announced outbound geometry | Preserve that charge; do not reserve the same declared work twice |
| Authorship callback | SDK `pallet-authorship` returns zero, but `pallet-collator-selection::note_author` registers its own generated Weight | Prefix includes the internal registration; zero hook return does not mean free work |
| Aura extension | SDK `cumulus-pallet-aura-ext` returns one read; finalization reads Aura authorities and writes its cache | Retain the not-yet-covered finalization owner |
| Session rotation | The hook returns its generated rotation owner; `start_session` independently registers reward settlement and trusted-mode contraction charges | The prefix contains both; close physical overlap before replacing owners, and reserve no rotation on non-rotation blocks |
| XCM version discovery | SDK returns partial DB charges; the retained generated profile omits its globally whitelisted queue and samples rejected routes only | Complete-owner certification remains open; neither the small SDK return nor the retained profile authorizes reserve release |
| Parachain context | `set_validation_data` registers enqueue work internally; the complete measured owner includes validation/metadata traversal | Preserve a sound complete owner until registered-charge dominance is proved for the admitted geometry |
| XCMP lazy migration | `XcmpQueue::on_idle` consumes its generated maximum even when the queue is empty | Still outstanding at freeze; preserve it independently of Message Queue service |
| Post-inherent, post-transaction and poll hooks | The prefix-boundary fixture checks zero meter usage and unchanged storage for the current empty/one-Actor cases | No work is observed in those fixtures; changed bindings require renewed tail classification |

Session-rotation benchmark setup matches production `TrustedSet`: it fills the invulnerable roster, active validators, old Aura authorities and queued keys to 20. Old and queued authorities differ only in the final byte of the last key, exercising the maximum shared comparison prefix and a real replacement. Verification checks exact replacement-key equality, not only counts. `session_rotation_benchmark_matches_trusted_production_topology` also proves reset from an explicitly selected LP-backed fixture. The former unconditional benchmark LP mode and 120-key queued geometry did not represent this production topology. The retained generated rotation Weight is unchanged; fixture correctness is not a smaller resource certificate.

Benchmark builds default to the production native-security mode. Portable staking fixtures opt into LP-backed selection through their existing helper; a benchmark-only raw selector is whitelisted centrally by the benchmark API, including absent reads. Production has neither selector storage nor its read. This isolates fixture selection from production storage evidence; the repaired temporary measurement is not a complete production binding.

`tests/actors_integration_tests.rs::session_rotation_hook_uses_the_generated_bounded_owner` separates the hook return from internal Mandatory registration and checks that a non-rotation call changes neither Weight nor session index. The trusted seeded rotation registers `settle_due_native_security_reward(14) + contract_native_security_obligations()` in addition to the outer return. `configs/mod.rs::frame_and_other_fixed_weight` currently includes only the outer rotation owner. This charge witness does not certify failure branches or justify adding overlapping physical costs; complete ownership remains required before the unified budget binding.

Aura's `MaxAuthorities` now uses the same associated type as Collator Selection's `MaxInvulnerables` (20). `configs/mod.rs::DelegationWeightedCollatorSessionManager` returns that bounded roster under production `TrustedSet`; SDK Session uses it at genesis and for subsequent queues, and Aura consumes the queued keys. `session_authority_bound_covers_declared_genesis_presets` builds both declared presets and checks authority/key parity. The maximum-rotation fixture also rejects an oversized roster through public administration and preserves the queued maximum until a smaller replacement activates. This fresh-genesis binding removes the unrelated 100,000-entry storage capacity; future LP selection requires a new bound and coordinated resource certification, not silent truncation. Generated Weight and production metadata/Wasm remain unchanged and require coordinated refresh.

The declared fresh trusted genesis has no native-security pots, snapshots or liabilities. Staking's `open_native_security_epoch` and `activate_native_security_epoch` are mode-gated producers; funding requires LP mode and an existing active pot, while claim/contraction paths require existing obligations. `tests/production_block_replay.rs::trusted_genesis_session_callbacks_preserve_empty_security_obligations` checks both presets, rejects producer ingress without mutation, and preserves empty obligation state and unsolicited custody across 14 scheduled rotations. Retained-state recovery tests fabricate their obligations directly and remain valid for that separate contract. The fresh-state argument does not authorize skipping inspection or removing portable settlement, nor does it certify raw storage injection, custom genesis or future LP-mode activation.

`configs/actor_config.rs::prepare_maximum_xcm_version_discovery` now removes only `VersionDiscoveryQueue` from the benchmark whitelist for this complete-hook profile; verification rejects a still-whitelisted queue. SDK `pallet-xcm` 29.0.0 marks that storage globally whitelisted, so the retained generated owner omitted its read, rewrite and measured proof. The repaired fixture includes those accesses, without changing other profiles or production execution. Its 100 short local `GeneralIndex` destinations all fail routing: successful UMP/XCMP notification, maximum location encoding and sort geometry remain outside this witness. The SDK hook's `reads_writes(1, 1)` return, plus another pair only after success, does not price the complete operation. No smaller reserve or prefix credit is certified by that return.

`tests/production_block_replay.rs::xcm_discovery_failed_prefix_then_ump_success_has_distinct_storage_owners` witnesses 99 rejected discovery destinations followed by a parent subscription through the runtime router. The successful hook locally appends one UMP message and installs query/notifier state absent from the failure-only benchmark; its returned Weight remains two DB read/write pairs with zero ProofSize. The test uses declared genesis plus synthetic host configuration, not network delivery or a full-Executive block. The sender's existing pending backlog remains a separate cost input: SDK `send_upward_message` traverses that vector after append, while its admission checks individual message size. A sound complete owner still needs the backlog bound and successful-route geometry, not just the discovery queue's 100-entry bound.

`ump_relay_capacity_is_not_a_local_pending_queue_bound` uses paid signed `RuntimeCall::dispatch` ingress to admit nine UMP messages totalling 1,224 bytes under synthetic relay queue fields of eight messages / 1,024 bytes. Each message remains within the separate 256-byte limit. Thus relay capacity is not the local pending queue's admission bound. SDK finalization exports only the prefix fitting current relay/per-candidate allowance and retains the suffix. This native dispatch witness excludes transaction extensions, complete-block admission and relay delivery; it establishes neither unlimited growth nor a certified maximum. Pricing, draining and complete-block constraints must not be mistaken for a proved local storage bound.

The full-Executive extension of that signed-send scenario fails on its first call with `Invalid(ExhaustsResources)`, before any send or finalization. The current generated `template/runtime/src/weights/pallet_xcm.rs::send` declares 18,446,744,073,709,551,000 RefTime, above even the 2,000,000,000,000 declared global ceiling. Consequently the direct-dispatch result is not evidence of transaction-admitted backlog growth. This binding blocks that ingress; it proves neither a pending-state invariant across hook/internal producers nor a bound that survives production Weight regeneration. Producer reachability must precede a complete successful-discovery cost claim.

The current UMP producer classes have distinct admission owners; the signed-send refusal does not cover them all. `configs/xcm_config.rs::XcmRouter` binds `ParentAsUmp<ParachainSystem, PolkadotXcm, PriceForParentDelivery>` only for the exact Parent destination. This source classification is not a complete path-cost or retained-state certificate.

| Producer class | Current admission boundary | Pending-queue implication |
| --- | --- | --- |
| Local XCM extrinsics | Generated send, transfer and subscribe/unsubscribe owners carry sentinel RefTime; local `execute` also has `XcmExecuteFilter = Nothing` | Direct dispatch tests do not prove transaction ingress |
| Governance payloads | `RuntimeProposalPayloadExecutor` exposes upgrade authorization, Router fee changes and treasury asset transfers, not arbitrary XCM calls | No current governance shortcut to UMP was found; an upgrade changes the baseline |
| Discovery hook | SDK `on_initialize` stops after one successful request; an existing `VersionNotifiers` entry rejects a duplicate destination | A per-hook success limit is not a bound on the shared pending vector |
| Migration/notification hook | SDK `lazy_migration` can notify stored targets through the router when migration state is present | Conditional internal production is outside the signed-send refusal |
| Inbound XCM executor | MessageQueue runs `ProcessXcmMessage`; Parent may use explicit unpaid execution; instruction count is limited to 100 and service has a Weight budget | XCM instructions can use `SubscriptionService`/`XcmSender` without dispatching pallet `send` |

Inbound `SubscribeVersion` calls `PolkadotXcm`'s `VersionChangeNotifier::start`, which sends a `QueryResponse` through the router before installing `VersionNotifyTargets`. `UnsubscribeVersion` removes that target, so the duplicate-subscription check does not exclude a later start from the same original origin. `SafeCallFilter = Nothing` blocks XCM `Transact`, not these instructions. The declared Parent unpaid barrier admits the full-Executive DMP witness below; it does not establish permissionless sender reachability on a live relay chain. `runtime-benchmarks` replaces the MessageQueue processor with a no-op, so an all-features test alone cannot certify production XCM execution.

`tests/production_block_replay.rs::parent_dmp_subscriptions_retain_ump_backlog_through_full_executive` authors and replays four linked native blocks from declared Development genesis. Synthetic relay proofs carry the actual cumulative DMP MQC; the real MessageQueue processor handles one subscription/stop pair, then two 17-pair programs. Each program passes the configured XCM instruction/Weight bounds. The test checks successful processing, stopped subscriptions, message conservation, retained FIFO order, individual-message/export limits and final backlog above both relay queue fields. It neither seeds pending UMP storage nor changes generated Weight.

| Block | Subscriptions processed | UMP enqueued | UMP exported | Pending messages | Pending payload bytes |
| --- | --- | --- | --- | --- | --- |
| 1 | 0 | 0 | 0 | 0 | 0 |
| 2 | 1 | 1 | 1 | 0 | 0 |
| 3 | 17 | 18 | 5 | 13 | 585 |
| 4 | 17 | 17 | 5 | 25 | 1,129 |

Block 3 includes one automatic discovery subscription in addition to the 17 incoming subscription responses. The synthetic relay fields remain eight messages / 1,024 bytes, with a 256-byte individual limit and five exported messages per candidate. Thus retained local backlog can exceed relay capacity after real native finalization despite the sentinel `send` Weight. This is a finite trusted-Parent context witness, not unlimited-growth, production-Wasm, physical maximum-cost or live-relay evidence. A complete sender/discovery cost owner still requires an explicit local pending-state bound; extending the burst alone cannot certify one.

The block sequence is `context inherents -> Mandatory Actor Prepass -> signed user dispatch -> Actor Drain -> on_finalize`. The node supplies the payload-free versioned prepass inherent exactly once after Timestamp and parachain context; runtime phase guards reject absence, duplication, staleness, or ordering after signed dispatch. Prepass services due Deadlines and the bounded parked-balance dependency scan, then reaches `ExternalPhase`; Drain serves one canonical Service round through `FreshDrain`, then generated finalization reaches `Finalizable`. `on_finalize` consumes only a matching current-block marker with zero outstanding reservations.

Prepass and Drain reserve maximum Actor Control before mutation and settle generated actual control afterward. Actor Actions reserve Shared Economic maxima from the Actor base turn and reclaim valid actual Weight transactionally. `BlockResourceMeterExtension` performs the equivalent maximum reservation and valid-actual reclaim for signed calls from the user base turn. Internal accounting and FRAME block Weight are reconciled fail-closed; uncertainty, overflow, stale state, and inconsistent post-dispatch evidence halt resource service rather than fabricate capacity.

`ActorResourceApi` exposes the configured two-dimensional budget, optional current authoritative block state, and latest bounded finalized non-authoritative snapshot as named SCALE projections. `ActorEligibilityApi` v6 exposes only the named `actor_eligibility` projection; clients do not reconstruct domain usage or phase from events and tuple positions.

`BlockResourceMeterExtension` uses SDK `calculate_consumed_extrinsic_weight` to reserve call plus extension Weight, class base cost and encoded-length ProofSize from the frozen Shared Economic state. Settlement replaces dispatch Weight by valid actual Weight but retains base/length costs, including after failed dispatch. `configs/resource_meter.rs::tests::admission_and_reclaim_preserve_frame_base_and_encoded_length` proves exact fit, one-unit RefTime/ProofSize rejection without state mutation and nonrefundable overhead. Remark/Router saturation fixtures use the same SDK envelope. No extra storage access or state field was added; the retained generated extension owner (`10,896,000 / 1,560`, one read and one write) still needs coordinated refreshed orchestration evidence.

`configs/mod.rs::RuntimeBlockWeights` gives Normal and Operational the common `MAXIMUM_BLOCK_WEIGHT` ceiling, `2,000,000,000,000 / 10,485,760`, also used by the Actors budget. The frozen economic meter partitions dispatch inside that ceiling; no class half-cap creates a smaller FRAME reference frame. `ActorOnIdleReserve` remains a configured admission parameter of `1,000,000,000,000 / 5,242,880`, not an additional or guaranteed Executive idle grant. The independent encoded block-length limit remains 5 MiB, with half available to Normal dispatch. Static fixed-work composition still exceeds the selected system quarter and does not implement settled-prefix reclamation.

`MessageQueueServiceWeight` is an independent `350,000,000,000 / 875,000` cap, preserving service capacity rather than multiplying its reservation when FRAME changes. Parachain inherent construction limits each message collection to the lesser of one sixth of maximum ProofSize and remaining proof; the upstream conditional XCM migration cutoff is one tenth of the common maximum. Fresh genesis mechanically excludes that migration branch, so it adds no active reservation; activating it requires renewed admission evidence. XCMP idle retains its generated leaf maximum. `configured_inbound_owners_require_complete_fixed_classification` reports these owners; context-provider/checker tests cover full and hashed count boundaries, including intentionally tiny PoV fixtures.

Normal and Operational both use `reserved = Some(Weight::zero())`, rejecting total FRAME overrun without disabling within-budget dispatch. Mandatory retains its special unbounded class treatment. `tests/actors_integration_tests.rs::configured_frame_budget_and_zero_operational_reserve_have_explicit_boundaries` checks the exact common ceiling, unchanged MQ cap and empty, RefTime-saturated, ProofSize-saturated and jointly saturated synthetic ledgers through public SDK admission; rejection preserves storage. The full-Executive W9 fixture separately proves ProofSize refusal with signed remarks and RefTime refusal with Router swaps while preserving Actor Q1 and the live Service prefix.

`tests/actors_integration_tests.rs::paired_base_refusal_uses_real_prefix_and_preserves_recovery_order` consumes base capacity through real bounded System Actor prefixes, without injected resource usage or budget overrides. Two-Step maximum-leg SplitTransfer isolates effect RefTime refusal; two-Step AddLiquidity isolates effect ProofSize refusal while minting real LP. A one-Step Transfer prefix isolates Control ProofSize refusal. Untouched suffix snapshots and the cursor prove no successor bypass. Empty external dispatch lets Drain recover the two effect frontiers in order while Q1 leaves every continuation at B+1; exhausted Control instead recovers through the next Prepass. Population derives from current effect bounds with a hard fixture ceiling, not a quota sweep. These are native hook/adapter witnesses with synthetic consensus context, not throughput or production proof certification.

Independent Control RefTime refusal is covered by package base/Drain tests and the runtime Drain witness through the same canonical paired-admission owner. The natural base Control fixture is ProofSize-limited; it neither claims a naturally RefTime-limited Control observation nor proves that such a frontier is globally unreachable.

`tests/actors_integration_tests.rs::paired_drain_refusal_preserves_runtime_actors_and_recovers_next_prepass` uses the real breaker to retain two funded User Actors through Prepass, then injects explicitly synthetic settled external/Control usage. Opening and Running each face independent one-unit RefTime/ProofSize shortages under current Contract resource envelopes. Drain preserves Actor, ring, hold, custody, fee-sink and event snapshots, charges only housekeeping plus inspection, leaves no reservation and finalizes normally. The next Prepass serves the same head before its successor; Drain cannot execute their B+1 tails. This tests runtime hook composition, not a reachable full-Executive workload or physical cost.

`paired_runtime_business_failure_commits_attempt_before_successor` instead blocks a real asset recipient through `pallet-assets`. In both base and Drain, the invoked transfer produces permanent `TokenError::Blocked`, leaves asset custody unchanged, charges its effect and User fee, applies authored AbortCycle and then executes the successor. Both attempts remain in the finalized effect ledger. Context is synthetic; task failure and fee/custody behavior use the runtime adapters without injected resource usage.

The full-Executive fixture supplies a synthetic relay PoV allowance of 10,485,760 bytes and computes pre-idle consumption from the common FRAME maximum. This native context is not evidence that a live relay admits that allowance, nor that charged ProofSize equals encoded PoV bytes. `full_executive_empty_workload_control_baseline_has_explicit_owners` retains explicit fixed/control attribution. Common-ceiling agreement does not certify production system-quarter fit, complete generated owners, physical proof containment or a new Actors-only baseline.

`tests/actors_design_comparison.rs` prepares a native isolated workload of 128 one-Step persistent System Transfer Actors over two blocks. A block-bound, thread-local `cfg(test)` host override feeds zero prefix/tail into the same `BlockResourceBudget::from_settled_prefix` constructor; production retains its conservative default. Genesis, funding and synthetic timestamp/validation context are outside Actor execution; no unrelated runtime hooks or background Actors run. The test asserts the exact 2e12/10 MiB, 1:2 envelope, ordinary Prepass → empty external phase → Drain, a deferred first-block suffix, next-block ordered recovery, Q1, recipient deltas and finalized accounting. A separate test checks wrong-block rejection and scope restoration, including unwind. These are functional preparation tests against current generated charges, not a throughput baseline, physical proof measurement or production-Wasm evidence.

`isolated_drain_refusal_preserves_fees_custody_and_next_block_recovery` reuses the ordinary runtime `assert_paired_drain_refusal` assertions with the isolated budget provider. Eight cases cross Control/effect, RefTime/ProofSize and Opening/Running. The real circuit breaker retains eligible work through Prepass; explicitly synthetic prior Control or external usage then leaves a one-unit deficit for Drain. Checks preserve Actor/ring/Contract/hold/custody/fee/event snapshots, charge only housekeeping and inspection, release every reservation and recover the same ordered head next block without duplicate service. The test-only host scope ends immediately after Prepass, so Drain and finalization must use the frozen budget. No ordinary user call or measured natural saturation is claimed; the original conservative-budget test still runs the same shared assertions.

`isolated_liquidity_borrowing_and_control_refusal_preserve_order` shares `assert_paired_base_refusal` with the conservative-budget suite. In the zero-system profile, real two-Step AddLiquidity Actors reach the base effect ProofSize frontier; Drain admits the refused head and successor, mints LP and settles exactly all invoked effects beyond the Actor base turn but within Shared Economic, with zero user dispatch. Drain receives only the block remainder after Prepass. The Transfer case instead reaches Control ProofSize and recovers next block. Snapshots preserve untouched suffixes and Q1 leaves tails at B+1. The isolated suite deliberately does not claim the conservative SplitTransfer effect-RefTime witness: that workload stops before its expected effect frontier under the larger envelope. RefTime boundary coverage remains the explicitly synthetic Drain matrix, not a claimed natural base saturation.

`isolated_business_failure_is_charged_and_does_not_block_successor` shares the real blocked-asset User witness across base and Drain with the same isolated host input. A permanent `TokenError::Blocked` retains the failed transfer's asset custody, charges the invoked effect and fee, applies AbortCycle and then admits the successful successor. The frozen budget and zero ordinary user-dispatch usage are checked through finalization. Unlike budget refusal, this is a committed attempt with authored failure semantics.

The selected isolated measurement domain is 128 Mutable System Actors, each with a Manual, zero-cooldown, one-Step persistent Native `Transfer(Fixed(1 ED))` to the same pre-endowed non-Actor recipient. Funding is RuntimePolicy; there are no predicates, tails, schedule windows, parked-balance plans or automatic close. Each source is prefunded with 1,000 UNIT outside execution. The Transfer preparation test pins the stored header, Step, resource context `(cursor=0, fragment=1, tails=0, predicates=0)` and generated effect selector, then checks exact source/recipient deltas, unchanged Fee Sink custody and one zero-fee Action receipt per invocation. Auxiliary liquidity, User-failure and synthetic-deficit cases are validation, not measurement population.

The following owner inventory separates selected execution from conservative admission. It is a source map, not a sum of physically measured costs or a declaration that present coefficients are sound. `W` denotes the runtime Actors `WeightInfo`; `RuntimeStepControlWeight` and `execute_mandatory_prepass` own the compositions rather than this table.

| Owner | Selected execution | Reserved or admission-only boundary |
| --- | --- | --- |
| Setup/readiness | Creation, prefunding and `manual_trigger` occur before measured hooks | Contract admission and useful-readiness source remain real, not charged to a later Step |
| Prepass entry | `W::scheduler_on_initialize_cutoff` | Frozen budget and mandatory headroom are admitted before Service |
| Dual-clock Deadline | Empty-frontier classification for Manual-only population | Complete `deadline_service_weight_upper`, including return, review and temporal maxima, remains reserved then settled |
| Balance dependency review | `dependency_scan_source_probe` and any actual scan work | Probe plus max of `process_dependency_scan_unit` and completion unit |
| Service inspection/refusal | Round begin/probe and `scheduler_actor_state_probe`; refused work retains inspection charges | Canonical head and paired Control/effect fit; no successor bypass |
| Successful inner control | Opening-head plan, `run_complete`, Queue placement, invocation receipt | `RuntimeStepControlWeight::base_maximum_control_weight` retains pooled plan/commit/placement, User completion and Suspended-head maxima even for this simple System domain |
| Outer Service | Advance/revalidation and its admitted/retired suffix maximum | `scheduler_complete_outer_weight_upper` also retains failure, Deadline, rollback and terminal comparators |
| Native effect | `W::task_transfer`, including synchronous runtime ingress/publication obligations | Stored effect maximum equals the invoked effect charge; recipient is existing and not an Actor |
| Fees/cleanup | No Pipeline fee or native fee debit; zero-fee Action receipt is retained; no authored close | `fee_collection` remains in the Step control maximum; Contract admission and terminal safety retain `close_actor`/cleanup ownership |
| Drain/finalization | `scheduler_on_idle_base`, actual remaining work and `block_resource_finalize` | Preserved Control headroom and ordinary Shared Economic borrowing only |

The existing `scheduler_service_system_transfer_native_fixed` extra benchmark is a singleton Native/Fixed diagnostic, not this population's complete measured block. Its genesis, recipient/funding geometry, resource setup and surrounding hook coverage differ. `task_transfer` instead exercises an Actor recipient with certified ingress. Neither a passing constructor nor this inventory certifies every retained maximum, resolves stale generated storage references, or permits copying temporary coefficients into production Weight. Input applicability and coordinated binding remain prerequisites for interpreting baseline capacity. The W0/W1 Transfer ledger binds the generated method's explicit DB read/write terms as well as total Weight: a compensating RefTime constant can preserve the total while making a claimed access count false. Generated DB charges remain distinct from physical unique-key observations.

Resource acceptance is layered rather than implemented as a second execution engine. `types::resource::tests` exhausts empty, partial, saturated, borrowing, component-fragmented, settlement, and corruption algebra under the protocol-fixed equal-thirds budget. `production_full_block_resource_harness_records_actor_and_user_contention` composes mandatory prepass, one Actor effect, one signed external dispatch, Drain, and finalized telemetry in one runtime block.

Mandatory Prepass first reserves and settles the complete independent-clock Deadline quantum, then admits the bounded parked-balance dependency scan. Finalization headroom is retained before the scan or Service competes; this is generated-envelope ownership, not a dedicated percentage. `mandatory_prepass_deadlines_survive_saturated_service_before_external_dispatch` proves both clocks publish B+1 Service before ExternalPhase even with a saturated ring. The same native fixture now equates normal Prepass/Drain hook actual with domain usage and distinguishes capacity deferral from an accounting fault.

Canonical Service now pre-admits discovery before topology/round mutation and Actor loading only after an eligible result; the pass owns concluding empty/closed probes too. Drain has no preliminary extra attempt. Native trie/read-boundary and telemetry tests prove that an uninspected frontier neither starves nor recovers, while paid progress and completed scan bounds retain healthy exits. These native charges still require final generated orchestration/settlement binding.

Late rollback retains valid effect charges or admitted maxima, conservatively charges branch Control and halts optional work without false recovery. Service also pre-admits the pure-cleanup bound for possibly closing Steps, including zero-Step nonce completion. Actual Close retains the Step maximum plus cleanup; nonterminal success releases the allowance. Native boundary and late-collector regressions cover both ownership seams, exact rollback and committed prefixes. Committed terminal Steps unlink Service and delete process authority rather than retain a `Retired` history row; residual custody stays untouched.

`RuntimeStepControlWeight` accepts only the current Step's `evaluation_units()`, bounded by `MaxPredicatesPerStep`; there is no independent Opening predicate axis. `runtime_step_control_predicate_domain_matches_current_step_bound` rejects doubled contexts and checks legal head/tail widths. Model-algebra and receipt tests do not establish physical benchmark containment. `retained_control_only_opening_profile_weights_fit_admission` checks a necessary numeric floor against the retained `_max` Manual Opening profiles over legal tail counts, not coverage of other Tasks or physical geometries. `predicated_stop_quote_retains_conservative_receipt_allowance` records current quote algebra; execution excludes every `StopCycle` from receipt emission.

`RuntimeStepControlWeight` already includes placement in its composed and measured inner models, while generic Service adds a conservative placement suffix. This overlap is not evidence that the suffix can be removed: zero-Step admission/Pipeline pricing still use the System/Manual profile, User/Trigger diagnostics are excluded by `scripts/benchmarks.sh`, and `close_actor` measures Deadline-resident cleanup rather than Service-resident destruction. The package's maximum-header Mutable User/System Manual, AddressEvent, AtTime, and Cadenced diagnostics add scoped retained/Service-close evidence, including future AtTime singleton/hold reclamation after paid-latch-preserving replacement and Cadenced Tick rearm/close. The host's unrelated Tick keys remain a measured baseline, not an assumed empty index. Maximum-header Immutable User Manual, AtTime and Cadenced terminal witnesses confirm authored nonce-one close after owner-close refusal and real paid readiness; temporal due sources are consumed before Opening, while cadence also reclaims its transient rearm. Other mutability cross-products remain open. These diagnostics are not production coefficients. Complete residual bookkeeping and retirement owners must precede any reduction.

Some runtime completion selectors associate `StepControlPlacement::None` with profiles that preserve an Actor and advance Service; those profiles do not certify terminal destruction. Generic terminal charging therefore does not reclaim from them. Scoped positive one-Step User/Manual Transfers with zero, maximum BalanceBelow/ObservationAbove, or mixed BalanceBelow/ObservationAbove/BlockNumberAbove current-Step predicates and maximum signed funding allowlist distinguish actual Task effect and Action fee from Pipeline control. A scoped User/Manual positive Transfer with nonce-one authored close confirms that a singleton Service-resident completion executes the Task and collects distinct fees before releasing process, Service membership, and owner hold without unwinding sovereign custody. A companion three-member Service fixture checks surviving peer links and cursor advance on the same effectful close. Neither profile contains deeper peer/indexed-detector terminal unions. A parallel System/Manual one-Step Transfer through ordinary Root creation moves real non-native custody, emits a zero-fee Action invocation receipt and collects neither User fee nor state hold; no Fee Sink credit occurs. A parallel System/Manual Burn checks exact non-native custody debit and `BurnExecuted` under the same zero-fee Action invocation and no-User-collection boundary. Observation-bearing variants use available maximum-encoded canonical feeds and fund certified post-Trigger Pipeline/Action capacity before measurement. Separate scoped positive one-Step User/Manual Burn and maximum-leg SplitTransfer extras retain that funding policy without predicates. Burn checks exact non-native debit; SplitTransfer checks distinct recipient credits, retained remainder and the maximum legal host fanout. Both check effect events and separate Pipeline/Action fee settlement. A further two-leg SplitTransfer failure diagnostic retains one paid B+1 User retry with no partial custody movement; fee-native source balance may fall only by the charged Pipeline/attempted Action fees, and a native repaired retry collects no second Pipeline fee. Two scoped User/Manual two-Step paths cover positive Transfer Opening into retained Running Service: native B+1 StopCycle consumes the paid Run without a second fee or custody movement, while a measured B+1 positive Burn on distinct custody pays only its new Action effect and completes without recollecting Pipeline. Neither binds a maximum legal header, deeper heterogeneous multi-Step progression or the complete Task/class/Trigger/predicate union. Profile qualification must cover Task preparation, Actor class, Trigger rearm, predicate sources and header width, not just phase/placement labels. Complete maximum/actual, discovery, placement, cleanup and failure-bookkeeping ownership remains a production binding requirement, separate from native accounting agreement and Wasm/replay rebinding.

A refused family grant settles only admitted discovery, work, and selection probes. An accepted grant settles one generated aggregate owner. Fixed scheduler base, coordinator, mandatory cleanup, and the shared envelope are subtracted once; the residual is the exact Actor execution floor. Exact coefficients and cohort traces belong to the generated binding and release measurements.

Funding authorization remains Contract policy, while every dynamic amount reads current sovereign Available balance at its execution attempt; no Actor-only funding accumulator, opening snapshot, or machine-fee ledger exists. Fresh-genesis Actors storage includes fragment-local resources, compact activation authority, sparse run state, bounded control topology, one transient block-resource state, and one latest finalized non-authoritative telemetry snapshot; the pre-`1.0` reference line defines no deployed-state migration.

System and User Actors share one persistent Service ring with one authoritative head and cyclic encounter order; no actor class reserves an execution share, Weight slice, or service right. Deadline work, dependency review, cleanup, and admission bookkeeping retain separate bounded owners. Within Drain, one round cutoff and per-process B+1 eligibility prevent reentry or bypass.

Creation, activation, fresh custody reattachment, and plan replacement reject any Actor Contract whose generated scheduler, complete bounded Pipeline Machine, and pure cleanup resources do not fit the guaranteed two-dimensional envelope. User sovereign service funding is not required until a ready Opening.

## Admission Certificate Authority

`configs/actor_config.rs::RuntimeAdmissionCertificateAuthority::current` derives authority from compiled configuration and generated Weight functions, not Actor population or block state. It rebuilds the Control and effect identities, hashes 20 configured-bound fields, takes the componentwise maximum of 12 lifecycle bindings and hashes the combined production identity. Those are four hash operations; constructing Weight values does not execute or charge the represented Tasks. This work belongs to the caller's CPU cost, even though it performs no pallet storage access.

Let `C = ceil(max(ActorMaxContractSteps - 1, 0)/4)`, `P = ActorMaxPredicatesPerStep`, and `L = ActorMaxSplitTransferLegs`. The current reference has `C=3`, `P=4`, `L=4`. The following is a source-level evaluation inventory, not generated execution Weight or an allocation/timing measurement.

| Identity input family | Generated Weight evaluations |
| --- | --- |
| User Opening and body reconstruction vectors | `2(C+1)` |
| Opening failure/retry/completion endpoint pairs | `6(C+1)` |
| Opening progress endpoint pairs | `2C` |
| Tail planning | `4` |
| Running and Suspended tail variants | `18(P+1)` |
| Suspended head endpoint pairs | `12` |
| Predicate vectors | `2P + max(P-1, 0)` |
| Fixed Control entries | `13` |
| Effect identity | `L+1` SplitTransfer entries plus `10` fixed entries |
| Lifecycle maximum | `12` entries |

At these bounds, Control identity evaluates 168 generated entries, effect identity 15, and lifecycle maximum 12. The implementation constructs 13 Control vectors and one effect vector; compiler optimization may alter physical allocation/work, so this does not establish RefTime. `RuntimeStepControlWeight::production_weight_identity_from_user_opening`, `TmctolTaskEffectWeight::production_weight_identity` and `RuntimeAdmissionCertificateAuthority::current` own this composition. No cache or per-block memoization is implied.

`admission_authority_is_storage_independent_and_covers_lifecycle_bindings` calls the provider without externalities, checks Control/effect composition, verifies lifecycle dominance with both maximum components attained, and checks unchanged storage roots and equal authority at different block numbers. This proves the current declared composition, not the soundness of its generated coefficients. Known Close underbounds remain; lifecycle compact-Weight width must be refreshed when production bindings change. The package Close inventory and `close_host_authority_calls_belong_to_detachment` attribute one invocation to detachment on successful public Active Close, not one per body reload; Contract commitment hashing remains separate.

## Generated Evidence and Artifacts

`scripts/actors-assurance.sh` owns freshness checks for Actors semantic, fee-envelope, ABI, ingress, weight, and metadata evidence. Production Wasm, metadata, descriptors, and generated client evidence remain owned by their build/export commands and checked directly by the `full` validation profile.

`template/runtime/src/weights/pallet_deos_actors.rs` owns complete generated methods and storage annotations. `scripts/actors-assurance.sh` verifies exact-tree Weight, production Wasm, metadata, ABI, and client-evidence identities while running named heavy profiles. This integration map records load-bearing composition, not scoped artifact hashes or benchmark-host timing.

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

## Pipeline Fee and Lifetime-Cleanup Partition

`RuntimeStepControlWeight::base_maximum_control_weight` composes current-Step planning, tail reconstruction, predicates, Run commit and Service/Deadline placement, then compares applicable direct inner profiles. It does not call `close_actor`. Receipt and Action-collection allowances are added separately. Hashing `close_actor` into `production_weight_identity_from_user_opening` binds provenance; it is not an arithmetic contribution to this Step-control price.

| Selected owner family | Retained measured operation | Destruction boundary |
| --- | --- | --- |
| Opening failure/retry/completion/progress and User header completion | `assert_reachable_opening` requires active Persistent authority and no auto-close/window | Completes or suspends a Cycle without destroying Actor state |
| Running completion/progress | `assert_reachable_running_inner` checks the retained successor | Run progress/completion, not Actor Close |
| Suspended head/tail retry/completion/progress | `assert_reachable_suspended_head_retry`, `assert_reachable_suspended_tail_success` and `assert_reachable_suspended_tail_skip` | Retains active or Run/Deadline authority; fixture funding teardown is outside measurement |
| `run_complete`, `task_stop_cycle`, zero-Step completion | Clears/completes a Run or stops a Cycle | Must remain in Pipeline pricing despite the word completion |
| Complete outer Service terminal/minimal-apoptosis profiles | Canonical Service with final state destruction | Physical outer admission, not an input to `derive_pipeline_machine_envelope` |

This is source/fixture ownership evidence, not certification of coefficient maxima or production Wasm. The Pipeline formula no longer adds a final-destruction surcharge. `zero_step_pipeline_quote_excludes_creation_backed_cleanup_without_reducing_admission` proves that the fee excludes Close while physical Contract admission retains the full close/outer maximum. No complete public-call Weight is subtracted from inner Cycle coefficients. The package architecture owns the generic conversion and admission boundaries.

`PipelineMachineEnvelope`, `PipelineFeeBreakdown` and `ActorPipelineFeeQuote` expose no cleanup-fee field. The runtime cost-vector example and browser `automation/cost.ts`/`cost-vectors.ts` agree that the Pipeline total equals the machine fee. Browser projections reject the retired cleanup field even when zero. Metadata, descriptors, the ABI manifest, protocol bounds and fee/cost vectors reflect the current source shape; their regeneration is not production Weight/Wasm certification.

## Control Plane and Read Surfaces

Canonical active projection joins `ActorSemanticStates`, certified Contract fragments, admission identity, `ActorProcesses`, exact Service/Deadline/Parked residence, and optional Run state at one finalized block. Dormant identity, detector topology, dirty-source progress, and bounded simulation remain canonical-chain truth.

The runtime simulation API executes the same package evaluator and finalizer used by scheduler service. Its bounded records carry canonical `StepOutcome` values, including concrete failure cause plus retry disposition, and its status is the shared `AttemptDisposition`; DEOS adds no adapter-side simulation model.

Version 6 `ActorEligibilityApi` reports `NotRegistered`, `Dormant`, or canonical Active classification at one finalized block. It preserves terminal reason, retry/block/timestamp-tick payloads, the four-family Trigger activation shape, latch, and process residence. Clients do not reimplement eligibility. The projection never promises service because Service order and available Weight decide admission.

`ActorCostApi` binds the DEOS `Balance` and returns one bounded canonical quote. User Creation Fee, current family-specific Trigger Weight/fee, upfront Pipeline Machine amount, current maximum Action-effect Weight/fee, and geometry-bound state hold remain separately named. Trigger provenance hashes the six generated occurrence owners; Pipeline provenance carries the admitted runtime/Weight identities; state-hold provenance exposes DEOS pricing of one ED per present component plus one `MICRO_UNIT` per retained SCALE byte. System Actors report zero Actors fees and explicit hold exemption without resource privilege.

`automation/cost.ts` projects `ActorCostApi` without recombining owners and maps committed `ActionFeeCharged` receipts by exact Actor/Cycle/Step coordinates; `adapters/blockchain/actor-cost.ts` invokes the typed API at one caller-supplied finalized hash and returns explicit unavailability instead of local fee inference. `automation/cost-vectors.ts` fail-closes malformed identities, totals, strategies, family/geometry coverage, System exemption, and dormant semantics before generated runtime vectors enter browser validation.

The browser Trigger editor offers exactly Manual, AddressEvent, AtTime, and Cadenced, routes observation comparisons to execution-time Step preconditions, and explains latched-fire coalescing plus Service backpressure. It exposes no Trigger-family bond quote because dispatch owns no such economic surface. The browser's authoring, artifact, matching-Wasm, simulation, observation, and governance-composition surfaces live under `web-client/src/lib/automation/` and `web-client/src/lib/observation/`. They bind metadata and runtime identity rather than recreating pallet semantics.

Unbounded history, archive search, forecasting records, governance preparation history, and longitudinal telemetry remain materialized-provider work under [`actors-control-plane.contract.en.md`](./actors-control-plane.contract.en.md) and [`read-model.contract.en.md`](./read-model.contract.en.md).

## Validation and Operations

Package tests own executable actor, scheduler, trigger, lifecycle, storage, and try-state behavior. Runtime tests own adapters, fees, ingress, genesis topology, Oracle/Router rollback, staking, XCM, generated-weight binding, block-budget partition, and full System/User composition.

Actors try-state reconciles semantic partitions, process status and residence, Service-ring links/count/cursor, Block/Tick Deadline heaps, C32 bucket/vacancy links, generation-bound handles, Park/Pending ownership, owner slots, System sovereign locators, and parked-balance dependency sources/cursors. Retained pre-fork scheduler declarations are rejected as active authority.

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

`scripts/actors-assurance.sh` owns package portability, external embedding, runtime integration, scheduler fairness, dense/sparse liveness, 10,000-Actor Service stress, and occupancy proof commands. Stress evidence covers cyclic Service fairness and exact Deadline draining. These are configured service-unit measurements, not user-facing time promises.

`full_executive_native_proof_owner_partition` in `template/runtime/src/tests/production_block_replay.rs` diagnoses empty, one-Transfer and saturated Manual workloads through a fresh native Executive replay. The shared `wasm_replay::key_proof_overlap` analyzer reconstructs each recorded access from proof nodes; the test partitions exclusive, cross-owner shared and unattributed raw bytes, reports encoded pre-state values and Service reads without a committed Step, and verifies unchanged authored post-state. Recording excludes authoring and diagnostic state enumeration. Shared nodes count once, so removing an absence read need not save proof bytes. This is whole-block native trie evidence, not per-Step marginal cost, full parachain PoV, a production-Wasm certificate or permission to discount generated Weight.

The same test module's W0/W1 resource ledger reconstructs the charged one-Step System Transfer from generated selection, classification, Opening, completion, placement, receipt and outer-suffix owners, checking exact totals and Prepass versus later refusal-classification charges under Actor-only and mixed demand. The phase-stop fixture reads the retained head's certified Step maximum, checks its generated component decomposition and composes the inspection/Service admission envelope, distinguishing that reserve from successful settlement and post-phase remaining capacity. Reconstruction runs outside block execution. Both replay modules are gated by `cfg(test)` in `runtime/src/tests/mod.rs`; pallet benchmark fixtures and the runtime Benchmark API require `runtime-benchmarks`, which the production build entrypoint does not enable. These diagnostics add no consensus storage, event, hook or production Weight coefficient.

A successful effectful Step that remains in Service advances its cursor once in `execute_effectful_step_on_service_with_deadline`. Current accounting prices Queue placement inside `RuntimeStepControlWeight::base_actual_control_weight` and adds the outer advance/retire maximum in `service_canonical_round_head_inner`. Standalone placement benchmarks do not include wrapper semantic/frontier revalidation or the full before/after validation of semantic replacement. This overlapping allowance is not a complete disjoint cost partition and cannot safely be removed from admission or settlement without bounding those other operations.

Mixed-clock evidence advances timestamp by many ticks at once and proves bounded Block/Tick arbitration, one execution and one canonical residence per Actor, bounded completion, and no cadence catch-up burst.

Benchmark-Wasm generation owns every selected branch coefficient and database term. Release measurements separately report Service fairness, mixed Task populations, useful effects, completion, latency, and both Weight dimensions for the exact generated tree. Reproducible profile results are evidence, not an execution SLA; congestion and fragmented component capacity may delay any individual Actor.

## Rollback and Operational Evidence

Task rollback and lifecycle rollback remain distinct corpus boundaries. A DEX adapter that fails after input transfer restores actor/pool task writes while `ContinueNextStep` permits the following transfer and cycle summary to commit.

Operational-recovery fixtures bind exact runtime version identity, breaker deferral/recovery, Continuation Deadline return, and bounded repair. Version drift yields `EvidenceMismatch` without changing factual Actor state. Breaker activation produces no partial cycle/close evidence; recovery admits the deferred close. A suspended retry wakes at cooldown without another signal, while one-actor permissionless repair remains available under the breaker and performs no hidden task work.

Full-corpus validation requires exclusive Service/Deadline/Parked residence, atomic pre-state restoration, and bounded Weight ownership. Quick Actors acceptance validates this contract; full acceptance executes the anchored runtime corpus.

Operational observation uses the canonical Service head/count/cursor, active limit, Block/Tick Deadline frontiers, actor-local process residence, starvation events, and sweep events. Weight and scan deferral remains silent and state-preserving. These surfaces expose pressure without promising an exact future execution block.

### Capacity Economics

The reference runtime binds `MaxActiveActors = MaxActorIdentities = MaxQueueLength = 10,000`, `MaxOwnerSlots = 255`, `MaxSweepBatch = 5`, and `ActorCreationFee = EXISTENTIAL_DEPOSIT = 0.001` fee-native units. Filling the identity cap with User actors therefore requires at least 10 fee-native units in nonrefundable creation fees and at least 40 distinct owner accounts. Active User liveness may additionally require balances above the protected minimum, but that balance remains actor custody and is not an anti-spam fee.

The same 10,000 ceiling bounds simultaneous active Actors and Service residence. One maximum permissionless sweep call examines five explicit identities, so 2,000 full batches cover the complete identity cap. A hypothetical one-batch-per-block sequence spans 12,000 seconds, or 3 hours 20 minutes, at the six-second target; this is a latency illustration, not guaranteed throughput, because dispatch Weight, competing block demand, eligibility, and submitted transaction count control realized progress.

The guaranteed saturated model assumes only one maximum attempt or cleanup fits the reserved Service envelope. With 10,000 eligible residents, no bypass, and one maximum attempt per block, a tail Actor is encountered within one 10,000-block ring traversal. Lighter plans may admit more work, but `MaxExecutionsPerBlock = 1,000` is only a count ceiling. Cadence denotes temporal eligibility; cyclic position and available Weight determine realized service.

Governance can lower the active-admission limit, operate the breaker, and repair bounded scheduler state, but it cannot confiscate or forcibly close a healthy owner-controlled User actor merely to recover capacity. Permissionless sweep closes only actors meeting the specified liveness/expiry conditions. Consequently, a fully funded live identity-cap fill has no bounded governance-only eviction latency in the current contract. DEOS provides explicit finite cost and bounded repair, not adaptive creation pricing or a guarantee against economically funded saturation.

The zombie-spam regression compares the complete actor-cap creation-cost floor with bounded permissionless sweep fees. Governance changes to creation fee, fee conversion, or sweep bounds must preserve the measured dominance relation rather than relying on a copied ratio.

Package scheduler, trigger, lifecycle, storage, and extrinsic internals remain authoritative in the package architecture. This integration map owns only the concrete bindings and policies layered over those mechanisms.
