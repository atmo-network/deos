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

The DEOS runtime maintains no Trigger-family bond, rent, or fee reserve. It binds `pallet-balances` as `StateHoldCurrency`, `RuntimeHoldReason::Actors(ActorState)` as the dedicated reason, one ED as each present component's fixed base, and one `MICRO_UNIT` per retained SCALE byte. `ActorStateHolds` separates identity, Contract head/body, detector topology, and Run capacity per User Actor. Dormant Actors release Active components, while System Actors are hold-exempt host capacity. Lifecycle changes reconcile owner hold deltas transactionally, and close never touches sovereign custody.

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

The guarantee is deliberately closed-world. External Oracle publishers, ordinary users, market counterparties, and uncertified balance movement are outside this graph. Oracle publication enters the separately bounded transition-ingress contract; User cycles remain permitted and paid; uncertified movement never fabricates AddressEvent activation.

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

For Actors Transfer/SplitTransfer, the invoking Task effect Weight covers recipient preflight, ledger movement and ingress notification; do not add a separate `AddressEvent` source Weight for the same internal movement. The recipient's useful-readiness Trigger fee and the invoker's Action/Pipeline fees remain separate economic owners. `certified_ingress_inventory_is_closed_and_typed` binds this runtime attribution; it does not prove physical sufficiency. EXP-0150's reissued four-User witness invokes the exact prepared SplitTransfer Task body, including the all-leg preflight, balance check, transfers, event and transaction; it exceeds the retained four-leg effect coefficient in both dimensions. The benchmark excludes surrounding Step control, and this selected effect witness is not a universal Task selector or a Weight/Wasm/ABI binding.

Certified ingress to an active `WindowExpired` recipient may synchronously finalize it inside the invoking Task effect. `four_leg_split_transfer_closes_expired_recipient_during_prepass_service` confirms this is reachable in the reference runtime even with four recipients and an admitted mandatory-Prepass Service turn: at least one recipient closes after its own ledger transfer. This selected test neither counts a universal maximum of inline closes nor prices terminal cleanup. The reference `TmctolTaskEffectWeight` chooses one maximum by `legs.len()` and settles that same reservation when invoked; recipient class, expiry, and native adapter side effects do not select a cheaper Task envelope. EXP-0151's reissued matched prepared-Task diagnostic with one expired maximum-header User and three healthy ones exceeds its all-healthy comparator in both RefTime including DB and ProofSize. That selected comparison is not a universal close maximum or production coefficient, and it does not prove that its exact maximum-header geometry occurs in the tested scheduler interleaving.

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

Fee-collector ledger movements are intentionally excluded from certified AddressEvent ingress. Actors charges generated Manual, matching AddressEvent, affected-Actor ObservationChange, fired-Actor ObservationCrossing, due AtTime, and due Cadenced occurrence owners, but that collector credit—like transaction-payment, governance-opening, and XCM fee credits—reaches Fee Sink without recursively creating Trigger readiness; its single cadence owns allocation. `ACTORS_ADDRESS_EVENT_PRODUCER_INVENTORY`, generated ingress evidence, paired-executive tests, and the embedding fixture continue to cover paths that actually signal an actor.

## Fee Composition

`RuntimeStepControlWeight` reserves one generated `fee_collection` allowance in Actor Control for each non-StopCycle Step, independently of Actor class. Opening, Running and Suspended actual control settle it only when a nonzero User Action fee must be collected; System and non-invoked attempts reclaim it. The existing bounded Pipeline machine envelope prepays this control work, while Action fees still derive solely from Task-effect Weight. This is separate from Pipeline admission collection. Base-selector Opening identity and component-wise reservation checks remain fail-closed.

`RuntimeStepControlWeight::user_opening_control_weight` supplies the same zero-tail and positive-tail User header profiles to maximum/actual selection and the production Control identity. The identity fingerprints every legal tail count; `control_weight_identity_binds_every_consumed_user_opening_profile` in `runtime/src/tests/actor_control_weight_identity.rs` rejects an omitted profile or either unbound Weight dimension. These are binding-integrity checks, not physical containment evidence. Complete generated ownership and final Weight/Wasm rebinding remain separate release gates.

Manual, AddressEvent, ObservationChange, ObservationCrossing fire, AtTime, and Cadenced are the implemented Trigger-fee families. Each family charges its generated occurrence owner only for a useful `pending_signal: false -> true` transition. Redundant latched activity performs no Actor-specific evaluation, fee, event, activation, or causal-history accumulation. Source-owned movement and authoritative Observation state may continue independently.

Opening re-arms stateful Trigger families from current authority; AtTime remains one-shot consumed; Cadenced resumes from the first deadline strictly after the current tick. Underfunding creates no readiness or apoptosis. Collector rejection instead rolls back exact source authority. A homogeneous Crossing batch whose collection cannot complete rolls back its aggregate attempt and executes its already-admitted scalar fallback without charging the publisher.

`PipelineMachineEnvelope` stores the Pipeline/cleanup quote from current host bindings in the certified Contract head. Idle readiness consumption charges that total before Opening; one-unit shortfall selects `CycleAdmissionInsufficient` cleanup without refunding the prior Trigger fee. Running or Suspended service performs no machine affordability or collection. Current-Step admission validates control and effect evidence independently; `StepFeeBreakdown` reserves and settles only the current Action effect. Non-invocation, false predicates, skipped resolution, `FundingUnavailable`, and `StopCycle` produce no Action collection.

