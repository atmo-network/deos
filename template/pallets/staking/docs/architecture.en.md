# DEOS Staking: Share-Vault and Session-Native LP Security Architecture

> Package navigation: [`specification.en.md`](./specification.en.md) · [`embedding.md`](./embedding.md) · [`../README.md`](../README.md)
>
> **On-Chain Namespace**
>
> - Pallet: `pallet-staking`
> - PalletId: `staking0`
> - Deterministic pool account: `pool_account(asset_id) = PalletId.into_sub_account_truncating(asset_id)`
> - Current runtime native staking asset id: `0` (`$NTVE`)
> - Native/local staking receipts use the `0x5...` namespace (`stNTVE`, `stXXX`)
> - Foreign staking receipts use the `0x6...` namespace
> - Native collator nomination is backed by locked canonical `NTVE/stNTVE` LP, not by liquid `stNTVE`

## Executive Summary

`pallet-staking` is the DEOS reference runtime's multi-asset share-vault implementation plus the native `$NTVE` LP-backed security surface used by the current launch line.

The common staking kernel remains simple:

> Backing inflow belongs to the pool and raises receipt value without iterating all holders

The current native implementation adds a separate security layer on top of that kernel:

- `stake(NativeStakingAssetId, amount)` mints liquid, yield-bearing `stNTVE`
- The canonical `NTVE/stNTVE` AMM is the liquidity surface for native security
- Collator nomination requires explicit custody of `NTVE/stNTVE` LP through `lock_native_lp_for_collator`
- Native `NativeVotePower` comes from explicit custody sources, not transferable receipt movement
- Ready LP-backed session planning atomically freezes eligible operators, account LP value, governance coefficients, reward weights, and the total denominator
- Generic share-vault yield remains receipt appreciation through backing inflow; the legacy generic reward engine is absent

This document describes shipped implementation truth. The broader normative target lives in `template/pallets/staking/docs/specification.en.md`.

## Architecture Overview

### Design Principles

1. `Pool-first accounting`
   Share price moves through pool backing, not per-holder reward writes.
2. `Liquid native receipt`
   `$NTVE` staking mints yield-bearing `stNTVE` without binding the account to a collator.
3. `Liquidity-backed nomination`
   Native collator backing is explicit locked `NTVE/stNTVE` LP custody.
4. `Explicit security state`
   Transferable `stNTVE` movement does not update collator backing or native reward eligibility.
5. `Reward contraction`
   The legacy generic reward engine is absent; retained session snapshots and pots carry the bounded native funding and settlement contract.
6. `Runtime-as-Config`
   Receipt lifecycle, operator validation, LP validation, governance coefficients, and read-model valuation are runtime-provided.

### System Architecture

```mermaid
graph TD
    User[User] -->|stake(NativeStakingAssetId, amount)| NativePool[NTVE share-vault]
    NativePool -->|mint liquid receipt| StNTVE[stNTVE]
    User -->|add liquidity| Amm[Zero-fee NTVE/stNTVE AMM]
    Actors[System Actors LP provisioning actor] -->|DonateLiquidity NTVE/stNTVE| Amm
    Amm -->|mint LP to user| Lp[NTVE/stNTVE LP]
    User -->|lock_native_lp_for_collator| CollatorLock[Collator LP lock custody]
    CollatorLock -->|conservative native value| Session[Collator session ranking]
    CollatorLock -->|frozen eligible value| SecuritySnapshot[Atomic security snapshot]
    GovLocks[Governance LP/asset locks] -->|NativeVotePower| Governance[Governance]
    Governance -->|bounded coefficient read at session boundary| SecuritySnapshot
```

## Implementation Modules

`src/lib.rs` remains the single package facade and sole owner of FRAME storage declarations, dispatchable indices, events, errors, view signatures, and public SCALE models. FRAME macro ownership therefore stays coherent while implementation details are grouped behind the facade.