Package-generated `actors-fee-envelope-vectors.json` constrains the browser's Action-only suffix reservation and protected-floor behavior. Runtime-generated `actors-cost-vectors.json` binds metadata and Actors Weight hashes to separate `ActorCostApi` owners across Manual `0/1/4/8/32` geometry, every Trigger family at one Step, dormant User absence, and explicit System exemption. `runtime/examples/actor_cost_vectors.rs`, `automation/cost-vectors.ts`, Actors assurance, and full regeneration own generation, fail-closed parsing, freshness, and drift evidence. Visible presentation remains open.

Runtime Pipeline projection binds the generated zero-Step owner, every certified Step-control branch across authored retry counts, and exact close cleanup rather than a broader lifecycle maximum. Runtime settlement charges Pipeline Machine control once per admitted Cycle and charges every invoked success or typed adapter failure from valid actual effect Weight.

Each committed Action-bearing attempt appends `ActionFeeCharged` with `(actor_id, cycle_nonce, step_index)`, actual effect Weight, and the exact User charge or System zero after semantic boundary events. Non-invocation and `StopCycle` emit no Action receipt. Pipeline, Action, collection, or evidence failure rolls back process residence, Step state, effects, close, fees, and events within the owning current-Step transaction.

`TmctolFeeCollector` transfers the complete charge into Fee Sink System Actor `1` through a ledger-only movement, matching transaction-payment and XCM-trader collection without fabricating AddressEvent readiness. One 120-tick cadence under the 500 ms consensus clock owns a stable 60-second allocation period. Its plan processes 10% of the current spendable Native buffer only when each configured split leg receives at least one ED, allocates the processed amount under the current phase policy, and retains the unprocessed buffer plus indivisible remainder. Runtime regressions prove collection-only custody, threshold skips, timestamp readiness, and no early execution.

The runtime-upgrade integrity gate re-derives the complete Fee Sink/native-security topology. It requires the exact Mutable persistent RuntimePolicy cadence contract and native 10% split; mode-shaped `50/50` staking/liquidity or `34/33/33` security/staking/liquidity legs with total share one; a retained liquidity System locator; one shared collector/actor custody account; distinct Fee Sink, staking ingress, security reward, liquidity, and LP-lock accounts; and ED anchors for every endpoint admitting arbitrarily small native flow.

Outside `runtime-benchmarks`, `RuntimeNativeSecurityModeProvider` fixes `TrustedSet`: successful security-reward funding is unavailable, while Fee Sink's active two-leg Native split sends one leg to the staking pool and one to the liquidity System Actor. When the native staking asset is registered, `TmctolAssetOps::transfer` converts the latter leg by slashing the received Native and minting the local staking asset before certified ingress; its conversion and ingress remain inside that SplitTransfer Task effect. The Fee Sink integration regression exercises this branch in TrustedSet. EXP-0152's reissued matched-Wasm diagnostic prepares an active liquidity Actor and invokes the complete prepared two-leg pool/bridge Task effect with certified ingress. Its measured source is not the Fee Sink, so the benchmark-only LP-backed security-reward route is **not invoked**: the pool/bridge effect branch matches the TrustedSet plan, while Fee Sink Step control and failure behavior remain unmeasured. Its selected value is below the stale projection, but neither this fixture nor an ordinary System-recipient comparator is a universal production bound.

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
| Crossing worker | 20% of maximum block Weight; admits one complete current worst-case unit |
| Deadline service | One pre-admitted Block-then-Tick quantum in mandatory Prepass, before materialization and Service; no repeated Drain quantum |
| ObservationChange fanout | 20% of maximum block Weight |
| `MaxCrossingMembersPerFeed` | 10,000 total memberships |
| `MaxUserCrossingMembersPerFeed` | 9,000 User memberships; remaining 1,000 positions are System-only |
| `MaxCrossingActorsPerBlock` | 128 candidate ceiling; the separate eight-transition component cap and shared Actor Control envelope bind first. The current retained 48-member cohort materializes `5, 8, 8, 8, 8, 8, 3` candidates before quiescence |
| `MaxContractSteps` | Configured maximum remains within `0..=255`; each User or System Contract admits `0..=12` Steps under the DEOS production binding, while independent hosts may select another bounded value |
| `MaxRetryAttempts` | 10 cursor-local unsuccessful attempts |
| `MaxConsecutiveFailures` | 10 |
| `MaxAutoCloseNonceHorizon` | 10,000 |

The fixed context owner uses measured DMP plus the smallest component-wise XCMP residual and outer scheduler bookkeeping whose component-wise sum dominates the complete maximum-context benchmark.

The current runtime composes `BlockResourceBudgetValue` from the maximum block Weight minus the fixed/context envelope, assigns Actor Control exactly one third of that schedulable remainder and assigns Shared Economic the complement. Shared Economic divides floor/remainder base turns between Actor effects and signed user dispatch. Either side may borrow unused capacity from the other, without consuming the other's guaranteed base under simultaneous saturation; both Weight dimensions fragment independently. The configured share now matches the resource-policy specification's **one-third** maximum. The live reference cap from the current fixed envelope is `303,433,283,909 RefTime / 676,150 ProofSize`; these are domain ceilings, not proof that a full fanout page or mandatory Prepass fits.

The block sequence is `context inherents -> Mandatory Actor Prepass -> signed user dispatch -> Actor Drain -> on_finalize`. The node supplies the payload-free versioned prepass inherent exactly once after Timestamp and parachain context; runtime phase guards reject absence, duplication, staleness, or ordering after signed dispatch. Prepass materializes bounded source work and reaches `ExternalPhase`; Drain serves one canonical Service round through `FreshDrain`, then generated finalization reaches `Finalizable`. `on_finalize` consumes only a matching current-block marker with zero outstanding reservations.

Prepass and Drain reserve maximum Actor Control before mutation and settle generated actual control afterward. Actor Actions reserve Shared Economic maxima from the Actor base turn and reclaim valid actual Weight transactionally. `BlockResourceMeterExtension` performs the equivalent maximum reservation and valid-actual reclaim for signed calls from the user base turn. Internal accounting and FRAME block Weight are reconciled fail-closed; uncertainty, overflow, stale state, and inconsistent post-dispatch evidence halt resource service rather than fabricate capacity.

`ActorResourceApi` exposes the configured two-dimensional budget, optional current authoritative block state, and latest bounded finalized non-authoritative snapshot as named SCALE projections. `ActorEligibilityApi` v6 replaces tuple-shaped fault/capacity output with `MaterializationFaults` and `CrossingCapacity`; clients do not reconstruct domain usage, phase, semantic faults, or capacity dimensions from events and tuple positions.

Generated `BlockResourceMeterExtension` execution (`10,896,000 / 1,560`, one read and one write) reserves each signed extrinsic's complete declared maximum from Shared Economic state during prepare, settles valid `PostDispatchInfo` actual Weight after dispatch, and rejects missing, stale, over-capacity, or inconsistent state with distinct transaction-invalidity evidence. Runtime block policy assigns 50% to dispatch and 50% guaranteed `on_idle` headroom. No dedicated Operational reserve exists while no concrete critical Operational call consumes one. `ActorOnIdleReserve` binds directly to `1,000,000,000,000 / 2,500,000`.

Resource acceptance is layered rather than implemented as a second execution engine. `types::resource::tests` exhausts empty, partial, saturated, borrowing, component-fragmented, settlement, and corruption algebra under the protocol-fixed equal-thirds budget. `production_full_block_resource_harness_records_actor_and_user_contention` composes mandatory prepass, one Actor effect, one signed external dispatch, Drain, and finalized telemetry in one runtime block.

Mandatory Prepass first reserves and settles the complete independent-clock Deadline quantum, then shares remaining materialization capacity between Crossing and broad fanout with work-conserving rotation. Finalization headroom is retained before either materialization or Service competes; this is generated-envelope ownership, not a dedicated percentage. `mandatory_prepass_deadlines_survive_saturated_service_before_external_dispatch` proves both clocks publish B+1 Service before ExternalPhase even with a saturated ring. The same native fixture now equates normal Prepass/Drain hook actual with domain usage and distinguishes capacity deferral from an accounting fault.

Under retained runtime Weight, every placed Crossing batch larger than a pair selects the same `crossing_placed_maximum_unit` (`649,694` ProofSize). Its minimal worker base, work probe, pair classification probe, maximum branch, and fault reserve total `676,224` ProofSize, **74 above the entire `676,150` Actor Control cap** even before Deadline or Service. Admission therefore downgrades or refuses such batches in this runtime; the live late-fee rollback selector is pair→scalar, not a larger-batch failure. `crossing_placed_batch_above_pair_uses_one_maximum_owner` and `current_control_cap_cannot_admit_a_placed_batch_larger_than_a_pair` bind the source selector and runtime arithmetic. Future generated weights or resource-policy changes must reassess this conclusion; generic Actors does not prohibit larger batches.

Canonical Service now pre-admits discovery before topology/round mutation and Actor loading only after an eligible result; the pass owns concluding empty/closed probes too. Drain has no preliminary extra attempt. Native trie/read-boundary and telemetry tests prove that an uninspected frontier neither starves nor recovers, while paid progress and completed scan bounds retain healthy exits. These native charges still require final generated orchestration/settlement binding.

Late rollback retains valid effect charges or admitted maxima, conservatively charges branch Control and halts optional work without false recovery. Service also pre-admits the pure-cleanup bound for possibly closing Steps, including zero-Step nonce completion. Actual Close retains the Step maximum plus cleanup; nonterminal success releases the allowance. Native boundary and late-collector regressions cover both ownership seams, exact rollback and committed prefixes. Committed terminal Steps unlink Service and delete process authority rather than retain a `Retired` history row; residual custody stays untouched.

`RuntimeStepControlWeight` accepts only the current Step's `evaluation_units()`, bounded by `MaxPredicatesPerStep`; there is no independent Opening predicate axis. `runtime_step_control_predicate_domain_matches_current_step_bound` rejects doubled contexts and checks legal head/tail widths. Model-algebra and receipt tests do not establish physical benchmark containment. `retained_control_only_opening_profile_weights_fit_admission` checks a necessary numeric floor against the retained `_max` Manual Opening profiles over legal tail counts, not coverage of other Tasks or physical geometries. `predicated_stop_quote_retains_conservative_receipt_allowance` records current quote algebra; execution excludes every `StopCycle` from receipt emission.