| Module | Implementation ownership | Facade boundary |
| --- | --- | --- |
| `src/pool.rs` | Share-vault accounting, receipt lifecycle, deterministic accounts, pool sync, and bounded arithmetic | Dispatchables and public query names remain in `src/lib.rs` |
| `src/custody.rs` | Governance-lock admission and retained account/operator/global custody aggregates | FRAME storage and custody dispatchables remain in `src/lib.rs` |
| `src/security.rs` | Mode-derived operation admission, readiness, epoch retention, reward-weight arithmetic, and certified funding settlement | Public mode/epoch models and calls remain facade-owned |
| `src/views.rs` | Bounded native-security, pool, collator, and governance-custody projection construction | `#[pallet::view_functions]` signatures remain in `src/lib.rs` |
| `src/invariants.rs` | Try-runtime-only reward custody/liability reconciliation helpers | The facade hook remains the one `try_state` entrypoint |

Public SCALE-bearing models remain declared in `src/lib.rs`, so internal module names cannot enter metadata type paths and no `scale_info` path rewrite is needed. The metadata equality gate fails closed if later extraction moves a model and requires explicit `scale_info` path control before acceptance.

## Account and Asset Topology

### Native security mode

The runtime owns one immutable code-level `NativeSecurityMode::{TrustedSet, LpBackedSelection}` decision, exposed with readiness and epoch state through `native_security_view()`. One internal operation-availability classifier derives nomination, redelegation, candidate selection, certified funding, compound, and Trusted contraction semantics; liquid claims and custody exits are mode-independent.

Calls, readiness, session management, and runtime adapters consume the classifier directly. Clients derive mode-dependent action availability from the canonical mode instead of a duplicated seven-boolean capability view.

`native_security_view()` fail-closed classifies mode, pool, receipt, LP, reserve, issuance, valuation, bounded-index, positively backed candidate-operator, and duplicate-candidate state. It also projects current and uniquely Planned epoch identity plus whether any Open/Finalized pot or nonzero liability leaves settlement obligations. The pot scan reads at most the hard retention bound plus one and rejects excess retention or multiple Planned epochs. Candidate deposits cannot satisfy operator readiness; none of these surfaces is mutable policy storage.

The reward architecture is mode-aware. `TrustedSet` uses permissioned collators, collects transaction, Actor-execution, governance-opening, and XCM-execution fees in Fee Sink, and divides available native balance 50/50 between staking ingress and liquidity provisioning. DEOS Router trading fees remain on the Burn Actor path.

The LP-donation half flows through Fee Sink → Actors #14, with a native-balance bridge into the local native-staking asset before donation execution. After that donation hook, the staking-yield half burns native balance held by the staking pool account and mints the local native-staking asset into pool truth.

LP-backed mode divides Fee Sink flow into one 34% security-reward leg and two 33% staking-ingress/liquidity-provisioning legs so integer shares sum exactly. The security leg is accepted only through the source-checked certified funding boundary; claims and expiry consume the retained bounded settlement contract.

`LpBackedSelection` is a runtime-upgrade boundary. The current runtime binds `TrustedSet`; session candidate inclusion, candidate-only operator admission, new LP nomination, and redelegation read the same mode owner. `NativeSecurityReadiness` is a pure current-state projection and contains no attempted-transition result.

Under LP-backed mode, each planning attempt overwrites one `LastNativeSecurityBoundaryDiagnostic { planned_epoch, outcome }`. `NativeSecurityBoundaryOutcome` distinguishes `NotReady(readiness)`, `SnapshotOpened`, and `SnapshotOpenFailed`; a non-ready attempt returns before cleanup, snapshot, or ranking, while failed opening preserves active state and returns `None`.

At every session start, the runtime invokes mode-independent retention for the oldest overdue epoch and charges the measured worst-case bounded-scan Weight. During ordinary progression this is exactly the epoch crossing `SecurityRewardClaimHorizon`. A settlement failure cancels that boundary's zero-credit plan and prevents newer planning until recovery.

After successful retention, LP-backed mode promotes the current plan to Open and finalizes the prior Open pot. Trusted mode instead finalizes any retained Open pot without changing liability, removes the current unactivated zero-credit plan, clears active identity, and leaves the resulting Finalized claim/expiry rights live.

There is no diagnostic history or block-hook writer. Liquid claims, expiry recovery, unlock requests, and matured withdrawals remain available so mode contraction cannot trap obligations or custody.