`RuntimeStepControlWeight` already includes placement in its composed and measured inner models, while generic Service adds a conservative placement suffix. This overlap is not evidence that the suffix can be removed: zero-Step admission/Pipeline pricing still use the System/Manual profile, User/Trigger diagnostics are excluded by `scripts/benchmarks.sh`, and `close_actor` measures Deadline-resident cleanup rather than Service-resident destruction. The package's maximum-header Mutable User/System Manual, AddressEvent, AtTime, Cadenced, Crossing Armed/Waiting and ObservationChange diagnostics add scoped retained/Service-close evidence, including future AtTime singleton/hold reclamation after paid-latch-preserving replacement and Cadenced Tick rearm/close. The host's unrelated Tick keys remain a measured baseline, not an assumed empty index. Maximum-header Immutable User Manual, AtTime and Cadenced terminal witnesses confirm authored nonce-one close after owner-close refusal and real paid readiness; temporal due sources are consumed before Opening, while cadence also reclaims its transient rearm. Other mutability cross-products remain open. Crossing uses a maximum-width feed ID and real paid observation, with its singleton detector/leaf/page/feed-count cleanup checked directly in benchmark Wasm. Armed singleton rearm has a real low-observation witness, and maximum-header Waiting terminal close swaps a real tail guard out of its second leaf page; a latched feed need not have an empty pending queue. Armed vacancy/new-page rearm and full-destination terminal close have separate maximum-header User/System witnesses; cursor, deeper terminal geometry and complete containment remain unqualified. ObservationChange has real publication, deferred fanout and singleton slot/page cleanup outside/inside the measured boundary respectively; a full shared single-page close now has a separate User/System survivor witness, and a separate one-vacancy free-slot page-fill witness is reachable for both classes; a linked two-page head-close witness is also reachable, but a separate maximum-header head-unlink successor-relink witness is reachable; active two-page ObservationChange fanout now has a distinct real multi-member Service-close witness, and pending-second-page retained/terminal Opening and a pending head-unlink witness are reachable after only the first real fanout page, with native post-Opening continuation checks for second-page delivery; other pending-feed, tail/interior unlink and failure cross-products remain open. These diagnostics are not production coefficients. Complete residual bookkeeping and retirement owners must precede any reduction.

Some runtime completion selectors associate `StepControlPlacement::None` with profiles that preserve an Actor and advance Service; those profiles do not certify terminal destruction. Generic terminal charging therefore does not reclaim from them. Scoped positive one-Step User/Manual Transfers with zero, maximum BalanceBelow/ObservationAbove, or mixed BalanceBelow/ObservationAbove/BlockNumberAbove current-Step predicates and maximum signed funding allowlist distinguish actual Task effect and Action fee from Pipeline control. A scoped User/Manual positive Transfer with nonce-one authored close confirms that a singleton Service-resident completion executes the Task and collects distinct fees before releasing process, Service membership, and owner hold without unwinding sovereign custody. A companion three-member Service fixture checks surviving peer links and cursor advance on the same effectful close. Neither profile contains deeper peer/indexed-detector terminal unions. A parallel System/Manual one-Step Transfer through ordinary Root creation moves real non-native custody, emits a zero-fee Action invocation receipt and collects neither User fee nor state hold; no Fee Sink credit occurs. A parallel System/Manual Burn checks exact non-native custody debit and `BurnExecuted` under the same zero-fee Action invocation and no-User-collection boundary. Observation-bearing variants use available maximum-encoded canonical feeds and fund certified post-Trigger Pipeline/Action capacity before measurement. Separate scoped positive one-Step User/Manual Burn and maximum-leg SplitTransfer extras retain that funding policy without predicates. Burn checks exact non-native debit; SplitTransfer checks distinct recipient credits, retained remainder and the maximum legal host fanout. Both check effect events and separate Pipeline/Action fee settlement. A further two-leg SplitTransfer failure diagnostic retains one paid B+1 User retry with no partial custody movement; fee-native source balance may fall only by the charged Pipeline/attempted Action fees, and a native repaired retry collects no second Pipeline fee. Two scoped User/Manual two-Step paths cover positive Transfer Opening into retained Running Service: native B+1 StopCycle consumes the paid Run without a second fee or custody movement, while a measured B+1 positive Burn on distinct custody pays only its new Action effect and completes without recollecting Pipeline. Neither binds a maximum legal header, deeper heterogeneous multi-Step progression or the complete Task/class/Trigger/predicate union. Profile qualification must cover Task preparation, Actor class, Trigger rearm, predicate sources and header width, not just phase/placement labels. Complete maximum/actual, discovery, placement, cleanup and failure-bookkeeping ownership remains a production binding requirement, separate from native accounting agreement and Wasm/replay rebinding.

A refused family grant settles only admitted discovery, work, and selection probes. An accepted grant settles one generated aggregate owner. Fixed scheduler base, coordinator, mandatory cleanup, and the shared envelope are subtracted once; the residual is the exact Actor execution floor. Exact coefficients and cohort traces belong to the generated binding and release measurements.