The outer collection rule sends 100% of transaction, Actors, governance-opening, and XCM-execution fees into Fee Sink without an immediate author split. Block issuance remains unconfigured and must receive a separate source/amount decision before entering Fee Sink or the security budget.

### Per-asset sovereign pool

Each registered staking asset has one deterministic `pool_account(asset_id)` for share-vault backing, stake deposits, and direct backing inflow. The runtime routes phase-one liquidity provisioning directly through Actors rather than maintaining generic per-asset reward channels. Native LP security uses separate deterministic `native_security_reward_account()` custody only with retained pots and exact liability accounting.

### Receipt asset lifecycle

Receipt ids are runtime-resolved by `StakedAssetIdResolver`:

- `$NTVE` and local assets derive into the local staked namespace (`TYPE_STAKED = 0x5000_0000`)
- Foreign assets derive into `TYPE_STAKED_FOREIGN = 0x6000_0000`
- Receipt classes are local `pallet-assets` assets

`register_staking_asset(asset_id)` requires a resolvable receipt id and atomically creates and indexes the receipt class through the runtime lifecycle hook. For `$NTVE`, metadata is currently `Staked Native Token` / `stNTVE` / `12` decimals.

### Canonical native LP token

The runtime validates the native staking LP token through `RuntimeNativeStakingLpAssetValidator`:

1. Resolve `$NTVE -> stNTVE`
2. Resolve Asset Conversion pool id for `AssetKind::Local(NTVE)` and `AssetKind::Local(stNTVE)`
3. Read `pallet_asset_conversion::Pools[pool_id].lp_token`
4. Accept only that LP asset id

The Asset Conversion adapter seeds `NextPoolAssetId` into the LP namespace before creating pools, so canonical LP ids stay out of ordinary local asset id space.

## Storage Topology

### Core pool and receipt state

| Storage | Role | Notes |
| --- | --- | --- |
| `Pools[asset_id]` | Share-vault totals | shares and accounted balance |
| `LiveStakedAssetBaseAssets[staked_asset_id]` | Reverse receipt lookup | bounded receipt -> base lookup |

### Native LP security state

| Storage | Role |
| --- | --- |
| `NativeLpLocks[(account, operator)]` | Collator-specific locked LP position |
| `NativeSecurityParticipants` | Bounded accounts with at least one active collator LP position |
| `NativeNominationOperators[account]` | Bounded operators for one active participant |
| `OperatorNativeLpLocked[operator]` | O(1) backing read for each bounded candidate during readiness, eligibility filtering, and session ranking |
| `AccountNativeLpLocked[account]` | O(1) account backing read for governance `NativeVotePower` |
| `TotalNativeLpLocked` | O(1) global backing read for governance turnout/supply projection |
| `PendingNativeLpUnlocks[(account, operator)]` | Delayed withdrawal request after collator backing removal |

Unlock requests immediately remove LP from backing and reward/governance aggregates, then delay token withdrawal by `NativeLpUnlockDelay`. Repeated requests accumulate under one `(account, operator)` record and extend to the latest maturity. Full exit may be followed by a new active lock while old pending custody remains separate; withdrawing old custody cannot rewrite the new position.

`try_state` reconciles participant/operator indexes against active positions, the retained account/operator/global LP aggregates, pending unlocks, and physical canonical LP custody. It also reconciles every retained reward snapshot/pot pair, Planned/Open/Finalized status and epoch identity, claimed totals, claim-marker eligibility, exact liability, and reward custody lower bound excluding the persistent ED anchor.

Orphan pots, missing snapshots, active-epoch mismatch, underfunded custody, and liability drift fail closed. This evidence path may traverse storage because it is restricted to `try-runtime`; consensus entrypoints use bounded vectors and direct lookups.

### Native governance custody state

- `NativeGovernanceLpLocks[account]`: standalone `NTVE/stNTVE` LP locked for NativeVotePower only
- `PendingNativeGovernanceLpUnlocks[account]`: delayed standalone LP withdrawal
- `NativeGovernanceAssetLocked[(account, asset_id)]`: locked `$NTVE` or `stNTVE` for NativeVotePower
- `TotalNativeGovernanceAssetLocked[asset_id]`: aggregate locked native-governance asset amount
- `PendingNativeGovernanceAssetUnlocks[(account, asset_id)]`: delayed native-governance asset withdrawal

Standalone governance LP feeds NativeVotePower but not nomination rewards. Account collator-locked value is no longer stored separately: `native_locked_lp_position` derives it from at most `MaxNominationsPerAccount` indexed positions, and session reward snapshots derive only the eligible-operator subset they actually need.

### Session security snapshot and funding state

- `ActiveNativeSecurityEpochSnapshot`: currently active Open session-native snapshot
- `NativeSecurityEpochSnapshots[epoch]`: retained snapshot keyed by canonical `SessionIndex`
- `NativeSecurityRewardPots[epoch]`: frozen denominator, certified credit, paid total, and Planned/Open/Finalized status
- `NativeSecurityRewardLiability`: exact sum of certified credit not yet settled
- `NativeSecurityRewardClaims[(epoch, account)]`: duplicate-proof marker for one paid frozen right

Retained reward maps are hard-bounded to `SecurityRewardClaimHorizon + current + one planned` epochs. Consensus retention scans are bounded by that invariant; planning rejects an overdue epoch, a second Planned pot, or a full retained window before constructing a new snapshot.

The snapshot enumerates only `NativeSecurityParticipants` and candidate-eligible operators supplied by runtime planning. It freezes each participant's LP assigned to that eligible set, conservative native value, governance coefficient, reward weight, each eligible operator's conservative backing, and the total denominator.

Planning prevalidates retention admission, constructs the complete value, then stores one zero-credit Planned snapshot/pot without changing active state. Session start settles the oldest overdue epoch first, promotes the current plan to Open, finalizes the prior pot, and replaces the active snapshot transactionally. If settlement fails, the current zero-credit plan is removed and newer planning remains blocked until the oldest obligation settles.

Later locks, unlocks, redelegation, governance memory, pool valuation, or candidate eligibility cannot rewrite Planned/Open/Finalized values; only a later plan observes them.

`fund_native_security_reward(amount)` is the typed certified pallet call. It derives the current `SecurityEpoch`, requires the configured funding origin, transfers native currency only from `SecurityRewardFundingSource`, and updates pot credit plus liability in one transaction. The runtime Fee Sink adapter also preflights and certifies only the exact Fee Sink-to-reward-account native leg. Direct reward-account balance has no accounting effect.

The accepted `frame-omni-bencher 0.22.0` production-runtime run used 100 steps, 50 repeats, and measured ProofSize with one distinct candidate operator per participant. For participants `p ∈ [1, 100]` and retained epochs `r ∈ [1, 13]`, planning charges `91,186,330 + 51,406,814p + 4,054,386r` RefTime plus database Weight and `11,061 + 2,854p + 2,597r` ProofSize.

At `p = 100` and measured-success `r = 13`, planning charges 5,284,574,748 RefTime and 330,222 proof bytes before database Weight. The runtime conservatively charges one additional retained-epoch unit so a full-window rejection is covered. Session retention at the hard `r = 14` bound charges 263,468,359 RefTime, 290,294 ProofSize, 119 reads, and 105 writes. Trusted contraction charges 27,099,000 RefTime, 14,309 ProofSize, four reads, and four writes. All are Mandatory session work.

`MaxNativeSecurityParticipants = 100` follows the operator/candidate topology and measured range. `MaxNominationsPerAccount = 16` remains a position bound, while one account contributes at most one participant snapshot row.

## Core Execution Flows

### 1. Pool registration

`register_staking_asset(asset_id)`:

1. Requires `AdminOrigin`
2. Requires base asset existence
3. Requires a resolvable receipt id and creates the receipt asset through `StakedAssetLifecycle`
4. Indexes receipt -> base in `LiveStakedAssetBaseAssets`
5. Creates `Pools[asset_id]` with current pool-account backing as `accounted_balance`
6. Emits `StakingAssetRegistered`

If the pool account is prefunded before registration, that balance becomes accounted backing immediately rather than dust.

### 2. Liquid staking

The public `stake(asset_id, amount)` call admits every registered staking asset, including `$NTVE` through `NativeStakingAssetId`. Native and non-native staking share the same mutation contract, and runtime adapters pass the configured asset identity directly.

Staking uses one share formula:

```text
if total_shares == 0:
  minted_shares = amount
else:
  minted_shares = amount * total_shares / accounted_balance
```

Implementation details:

- Math uses `U256` intermediates
- `sync_pool_state(asset_id)` runs before crediting shares
- `total_shares == 0 && accounted_balance > 0` rejects as `PoolHasUnownedBalance`
- Every successful stake mints the resolved `stXXX` receipt
- Successful stake touches reward snapshot state for the next epoch

For native `$NTVE`, staking is liquid: it creates transferable `stNTVE`, not collator security backing. `stake_value(asset_id, account)` derives one value from current receipt ownership; no passive/delegated exposure model exists.

### 3. Unstake

`unstake(asset_id, shares)` burns the caller's resolved `stXXX` receipt balance and returns the proportional base-asset amount:

```text
available_shares = live stXXX balance
amount_out = shares * accounted_balance / total_shares
```

Native unstake is therefore an exit from liquid `stNTVE` value, not an exit from collator nomination. Collator nomination exits use the LP unlock lifecycle.

### 4. Native LP collator nomination

`lock_native_lp_for_collator(lp_asset_id, amount, operator)`:

1. Ensures `operator` is valid through `NativeOperatorValidator`
2. Ensures `lp_asset_id` is the canonical `NTVE/stNTVE` LP
3. Prevalidates first-position admission against `MaxNativeSecurityParticipants` and `MaxNominationsPerAccount`
4. Transfers LP from the user into `native_lp_lock_account()`
5. Updates the position plus participant/operator indexes
6. Updates the session-ranking, governance-account, and governance-global aggregates: `OperatorNativeLpLocked`, `AccountNativeLpLocked`, and `TotalNativeLpLocked`
7. Touches native reward snapshots when the native pool exists
8. Emits `NativeLpLocked`

During the current trusted-collator phase, session sets use the configured invulnerables directly. Collator-locked LP remains the authoritative backing surface for nomination rewards, NativeVotePower, and bounded valuation. If permissionless candidate ranking is enabled later, candidate ordering uses conservative collator-locked LP native-equivalent value; the removed native-binding compatibility path no longer affects candidate ordering.

### 5. Unlock and redelegation lifecycle

`request_unlock_native_lp(operator, amount)`:

- Requires no active governance lock horizon for the account
- Removes backing immediately from operator/account/collator aggregates
- Creates or updates `PendingNativeLpUnlocks[(account, operator)]`
- Preserves every already Planned, Open, or Finalized snapshot and pot unchanged
- Emits `NativeLpUnlockRequested`

`withdraw_unlocked_native_lp(operator)` transfers the LP back after `NativeLpUnlockDelay`.

`redelegate_native_lp(from_operator, to_operator, amount)` moves locked LP between operators without releasing custody to the account. It validates the new operator and updates operator aggregates while preserving account-level and physical-custody totals. Already Planned/Open/Finalized snapshots stay immutable; the changed operator appears only in a later plan built from canonical positions.

### 6. Governance-only native custody

`lock_native_lp_for_governance(lp_asset_id, amount)` locks canonical `NTVE/stNTVE` LP for NativeVotePower without collator nomination.

`lock_native_asset_for_governance(asset_id, amount)` locks either `$NTVE` or `stNTVE` for NativeVotePower.

Governance unlock requests are blocked while `NativeGovernanceLockProvider::lock_until(account)` is active. The runtime provider reads `pallet-governance::GovernanceLocks`.

### 7. Conservative native-equivalent LP valuation

Runtime valuation is centralized in `DelegationWeightedCollatorSessionManager::try_conservative_native_lp_value(locked_lp)`, with the legacy scalar wrapper reserved for non-security display/governance consumers:

```text
native_equivalent = 2 * min(reserve_NTVE, reserve_stNTVE * staking_exchange_rate) * locked_lp / lp_supply
```

Missing receipt identity, pool identity, reserves, issuance, staking shares, or backing returns `None`. LP-backed readiness consumes that typed result and fails closed instead of treating unavailable valuation as zero backing; scalar consumers may explicitly narrow `None` to zero only outside security admission and snapshot readiness.

This value is used by:

- Permissionless candidate ranking through operator locked LP when that runtime phase is enabled; equal conservative backing is resolved by canonical account order, and collator-selection deposits never participate in security ranking
- Atomic native security snapshot weights through eligible account collator-locked LP
- Governance NativeVotePower through aggregate account locked LP
- Read-model views through `RuntimeNativeStakingReadModelProvider`

The formula intentionally ignores optimistic value from an unbalanced pool side.

## Reward Architecture

The pallet owns session eligibility freezing and certified native security funding. Ready LP-backed planning atomically retains candidate-eligible LP value, governance coefficients, account weights, and the denominator for the future `SecurityEpoch`; `start_session` activates that exact snapshot and finalizes the prior pot. Block hooks cannot plan, activate, fund, or mutate reward state.

Certified funding is explicit rather than inferred: the configured operation moves native currency from Fee Sink custody to `native_security_reward_account()` while increasing the matching pot and exact liability. Multiple certified contributions accumulate in the same Open pot. Unsolicited reward-account balance remains uncredited custody.

The generic reward engine remains absent: there is no block-number-derived reward identity, sparse touch set, rollover cursor, balance-delta inference, bootstrap call, non-native denominator, generic claim path, truncation state, or reward-event ingress. Generic share-vault yield remains independent.

Native liquid and bounded batch claims remain mode-independent and share frozen-snapshot settlement with exact liability reduction and `SecurityRewardClaimHorizon` admission. In batch order, each `NativeSecurityRewardClaimed` event reports the sequential liability immediately after its own epoch claim rather than repeating the final batch value.

Session-start retention and permissionless expiry recovery use one transition that atomically returns exact unclaimed remainder, rounding dust, and uncredited custody excess to Fee Sink once. The transition verifies retained custody against remaining liability, clears at most the snapshot participant bound of claim markers, and removes the snapshot and pot.

Liability decreases only by accounted reward remainder. There is no intermediate expired state or separate cleanup call.

Atomic compound claims consume the same finalized claim once, derive the native/staked split from current staking-share and canonical pool ratios with widened arithmetic, mint `stNTVE` through the staking pool, add canonical liquidity under a runtime 1% ratio/debit bound and caller `min_lp_out`, then lock measured LP output to the explicit validated operator. The pallet transaction rolls back claim accounting, native payout, staking, liquidity, LP minting, and nomination custody on any failure.

## Governance Integration

### NativeVotePower sources

The runtime `$BLDR` protection track sums explicit custody sources:

1. Locked `$NTVE`
2. Locked `stNTVE`, converted through staking exchange rate
3. Standalone locked `NTVE/stNTVE` LP, conservatively valued
4. Collator-locked `NTVE/stNTVE` LP, conservatively valued

Liquid balances outside these custody surfaces do not count as NativeVotePower.

### Frozen ballot settlement

`pallet-governance` stores `ProposalBallot { account, vote_epoch, weight, raw_power }` at vote time.

Resolution and tally views sum stored ballot facts, not live provider state. Later AMM reserve donations, exchange-rate changes, or custody changes do not mutate already-cast ballot weight.

### Governance lock horizon

Each accepted ballot extends:

```text
GovernanceLocks[account].lock_until = max(current, proposal_effective_primary_close_epoch + ProposalEnactmentDelay)
```

Staking unlock paths consult this horizon before reducing NativeVotePower custody.

## Runtime Bindings

The reference runtime wires `pallet-staking` with these key adapters:

- `RuntimeNativeSecurityModeProvider`: owns the code-level `TrustedSet | LpBackedSelection` decision; production returns `TrustedSet`, while the isolated `runtime-benchmarks` build selects `LpBackedSelection` so mode-gated calls measure their reachable production branch without adding mutable consensus policy
- `RuntimeNativeOperatorValidator`: preserves trusted invulnerables as valid existing custody targets and admits candidate targets only under `LpBackedSelection`; the pallet separately rejects all new collator nomination and redelegation calls outside that mode
- `RuntimeNativeStakingLpAssetValidator`: validates canonical `NTVE/stNTVE` LP token
- `RuntimeStakedAssetIdResolver`: resolves base asset -> receipt asset id
- `RuntimeStakedAssetLifecycle`: creates receipt assets and metadata
- `RuntimeSecurityEpochProvider`: exposes pallet-session `SessionIndex` through the exact `SecurityEpoch` alias; runtime regression evidence proves block movement cannot advance security identity
- `RuntimeGovernanceParticipationCoefficientProvider`: reads the governance-owned bounded participation coefficient when Staking builds a snapshot
- `RuntimeNativeStakingReadModelProvider`: exposes native pool/LP valuation for bounded views
- `RuntimeNativeGovernanceLockProvider`: reads governance lock horizon