Funding authorization remains Contract policy, while every dynamic amount reads current sovereign Available balance at its execution attempt; no Actor-only funding accumulator, opening snapshot, or machine-fee ledger exists. Fresh-genesis Actors storage includes fragment-local resources, compact activation authority, sparse run state, bounded control topology, one transient block-resource state, and one latest finalized non-authoritative telemetry snapshot; the pre-`1.0` reference line defines no deployed-state migration.

System and User Actors share one persistent Service ring with one authoritative head and cyclic encounter order; no actor class reserves an execution share, Weight slice, or service right. Materialization, Deadline work, cleanup, and admission bookkeeping retain separate bounded owners. Within Drain, one round cutoff and per-process B+1 eligibility prevent reentry or bypass.

Creation, activation, fresh custody reattachment, and plan replacement reject any Actor Contract whose generated scheduler, complete bounded Pipeline Machine, and pure cleanup resources do not fit the guaranteed two-dimensional envelope. User sovereign service funding is not required until a ready Opening.

## Reactive Delivery Evidence

The runtime bounds ObservationChange fanout pages and canonical Deadline service independently. Compact activation authority avoids reading unreachable Contract tails, but ordinary fanout still loads each reached Actor's head. A full admitted page of maximum-Step User heads measures `20,610,426,000` RefTime base + 1,352 reads/326 writes and `961,802` ProofSize, exceeding the retained page owner in both dimensions; maximum signed-funding and balance-watch headers increase selected ProofSize to `1,010,826` (EXP-0143). Its canonical Trigger transitions are inside the one measured fanout unit; their useful-readiness fees remain independent economic collections, not another fanout Weight charge.

| Stable topology after publication | Subscriber-page turns |
| --- | ---: |
| One feed with 10,000 subscribers | 157 |
| One sparse subscriber at a historical high page id | 1 |
| Four dirty feeds with 10,000 subscribers each | 628 |
| One revision restart after quiescence | At most 314 |
| Persistently unavailable destination capacity | Unbounded |

An ordinary fanout service unit is one exact subscriber-page turn. Capacity pressure retains the exact page and subscriber position for retry; terminal cleanup uses a separate scalar turn. Finite rows require stable active topology, no newer selected-feed revision and eventual Service or Deadline capacity. The rows count turns, not promised blocks or Actor execution latency. Current generated family minimum `74,954,145,000 / 334,664` underprices the selected maximum-Step head page. A stricter ordinarily admitted 64-User page with maximum signed-funding and balance-watch header fields measures `1,010,826` ProofSize (EXP-0143). Its selected complete quantum with retained base, probe and fault is `1,020,148`, or 20,148 above the nominal 20% fanout contribution. That contribution is **not** a separate live family gate: the scheduler combines it with Crossing's 20% into a shared materialization budget and lends unused capacity after rotated minimum reservations. The runtime `integrity_test()` separately requires the fanout family minimum to fit its nominal contribution; an honest W2-containing rebinding fails this startup invariant by at least 20,148 ProofSize without coordinated correction (EXP-0144). The shared 40% envelope is also clipped by remaining Actor Control before worker admission. The retained Actor Control cap is only `689,673` ProofSize, below even the selected W1 page's `961,802` before mandatory deductions (EXP-0145); correcting only the 20% contribution or integrity assertion cannot admit an honestly priced page. Reconcile the outer domain and both rotations against Crossing, mandatory Prepass, Service and `on_idle` headroom before rebinding. No per-block fanout latency promise is current.

`template/runtime/src/weights/pallet_deos_actors.rs` owns generated Service, Deadline, ingress, Task, Predicate, lifecycle, and orchestration coefficients. The benchmark script owns the generation tool and sampling parameters. Those values price bounded topology only; they imply no throughput promise.

The retained reference `close_actor` ProofSize (`81,886`) is below a reachable deep Tick Root System close estimate (`83,814`, EXP-0140) at host capacity. A current-source/Wasm 50×20 cohort measured `85,241` for the signed User deep close with combined maximum funding allowlist, parked watches, `MaxContractSteps`, the full count of canonical Balance predicates and a maximum-encoded `SplitTransfer` Task in its false first Step; matched base `close_actor` was `81,886` (EXP-0140). The fixture retains a real hold, paid Opening, Run cursor and maximum tail chunks. An earlier parked-only branch measured `84,064` on its own source/Wasm; its first failed probe lacked complete attempt prefunding.