## Actors and Asset Conversion Integration

`pallet-deos-actors` remains tokenomics-agnostic. It exposes generic:

```text
Task::DonateLiquidity { asset_a, asset_b, amount, max_ratio_error }
```

The runtime-specific `TmctolLiquidityOps::donate_liquidity` maps the `NTVE/stNTVE` pair to:

```text
AssetConversionAdapter::donate_native_staking_liquidity_from_ntve
```

That helper:

1. Reads current `NTVE/stNTVE` reserves and staking exchange rate
2. Computes the native stake-vs-donate split needed for balanced donation
3. Stakes the required `$NTVE` leg to mint `stNTVE`
4. Donates both legs directly into the pool account without minting LP
5. Rolls back transactionally on any failure

This is the protocol LP-farming path: System Actors strengthens existing LP holders by increasing reserves per LP token rather than minting claimable rewards.

## Read-Model Classification

### Bounded authoritative on-chain projections

The current runtime exposes bounded view/query surfaces for:

- `native_staking_exchange_rate()`
- `native_staking_liquidity_pool()`
- `native_locked_lp_position(account)`
- `native_collator_lp_position(account, operator)`
- `native_governance_custody_position(account, asset_id)`
- Governance `account_governance_power_view(domain, item_id, account)`
- Governance proposal tallies, timing, status, recent finalized proposal window, and vote-power profiles

These are intended for raw client / light-client consumption because they read bounded state.

### Externally indexed / materialized staking views

The following remain indexer/materialized responsibilities:

- Long-range reward history
- Full account position history
- Historical AMM reserve charts
- Cross-epoch APY analytics
- Operator backing history over time
- All holders sorted by locked LP or retained reward claimability once settlement ships

Do not move unbounded history or sorted dashboards into consensus state.

## Public Surface Ownership and Falsification

`src/lib.rs` owns the exhaustive calls, storage, events, errors, traits, public SCALE models, and bounded getter signatures; the implementation modules above own their delegated mechanics. This map groups those names by one constructor/mutator family and does not create a second enum or ABI inventory.

| Surface family | Shipped constructor or mutator | Explicit invariant | Executable evidence |
| --- | --- | --- | --- |
| Pool and liquid receipt | `register_staking_asset`, `stake`, `unstake` | Receipt shares remain transferable claims on exact accounted backing | `generic_stake_mints_liquid_native_receipt_without_binding`, `transferred_receipt_holder_can_unstake` |
| Native LP nomination | Lock, unlock, withdraw, and redelegate calls | Canonical LP custody, bounded indexes, delayed exit, and immediate active-backing removal share one position owner | `lock_native_lp_for_collator_moves_lp_into_lock_account`, `native_lp_redelegate_moves_backing_between_operators`, `try_state_rejects_native_nomination_index_drift` |
| Governance custody | Native asset, receipt, and LP governance lock calls | Frozen ballot rights and the aggregate lock horizon prevent withdrawal from changing settled power | `native_governance_lp_lock_unlock_lifecycle_updates_vote_power_aggregates`, `native_governance_asset_unlock_respects_account_governance_lock_horizon` |
| Session security | Runtime session manager calls retention, contraction, planning, and activation | One `SessionIndex` owner settles overdue epochs, contracts Open/Planned state under Trusted mode, and freezes LP-backed eligibility and reward weight atomically | `lp_backed_to_trusted_transition_preserves_every_retained_obligation`, runtime `trusted_session_boundary_finalizes_open_state_and_removes_unactivated_planning` |
| Certified rewards | Funding, liquid/batch claim, compound, and atomic expiry calls | Certified credit, exact liability, claim uniqueness, custody, and bounded retention mutate transactionally | `session_retention_runs_four_claim_horizons_without_external_cleanup`, `compound_security_reward_claims_roll_back_every_effect`, `try_state_reconciles_native_security_reward_liability_and_custody` |
| Runtime adapters and views | `Config` providers plus bounded getter/read-model functions | Mode, epoch, valuation, funding source, compound path, and client capabilities each have one runtime owner | Runtime `security_epoch_identity_is_shared_by_runtime_planning_funding_and_claim_views`, `compound_path_proves_exact_reward_reserve_issuance_custody_and_backing_deltas` |

Errors are typed fail-closed boundaries of these families rather than independent mechanisms. `native_security_view` returns only `NativeSecurityViewError::{RetentionBoundExceeded, MultiplePlannedEpochs}`; the exhaustive package test executes both view-only corruption results and fails compilation on an unclassified addition.

Events report only committed transitions from the same mutators. Package `src/tests.rs`, runtime `staking_integration_tests.rs`, generated metadata, and production weights falsify the implementation map.

## Operational Watchpoints

### Native AMM availability

Native LP nomination, Actors donation, and read-model valuation require the canonical `NTVE/stNTVE` pool to exist and be non-empty. The local development preset registers the native staking asset and `stNTVE` receipt at genesis and seeds the LP asset-id namespace, while `scripts/seed-web-client-state.sh` can create and fund the local pool after the chain starts.

### Production/operator NTVE/stNTVE bootstrap flow

Outside the local-dev preset, the canonical pool should be launched through an explicit operator/governance sequence rather than hidden genesis assumptions:

1. Register the local native staking asset with `pallet-staking` so the staking pool exists and the `stNTVE` receipt asset is initialized.
2. Ensure the Asset Conversion LP namespace is seeded into `TYPE_LP | 1` before creating the pool so the LP token cannot collide with local, staked, or foreign assets.
3. Create the canonical `AssetKind::Local(NTVE) / AssetKind::Local(stNTVE)` pool through the runtime/governance-approved pool-creation path.
4. Seed balanced initial liquidity from a designated bootstrap account; this mints the initial LP supply to that account and makes read-model valuation non-empty.
5. Run readiness checks before enabling dependent flows: `native_staking_liquidity_pool()` returns a pool, both reserves are non-zero, LP total issuance is non-zero, and `RuntimeNativeStakingLpAssetValidator` accepts the LP id. Operators can use `scripts/bootstrap-native-staking-local.sh check` as the local read-only readiness probe for this phase.
6. Activate the native staking LP provisioning System Actors only after the readiness checks pass; activation remains guarded by `activate_native_staking_liquidity_actor` so donation execution cannot start against a missing or empty pool.
7. If any step after pool creation fails, leave the actor inactive and treat remediation as an operator/governance action; do not silently fall back to liquid `stNTVE` balances or transfer-event-derived backing.

### Governance locks block vote-power withdrawal

Unlock requests for native governance custody and collator LP check the account governance lock horizon. A user may be unable to reduce NativeVotePower until the relevant proposal's enactment horizon expires.

### Pre-fork storage baseline

This repository is still the forkable framework line. Storage versions are current baseline markers rather than deployed-chain migration history. Downstream live forks own explicit migrations once launched.

## Current Limitations and Remaining Work

- Certified funding, retained snapshots/pots, exact liabilities, liquid/batch/compound settlement, claim markers, horizon admission, and atomic bounded expiry are implemented; deterministic package evidence runs four cleanup-free horizons and one LP-backed-to-Trusted transition with Open, Planned, partially claimed, compound-eligible, excess-custody, and pending-unlock state
- The accepted 100-step, 50-repeat retention benchmark charges 263,468,359 RefTime, 290,294 ProofSize, 119 reads, and 105 writes at the 14-epoch/100-participant runtime bound; the exact modularized candidate production Wasm SHA-256 is `43d2120a32dd3a9f085da71da016f4251775fb231ac5506fedc55aeb8929b368`
- Browser transport and staking-widget presentation expose mode-gated liquid claim and atomic compound with explicit epoch/operator/minimum-output inputs; composed runtime evidence proves claim consumption through canonical LP mint and operator lock, while live-network execution remains open
- Runtime snapshot weights use accepted production measurement; production forks should rerun benchmarks on their target hardware and runtime profile before launch