The false first Step never invokes the Task; this close-payload witness does not price SplitTransfer execution or establish a universal terminal maximum. A separate ordinary-admission maximum-Observation variant measured `85,702` ProofSize against matched base `81,886` on the current benchmark Wasm; it retained Running with cursor 1. The native mock instead left that Actor Idle with `Service(Pending)` and no Run, while retaining the Tick pointer and surviving signed deep close (EXP-0140). These are host-specific states, not additive profiles. A separate signed User close of an ordinarily Parked maximum balance-watch plan with a Block review and independent Cadenced Tick Trigger measures `993,159,000` RefTime base + 90 reads/95 writes, exceeding retained `close_actor` (`969,972,000` base + 69 reads/60 writes) even before nonnegative DB increments; its `41,658` ProofSize fits the retained `81,886` (EXP-0139). That initial comparison has local clock indices. A later ordinary signed Parked close retains the same Block review while repairing a near-capacity Tick index: matched 50×20 `2,343,143,000` RefTime base + 120 reads/117 writes and `97,202` ProofSize versus base `882,180,000` + 69/60 and `81,886` (EXP-0139). These selected branches establish underbounds, not a universal terminal maximum or deep Block-review bound. An independently admitted signed Suspended User close with a Block retry Deadline and deep Tick Trigger measures `83,820` ProofSize against same-Wasm base `81,886` (EXP-0142). Its complete RocksDB-weighted RefTime narrowly fits that base; the RefTime bases alone would give the wrong result. The Block retry index is local in that Suspended witness. A separate signed maximum-watch Parked close repairs a populated deep Block review index while retaining a local Tick Trigger: matched 50×20 `2,560,981,000` RefTime base + 121 reads/118 writes and `94,084` ProofSize versus base `880,294,000` + 69/60 and `81,886` (EXP-0139). On the same Wasm the deep-Tick Parked witness measures `2,348,171,000` base + 120/117 and `97,202` ProofSize: neither branch dominates both dimensions. A separate ordinary signed one-entry-tail deep Block review close measures `94,084` ProofSize and 121 reads/118 writes, matching the deep Block W3 storage/proof shape while retaining one-entry last-page removal (EXP-0139); RefTime order is not established against host noise. One ordinary signed maximum-watch Parked close repairs both deep clock indices in the same finalizer, each with a one-entry last page: matched 50×20 `2,962,366,000` RefTime base + 145 reads/136 writes and `139,561` ProofSize, above retained `close_actor` in both dimensions and above either isolated deep-clock witness on the same Wasm (EXP-0139). Do not sum complete closes. A separately admitted signed Suspended User close with a Block retry Deadline and independent Tick Trigger, both in deep indices, measures matched 50×20 `2,927,514,000` RefTime base + 90 reads/70 writes and `128,352` ProofSize, above retained `close_actor` in both dimensions; matched Parked W5 dominates this selected profile component-wise (EXP-0142). Other Block retry tail/page shapes, concurrent population splits and maximum indexed detector cleanup remain unqualified. The same generated owner binds public close dispatch, internal cleanup reserve, and the User Pipeline cleanup-fee upper; none is physically contained until coordinated terminal-owner rebinding. Contained local-page witnesses cannot bound deep-index repair.

## Generated Evidence and Artifacts

`scripts/actors-assurance.sh` owns freshness checks for Actors semantic, fee-envelope, ABI, observation, ingress, weight, and metadata evidence. Production Wasm, metadata, descriptors, and generated client evidence remain owned by their build/export commands and checked directly by the `full` validation profile.

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

## Control Plane and Read Surfaces

Canonical active projection joins `ActorSemanticStates`, certified Contract fragments, admission identity, `ActorProcesses`, exact Service/Deadline/Parked residence, and optional Run state at one finalized block. Dormant identity, detector topology, dirty-source progress, and bounded simulation remain canonical-chain truth.

The runtime simulation API executes the same package evaluator and finalizer used by scheduler service. Its bounded records carry canonical `StepOutcome` values, including concrete failure cause plus retry disposition, and its status is the shared `AttemptDisposition`; DEOS adds no adapter-side simulation model.

Version 6 `ActorEligibilityApi` reports `NotRegistered`, `Dormant`, or canonical Active classification at one finalized block. It preserves terminal reason, retry/block/timestamp-tick payloads, Crossing phase and revisions, latch, and process residence. `materialization_faults` projects only current Crossing and broad-fanout faults; `crossing_capacity` returns the 9,000 User / 10,000 total per-feed policy and exact counts. Clients do not reimplement eligibility. The projection never promises service because Service order and available Weight decide admission.

`ActorCostApi` binds the DEOS `Balance` and returns one bounded canonical quote. User Creation Fee, current family-specific Trigger Weight/fee, upfront Pipeline Machine/cleanup amounts, current maximum Action-effect Weight/fee, and geometry-bound state hold remain separately named. Trigger provenance hashes the six generated occurrence owners; Pipeline provenance carries the admitted runtime/Weight identities; state-hold provenance exposes DEOS pricing of one ED per present component plus one `MICRO_UNIT` per retained SCALE byte. System Actors report zero Actors fees and explicit hold exemption without resource privilege.

`automation/cost.ts` projects `ActorCostApi` without recombining owners and maps committed `ActionFeeCharged` receipts by exact Actor/Cycle/Step coordinates; `adapters/blockchain/actor-cost.ts` invokes the typed API at one caller-supplied finalized hash and returns explicit unavailability instead of local fee inference. `automation/cost-vectors.ts` fail-closes malformed identities, totals, strategies, family/geometry coverage, System exemption, and dormant semantics before generated runtime vectors enter browser validation.

The browser Trigger editor rejects invalid hysteresis, discloses no-retrofire/rearm semantics, distinguishes typed User-capacity versus total-capacity atomic failures, directs authors to same-block `crossing_capacity`, warns that broad-fanout service scales with subscribed pages, and explains latched-fire coalescing plus Service backpressure. It exposes no Trigger-family bond quote because dispatch owns no such economic surface. The browser's authoring, artifact, matching-Wasm, simulation, observation, and governance-composition surfaces live under `web-client/src/lib/automation/` and `web-client/src/lib/observation/`. They bind metadata and runtime identity rather than recreating pallet semantics.

Unbounded history, archive search, forecasting records, governance preparation history, and longitudinal telemetry remain materialized-provider work under [`actors-control-plane.contract.en.md`](./actors-control-plane.contract.en.md) and [`read-model.contract.en.md`](./read-model.contract.en.md).

## Validation and Operations

Package tests own executable actor, scheduler, trigger, lifecycle, storage, and try-state behavior. Runtime tests own adapters, fees, ingress, genesis topology, Oracle/Router rollback, staking, XCM, generated-weight binding, block-budget partition, and full System/User composition.

Actors try-state reconciles semantic partitions, process status and residence, Service-ring links/count/cursor, Block/Tick Deadline heaps, C32 bucket/vacancy links, generation-bound handles, Park/Pending ownership, owner slots, System sovereign locators, observation subscriptions, dirty-source links/cursors/counts, and revision baselines. Retained pre-fork scheduler declarations are rejected as active authority.

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

`scripts/actors-assurance.sh` owns package portability, external embedding, runtime integration, scheduler fairness, dense/sparse liveness, 10,000-Actor Service stress, Crossing relevance, and occupancy proof commands. Stress evidence covers cyclic Service fairness, exact Deadline draining, zero-match Crossing without Actor publication, bounded crossed cohorts, sparse-leaf cursor preservation, and large same-threshold membership. These are configured service-unit measurements, not user-facing time promises.

Mixed-clock evidence advances timestamp by many ticks at once and proves bounded Block/Tick arbitration, one execution and one canonical residence per Actor, bounded completion, and no cadence catch-up burst.

Benchmark-Wasm generation owns every selected branch coefficient and database term. Release measurements separately report Service fairness, mixed Task populations, useful effects, completion, latency, and both Weight dimensions for the exact generated tree. Reproducible profile results are evidence, not an execution SLA; congestion and fragmented component capacity may delay any individual Actor.

## Reactive Capacity Ledger

The runtime bounds Crossing and broad ObservationChange materialization under one shared envelope after mandatory Deadline work and finalization headroom, permits 9,000 User and 10,000 total Crossing memberships per feed, and bounds Service by the 10,000 active-Actor ceiling. Crossing billing selects one mutually exclusive generated execution owner after common, branch, and admission-selection work; independently measured whole branches are never summed.

| Crossing path | Authority | Acceptance condition |
| --- | --- | --- |
| No match / sparse seek | Transition and radix owners | No Actor publication |
| Post-installation skip | Scalar or homogeneous cohort owner | Installation revision excludes retrofire |
| Rearm | Scalar or homogeneous cohort owner | Current value restores the armed phase |
| Coalesced fire | Disabled/coalesced owner | Existing latch prevents duplicate readiness |
| Placed fire | Scalar, pair, tail, or non-tail cohort owner | Complete fee and Service/Deadline publication fit |
| Terminal fire | Terminal owner | Complete synchronous cleanup fits |
| Structural fault | Fault owner after transactional rollback | Exact bounded fault identity can be stored |

The ledger separates zero-match setup, sparse occupied-leaf search, source-page traversal, rearm, coalesced fire, placed fire, terminal cleanup, and structural fault. Generated Weight and exact-Wasm replay bind the selected values for the final source tree; future geometry changes require complete evidence refresh.

Declared funded Crossing-to-Transfer witnesses require one materialization and one committed Transfer per Actor with zero censoring or faults. Comparative latency, Control Weight, and occupancy observations remain Experiment Record evidence tied to their measured binding; they are not herd-scale, steady-state, wall-clock, fairness, or universal throughput claims.

Every Drain figure assumes one homogeneous eligible branch, solvent Actors, available Service/Deadline capacity, no materialization fault, production block reserve, and no competing family consumption beyond its protected minimum. Producer rate, Service occupancy, mixed thresholds, paused or already-latched Actors, insolvency, branch changes, and rotated lending order can change completion. These are bounded capacity facts, not an SLA.

The runtime Pool Index extension fault anchor admits a pool-creation call, injects failure only at post-dispatch LP/Oracle indexing, rejects the block candidate, and proves exact storage-root restoration across the pool, LP asset and reverse index, Oracle feeds, balances, events, and signer nonce. Package Router faults separately prove exact-root rollback when the second market leg or second directional Oracle publication fails after earlier pool, fee, and publication work.

The runtime cross-pallet hook-rejection anchor fills Actors dirty capacity, attempts direct Oracle publication, and proves Oracle observation/revision, Actors feed/list state, and runtime events equal the captured pre-state. After capacity recovery, one producer retry commits Oracle revision `1` and Actors latest revision `1`; no replay state or Router publication path participates.

`ACTORS_OBSERVATION_PUBLISHER_INVENTORY` closes the reference-runtime publisher set to `DEOS Oracle::OnObservationChanged`. The Oracle hook reaches Actors through `ObservationTransitionIngress` with the exact revision and previous/current scalar values; no second runtime publisher owns transition progression. Generated observation evidence scans the complete runtime-config Rust tree, requires exactly one Oracle-owned typed ingress call, and rejects direct broad-fanout bypasses.

`EVENT_COMPLETE_TRANSITION_OWNERS` closes every Oracle writer that can invalidate current dependency state: feed registration, pause/resume lifecycle mutation, deactivation, and changed or equal-value publication. Every row binds the collision-free typed key `EventCompleteDependencySource::OracleFeed(feed)`, its transaction boundary, and the exact post-write/pre-event publication point. Source-backed runtime evidence fails when a new `Feeds` or `Observations` writer is unclassified. The production `OnFeedStateChanged` adapter now resolves or reuses the exact feed/source bijection and advances the event-complete dependency revision in O(1) without subscriber traversal. Oracle benchmarks separately measure allocation on registration and reuse/coalescing on existing-feed lifecycle and publication paths; generated runtime weights charge the composed storage effects. Age expiry remains timed.

`TmctolObservationProvider::current` supplies Crossing rearm with the retained canonical value/revision even when its feed is Paused or its value is old; age-sensitive predicate observations use the separate `observe` interface. Deactivated feeds instead return `Unavailable`. Oracle deactivation is terminal, retains the feed identity and last observation, and has no ordinary resume path. A previously admitted Crossing therefore cannot become `Uninitialized` through ordinary Oracle calls, but can lose availability after deactivation.

Deactivating a feed used by a latched Crossing can prevent Opening at the Service head and stop later Actors despite spare Weight. Opening retains paid readiness and commits no Pipeline fee or Cycle. Authorized Mutable owner close, independently due terminal semantics, or an explicit runtime upgrade can resolve the obstruction within existing authority; observation loss grants no new Immutable close permission. `scheduler_paged_zero_step_user_crossing_unavailable` owns the refusal and Mutable-close recovery evidence.

Task rollback and lifecycle rollback remain distinct corpus boundaries. A DEX adapter that fails after input transfer restores actor/pool task writes while `ContinueNextStep` permits the following transfer and cycle summary to commit. Corrupt dirty-list linkage makes deactivation fail closed and restores actor, subscription, dirty-feed, list, and event pre-state; explicit linkage repair permits a fresh deactivation attempt to finish cleanup.

Operational-recovery fixtures bind exact runtime version identity, breaker deferral/recovery, Continuation Deadline return, and bounded repair. Version drift yields `EvidenceMismatch` without changing factual reactive state. Breaker activation produces no partial cycle/close evidence; recovery admits the deferred close. A suspended retry wakes at cooldown without another signal, while one-actor permissionless repair remains available under the breaker and performs no hidden task work.

Full-corpus validation requires revision linkage, dirty-feed uniqueness, subscriber-page reachability, exclusive Service/Deadline/Parked residence, atomic pre-state restoration, and bounded Weight ownership. Quick Actors acceptance validates this contract; full acceptance executes the anchored runtime corpus.

Operational observation uses the canonical Service head/count/cursor, active limit, Block/Tick Deadline frontiers, actor-local process residence, starvation events, current materialization faults, and sweep events. Weight and scan deferral remains silent and state-preserving. These surfaces expose pressure without promising an exact future execution block.

### Capacity Economics

The reference runtime binds `MaxActiveActors = MaxActorIdentities = MaxQueueLength = 10,000`, `MaxOwnerSlots = 255`, `MaxSweepBatch = 5`, and `ActorCreationFee = EXISTENTIAL_DEPOSIT = 0.001` fee-native units. Filling the identity cap with User actors therefore requires at least 10 fee-native units in nonrefundable creation fees and at least 40 distinct owner accounts. Active User liveness may additionally require balances above the protected minimum, but that balance remains actor custody and is not an anti-spam fee.

The same 10,000 ceiling bounds simultaneous active Actors and Service residence. One maximum permissionless sweep call examines five explicit identities, so 2,000 full batches cover the complete identity cap. A hypothetical one-batch-per-block sequence spans 12,000 seconds, or 3 hours 20 minutes, at the six-second target; this is a latency illustration, not guaranteed throughput, because dispatch Weight, competing block demand, eligibility, and submitted transaction count control realized progress.

The guaranteed saturated model assumes only one maximum attempt or cleanup fits the reserved Service envelope. With 10,000 eligible residents, no bypass, and one maximum attempt per block, a tail Actor is encountered within one 10,000-block ring traversal. Lighter plans may admit more work, but `MaxExecutionsPerBlock = 1,000` is only a count ceiling. Cadence denotes temporal eligibility; cyclic position and available Weight determine realized service.

Governance can lower the active-admission limit, operate the breaker, and repair bounded scheduler state, but it cannot confiscate or forcibly close a healthy owner-controlled User actor merely to recover capacity. Permissionless sweep closes only actors meeting the specified liveness/expiry conditions. Consequently, a fully funded live identity-cap fill has no bounded governance-only eviction latency in the current contract. DEOS provides explicit finite cost and bounded repair, not adaptive creation pricing or a guarantee against economically funded saturation.

The zombie-spam regression compares the complete actor-cap creation-cost floor with bounded permissionless sweep fees. Governance changes to creation fee, fee conversion, or sweep bounds must preserve the measured dominance relation rather than relying on a copied ratio.

Package scheduler, trigger, lifecycle, storage, and extrinsic internals remain authoritative in the package architecture. This integration map owns only the concrete bindings and policies layered over those mechanisms.
