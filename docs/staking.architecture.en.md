# Staking: Share-Vault, Receipt, and Native Binding Architecture

> **On-Chain Namespace**
>
> - Pallet: `pallet-staking`
> - PalletId: `staking0`
> - The pallet does not revolve around one shared operational account. It derives deterministic per-asset sovereign accounts instead:
>   - `pool_account(asset_id) = PalletId.into_sub_account_truncating(asset_id)`
>   - `reward_account(asset_id) = blake2_256((PalletId, "reward", asset_id))`
> - Current runtime native staking asset id: `0` (`$NTVE`)
> - Receipt namespaces:
>   - native/local receipts: `0x5...`
>   - foreign receipts: `0x6...`

## Executive Summary

`pallet-staking` is the DEOS reference runtime's live multi-asset share-vault staking implementation.
It already does more than the original share-vault kernel: in the current runtime it combines deterministic per-asset pool accounts, tokenized staking receipts, a native-only collator-binding surface, and the first bounded governance-conditioned reward path. In the shipped reference line, that staking surface serves the TMCTOL standard.

The core invariant is still the simple one:

> Backing inflow belongs to the pool and raises share value without iterating all stakers

What changed in implementation is the ownership and reward surface around that invariant:

- Ownership is moving from legacy `Positions` into transferable `stXXX` receipts
- Native `$NTVE` is the only security-relevant staking asset
- Governance-conditioned reward inflow is isolated into a second sovereign channel per asset
- Reward settlement compounds back into fresh same-asset `stXXX`

This document describes the implemented runtime architecture, not the abstract contract alone.

## Architecture Overview

### Design Principles

1. `Pool-first accounting`
   Share price moves through pool backing, not per-holder reward writes
2. `Governance-explicit onboarding`
   Asset registration and staking registration remain separate decisions
3. `Receipt-forward ownership`
   Fresh pools mint/burn `stXXX`; legacy `Positions` survive only as a compatibility bridge
4. `Native special case`
   Only `$NTVE` can bind to collator backing; non-native pools remain economic-only
5. `Dual-inflow separation`
   Backing inflow and governance-conditioned reward inflow are different channels with different accounting rules
6. `Runtime-as-Config`
   Governance domain, reward epoch, coefficient source, receipt lifecycle, and operator validation all come from runtime wiring

### System Architecture

```mermaid
graph TD
    Gov[Governance] -->|register_staking_asset| Pool[Per-asset pool]
    Gov -->|initialize_staked_asset| Receipt[stXXX asset class]
    Gov -->|bootstrap_reward_snapshot| Snap[Reward snapshot state]

    User[User] -->|stake / stake_native| PoolAccount[pool_account(asset_id)]
    PoolAccount --> Pool
    Pool -->|mint shares| Receipt

    External[External inflow] -->|same asset| PoolAccount
    Pool -->|lazy sync| SharePrice[Higher share price]

    RewardSource[Reward inflow] -->|same asset| RewardAccount[reward_account(asset_id)]
    RewardAccount -->|on_idle ingress| RewardEpochs[RewardEpochAccruals]

    GovMem[Governance reward memory] -->|coefficient| Snapshots[Reward weight snapshots]
    Receipt -->|transfer / mint / burn events| Snapshots
    User -->|claim_reward| RewardAccount
    RewardAccount -->|auto-compound| PoolAccount
    Pool -->|mint fresh stXXX| User

    User -->|bind_native / clear_native_binding| Binding[NativeBindings]
    Binding --> Backing[delegated_native_backing]
    Backing --> Collators[Trusted collator weighting]
```

## Account and Asset Topology

### Per-asset sovereign channels

For each registered staking asset, the pallet derives two deterministic accounts:

1. `pool_account(asset_id)`
   - holds common backing for the share-vault
   - receives direct stake deposits
   - receives external backing inflow
   - receives claimed reward amounts during auto-compound settlement

2. `reward_account(asset_id)`
   - isolated reward-liability channel
   - receives governance-conditioned reward funding
   - must not change share price directly
   - is consumed only through reward settlement into the pool

This separation is one of the most important implementation upgrades over the original pure share-vault kernel.

### Receipt asset lifecycle

Receipt derivation is runtime-resolved through `StakedAssetIdResolver`.
In the live runtime:

- `$NTVE` and local protocol assets derive into `0x5...`
- Foreign assets derive into `0x6...`
- Receipt ids remain local `pallet-assets` asset classes

At `register_staking_asset(asset_id)`:

- The pool is registered only if the base asset already exists
- The pallet computes `pool_account(asset_id)`
- If a receipt id is resolvable, the runtime lifecycle hook creates the `stXXX` asset class immediately
- The runtime assigns deterministic metadata and uses the pool sovereign account as admin/owner

Already-live legacy pools that predate receipt mode are upgraded later with `initialize_staked_asset(asset_id)`.

## Storage Topology

### Core ownership and pool state

| Storage                          | Role                               | Notes                                                      |
| :------------------------------- | :--------------------------------- | :--------------------------------------------------------- |
| `Pools[asset_id]`                | Pool totals                        | `total_shares`, `accounted_balance`, `active_staker_count` |
| `Positions[(asset_id, account)]` | Legacy share ownership             | Compatibility-only once receipt mode exists                |
| `NativeBindings[account]`        | Native account -> operator binding | Only meaningful for `$NTVE`                                |
| `OperatorCommissions[operator]`  | Per-operator commission            | Bounded by runtime `MaxOperatorCommission`                 |

### Reward accounting and snapshot state

| Storage                                                    | Role                                                 |
| :--------------------------------------------------------- | :--------------------------------------------------- |
| `RewardEpochAccruals[(asset_id, epoch)]`                   | Reward inflow attributed to one epoch                |
| `RewardLiabilityBalances[asset_id]`                        | Total unsettled reward liability                     |
| `RewardEpochTouchedAccounts[(epoch, asset_id)]`            | Sparse account touch set for next epoch rollover     |
| `RewardActiveWeightSnapshots[(asset_id, account)]`         | Current active reward snapshot                       |
| `RewardActiveTotalWeights[asset_id]`                       | Current active total reward weight                   |
| `RewardEpochTotalWeights[(asset_id, epoch)]`               | Frozen denominator used for one epoch's claims       |
| `RewardEpochWeightSnapshots[((asset_id, epoch), account)]` | Historical per-epoch snapshot                        |
| `RewardClaims[((asset_id, epoch), account)]`               | Claim-consumption marker and amount                  |
| `LastProcessedRewardEpoch`                                 | Last rolled reward epoch                             |
| `LastRewardIngressTruncatedEpoch`                          | Latest scan-cap truncation signal                    |
| `RewardTruncatedEpochs[epoch]`                             | Marks epochs as incomplete and therefore unclaimable |

### Important compatibility detail

`active_staker_count` still exists because legacy pools may still hold `Positions`.
In fresh receipt-mode pools it is no longer the canonical measure of economic ownership.
During migration it is best understood as a rollout-readiness counter for residual legacy state.

## Transitional Ownership Reality

The current implementation does not yet have one pure ownership source of truth across all pools.
During the receipt migration, several important paths intentionally use a hybrid surface:

- `unstake(asset_id, shares)` accepts the sum of `live stXXX balance + legacy Position shares`
- Query helpers such as `stake_value(...)` and `stake_fraction(...)` resolve through `effective_share_balance(...)`
- Reward snapshots also currently use that same effective share balance, so untouched legacy holders are not excluded from governance-conditioned reward weight while the bridge still exists

So the current architecture is not simply `receipt mode or legacy mode`.
It is a deliberately mixed compatibility regime until real chain state proves legacy positions are drained.

Two consequences matter operationally:

1. `active_staker_count` is a legacy-drain metric, not a universal ownership metric
2. deleting `Positions` too early would break both unstake rights and reward-snapshot correctness for still-unmigrated holders

## Core Execution Flows

### 1. Pool registration and receipt-mode activation

`register_staking_asset(asset_id)` does the following:

1. Root/governance ensures the base asset exists
2. Creates receipt asset class immediately when namespace resolution is supported
3. Initializes `Pools[asset_id]`
4. Seeds `accounted_balance` from the pool sovereign's current live balance
5. Emits `StakingAssetRegistered { asset_id, pool_account, reward_account }`

That last step matters operationally: if the pool account already holds backing when the pool is created, that balance becomes accounted state from genesis of the pool rather than silent dust.

### 2. Stake path

Current public entry paths:

- `stake(asset_id, amount)` for non-native assets only
- `stake_native(amount, operator)` for `$NTVE`

Generic native stake is intentionally rejected with `NativeStakeRequiresOperator`.

Both paths end up in the same share-crediting logic:

```text
if total_shares == 0:
  minted_shares = amount
else:
  minted_shares = amount * total_shares / accounted_balance
```

Implementation details:

- Share math uses `mul_div_floor(...)` with `U256` intermediates
- The pallet first calls `sync_pool_state(asset_id)`
- If `total_shares == 0 && accounted_balance > 0`, stake is rejected as `PoolHasUnownedBalance`
- In receipt mode, minted shares become `stXXX`
- In legacy mode, minted shares become `Positions[(asset_id, account)]`
- Every successful stake marks the account as a reward-touch candidate for the next epoch

### 3. Unstake path

`unstake(asset_id, shares)` operates against the combined ownership surface during migration:

```text
available_shares = live_receipt_balance + legacy_position_shares
amount_out = shares * accounted_balance / total_shares
```

The burn order is implementation-specific and important:

1. burn receipt balance first when present
2. burn residual legacy shares from `Positions` only if needed
3. transfer underlying from `pool_account(asset_id)` back to the caller
4. reduce `pool.total_shares` and `pool.accounted_balance`

This lets a transferred receipt holder exit even if they never had a legacy `Position`.

### 4. Lazy sync and outflow detection

`sync_pool_state(asset_id)` reads the actual pool-account balance and compares it to `accounted_balance`.

- Positive delta -> pool backing is updated and `PoolSynced` is emitted
- Negative delta -> rejected as `PoolOutflowDetected`

This is a deliberate implementation guard: the pallet refuses to silently normalize unexplained outflows.

### 5. Unowned-balance recovery

If a pool has backing before anyone owns shares, governance may call:

- `recover_unowned_pool(asset_id, beneficiary)`

The guard differs slightly by mode:

- In receipt mode: `pool.total_shares` must be zero
- In legacy mode: both `pool.total_shares` and `active_staker_count` must be zero

The asymmetry exists because `active_staker_count` is no longer authoritative once transferable receipts exist.

### 6. Legacy bridge

The receipt migration is intentionally monotonic:

- `initialize_staked_asset(asset_id)` backfills the missing receipt asset class
- `convert_position_to_receipt(asset_id)` lets a holder mint `stXXX` equal to their legacy shares and deletes the old `Position`
- Queries and `unstake` keep summing `live_receipt_balance + legacy_position_shares` until the bridge can be retired

This is the main reason the architecture document exists separately from the specification: the live code now includes transitional behavior that the pure share-vault model never needed.

## Native Binding Architecture

### Why native is special

Only the runtime's native staking asset participates in the security path.
All other staking assets stay economic-only.

Current public native surface:

- `stake_native(amount, operator)`
- `bind_native(operator)`
- `clear_native_binding()`
- `delegated_native_backing(operator)`

### Cached shares, read-derived value

The live implementation now keeps a first cached per-operator native-delegation surface for the collator-ranking hot path, but it still derives final backing value from the live native pool ratio.
More concretely:

- `on_idle` refreshes cached per-operator delegated native **shares** from native binding events plus native `stNTVE` balance-change events
- `bind_native` and `clear_native_binding` also refresh that cache immediately when the cache is already clean, so rebinding and clear-to-passive transitions do not wait for later event replay
- The session-manager ranking path reads those cached shares and converts them into live backing value through the current native pool state
- If the bounded native-delegation ingress truncates, ranking safely falls back to the exact read-derived `delegated_native_backing(operator)` path while a bounded two-phase repair loop clears stale cache entries and rebuilds live bindings over later `on_idle` passes

This preserves the intended invariant:

- If `stNTVE` leaves the account, backing falls with it
- If a recipient receives `stNTVE`, it stays passive until they explicitly bind
- If a previously delegated account reaches zero native exposure, the runtime retires that stale binding during cache refresh instead of leaving inert delegated state behind

### Current runtime validator

`RuntimeNativeBindingTargetValidator` accepts:

- Trusted invulnerable collators always
- Permissionless candidates only if `PermissionlessCollatorsEnabled` is active

So the current chain posture remains compatible with the trusted-collator launch line.

## Reward Architecture

### Epoch model

In the current runtime, reward epochs are simply block numbers.
That is intentionally narrow and easy to reason about during the first rollout line.

### Sparse one-epoch-lag snapshots

Reward weight is not recomputed globally.
Instead the pallet keeps a sparse active snapshot line and rolls only touched accounts forward.

The active snapshot stores:

- `shares`
- `coefficient`
- `weight = coefficient * shares`
- `effective_from_epoch`

In the current transition line, `shares` here are not receipt-only.
The snapshot path resolves through the pallet's effective ownership surface, which currently means:

```text
effective_share_balance(asset_id, account)
  = live receipt balance + residual legacy Position shares
```

That is the correctness bridge that keeps already-live legacy holders inside reward accounting until migration is complete.

Touch sources in the live runtime are:

- `stake` / `unstake`
- `stXXX` transfer, mint, burn, deposit, and withdrawal events from `pallet-assets`
- Governance events `WinningVoteRecorded` and `WinningVoteWindowEvicted`

The runtime ingress implementation aggregates touches and reward inflows in memory before mutating staking storage.

### Reward inflow ingestion

`on_idle` calls the runtime `RewardSnapshotEventIngress`.
That adapter now receives the real remaining idle budget after earlier staking maintenance work and scans system events only while the projected scan/touch/inflow work still fits inside that residual budget, subject to the hard `MaxRewardEventScanPerBlock` cap.
Receipt-driven reward-touch events no longer rediscover their base staking asset by iterating all pools; the pallet now maintains a live `stXXX -> base asset` reverse index when receipt assets are created or backfilled, and reward ingress resolves that mapping through the indexed surface. Governance-driven reward-touch events likewise no longer walk every staking pool to find matching domains; staking-asset registration now maintains a bounded `domain -> reward assets` index that the runtime reward ingress consumes directly.
It performs two bounded jobs:

1. aggregate deposits into `reward_account(asset_id)` and call `note_reward_inflow(asset_id, amount)`
2. aggregate touched accounts and call `note_reward_touch(asset_id, account)`

When either the remaining idle budget or the scan cap is exceeded:

- `RewardIngressTruncated` is emitted
- The epoch is marked in `RewardTruncatedEpochs`
- Claims for that epoch are blocked with `RewardEpochIncomplete`

This is an explicit correctness-over-convenience choice.

### Reward denominator freezing

`note_reward_inflow(asset_id, amount)` does three important things:

1. increases `RewardEpochAccruals[(asset_id, epoch)]`
2. increases `RewardLiabilityBalances[asset_id]`
3. freezes `RewardEpochTotalWeights[(asset_id, epoch)]` from the current active total weight if not already frozen

That makes reward accounting ingress-time attributable rather than balance-delta reconstructive.

### Reward claim path

Claim settlement is same-asset auto-compound only.

Public surfaces:

- `claim_reward(asset_id, epoch)`
- `claim_reward_batch(asset_id, epochs)`

Implementation path:

1. verify the epoch is closed
2. reject truncated epochs
3. reject duplicate claim attempts
4. require live receipt mode for the pool (`live_staked_asset_id(asset_id)` must exist) or fail with `StakedAssetNotInitialized`
5. compute `reward_claimable(...)` from epoch accrual, frozen denominator, and the claimant's epoch snapshot
6. move funds from `reward_account(asset_id)` into `pool_account(asset_id)`
7. mint fresh `stXXX` against the pre-compound pool price
8. write `RewardClaims`

This means reward settlement never pays out a separate liquid leg in the current baseline path, but it also means the current claim path is practically receipt-mode dependent.

## Staking Read-Model Contract

This subsystem follows the project-wide [`read-model.contract.en.md`](./read-model.contract.en.md) split.

### Canonical on-chain staking projections

The current pallet already provides chain-native reads for live bounded staking truth through:

- `pool(asset_id)` / `pool_state(asset_id)`
- `pool_account(asset_id)` and `reward_account(asset_id)`
- `position(asset_id, account)` plus effective-share/exposure helpers
- `stake_value(asset_id, account)` / `stake_fraction(asset_id, account)` / `stake_exposure(asset_id, account)`
- `native_binding(account)` for the live `$NTVE` binding surface
- `staked_receipt_balance(asset_id, account)` / `staked_receipt_value(asset_id, account)`
- `reward_coefficient(asset_id, account)`
- `reward_claimable(asset_id, epoch, account)` for known closed epochs
- Reward epoch/liability/truncation state for known `(asset_id, epoch)` queries

These are the authoritative bounded surfaces for current balances, exposure, binding state, reward weight, and known-epoch claimability.

### Indexed / materialized staking views

The pallet intentionally does **not** promise these as canonical on-chain surfaces:

- APY charts and long-range return series
- Historical claim timelines
- Wallet PnL history
- Search/filter across many past epochs or pools
- Dashboard/leaderboard analytics beyond current bounded state

Those belong to events plus external indexing/materialization rather than permanent pallet storage.

### Current launch-line decision for reward-claim discovery

The current runtime now treats reward-claim discovery as an explicit `Indexed / Materialized View`, not as a canonical on-chain projection.

Why:

- The pallet exposes `reward_claimable(asset_id, epoch, account)` for verification of a supplied closed epoch
- But it does **not** expose a bounded per-account recent-claimable-epoch index/query helper
- In the current design, claimable epochs can accumulate across an effectively unbounded historical set, so a fully chain-native discovery list would require either unbounded state growth or an explicit retention/horizon redesign

So the honest contract today is:

- Chain-native for `is epoch E claimable for account A?`
- Indexed/materialized for `which epochs should account A inspect or batch-claim next?`

Consumers SHOULD NOT infer claim-discovery state from raw storage topology and treat that as a stable product contract.

## Public Call Surface

| Call Index | Extrinsic                                      | Role                                |
| :--------- | :--------------------------------------------- | :---------------------------------- |
| `0`        | `register_staking_asset(asset_id)`             | Governance onboarding               |
| `1`        | `sync_pool(asset_id)`                          | User-triggered lazy sync            |
| `2`        | `stake(asset_id, amount)`                      | Non-native entry                    |
| `3`        | `unstake(asset_id, shares)`                    | Generic exit                        |
| `4`        | `recover_unowned_pool(asset_id, beneficiary)`  | Governance recovery                 |
| `5`        | `bind_native(operator)`                        | Attach live `stNTVE` to an operator |
| `6`        | `clear_native_binding()`                       | Return `stNTVE` to passive state    |
| `7`        | `set_operator_commission(commission)`          | Operator-side commission config     |
| `8`        | `stake_native(amount, operator)`               | Native entry path                   |
| `9`        | `initialize_staked_asset(asset_id)`            | Legacy receipt backfill             |
| `10`       | `convert_position_to_receipt(asset_id)`        | Holder migration                    |
| `11`       | `bootstrap_reward_snapshot(asset_id, account)` | Live-chain reward warm-up           |
| `12`       | `claim_reward(asset_id, epoch)`                | Single-epoch auto-compound claim    |
| `13`       | `claim_reward_batch(asset_id, epochs)`         | Bounded multi-epoch sweep           |

## Events and Errors

### Events that matter operationally

| Event                                       | Why it matters                                                                      |
| :------------------------------------------ | :---------------------------------------------------------------------------------- |
| `StakingAssetRegistered`                    | Confirms pool onboarding and publishes both sovereign accounts                      |
| `StakedAssetInitialized`                    | Confirms receipt backfill for a legacy pool                                         |
| `Staked` / `Unstaked`                       | Confirms user-facing entry and exit on the live ownership surface                   |
| `PoolSynced`                                | Signals recognized backing inflow on the common pool line                           |
| `RewardInflowRecorded`                      | Confirms reward-account ingress was attributed to one epoch                         |
| `RewardSnapshotBootstrapped`                | Confirms a live holder was materialized into reward snapshot state before freeze    |
| `RewardClaimed`                             | Confirms same-asset auto-compound settlement and minted receipt amount              |
| `RewardIngressTruncated`                    | Signals correctness-preserving truncation; that epoch must be treated as incomplete |
| `LegacyPositionConverted`                   | Confirms one holder left the legacy bridge and moved into `stXXX`                   |
| `NativeBindingSet` / `NativeBindingCleared` | Confirms live native backing attachment or passive reversion                        |
| `OperatorCommissionSet`                     | Confirms operator-side economic metadata updates                                    |
| `UnownedPoolRecovered`                      | Confirms governance drained prefunded no-owner state                                |

### Errors that expose real architecture boundaries

| Error                                           | Meaning                                                                                            |
| :---------------------------------------------- | :------------------------------------------------------------------------------------------------- |
| `PoolHasUnownedBalance`                         | First-stake capture of prefunded backing is blocked                                                |
| `PoolOutflowDetected`                           | Pool-account balance fell below accounted state; the pallet refuses silent normalization           |
| `StakedAssetIdCollision`                        | Receipt namespace or lifecycle setup is inconsistent with current asset state                      |
| `StakedAssetNotInitialized`                     | Reward claims or receipt conversions are trying to use a pool that is not yet in live receipt mode |
| `NativeStakeRequiresOperator`                   | Generic native entry is intentionally disallowed; native must go through the operator-aware path   |
| `InvalidBindingTarget`                          | Native binding target failed the runtime validator                                                 |
| `RewardEpochWeightFrozen`                       | Bootstrap came too late for the current epoch                                                      |
| `RewardEpochStillOpen`                          | Claim attempted before the epoch closed                                                            |
| `RewardEpochIncomplete`                         | Reward ingress truncation marked the epoch unsafe for settlement                                   |
| `RewardAlreadyClaimed` / `DuplicateRewardEpoch` | Claim replay or malformed batch input was rejected                                                 |

## Runtime Binding

Current runtime wiring in `template/runtime/src/configs/staking_config.rs`:

- `AdminOrigin = Root`
- `NativeStakingAssetId = 0`
- `GovernanceDomainId = AssetId`
- `RewardEpoch = BlockNumber`
- `StakedAssetIdResolver = RuntimeStakedAssetIdResolver`
- `StakedAssetLifecycle = RuntimeStakedAssetLifecycle`
- `RewardGovernanceDomainResolver = identity(asset_id)`
- `RewardCoefficientProvider = Governance::reward_coefficient(domain, account)`
- `RewardSnapshotEventIngress = RuntimeRewardSnapshotEventIngress`

Current runtime constants:

| Constant                         | Value |
| :------------------------------- | :---- |
| `MaxOperatorCommission`          | `50%` |
| `MaxRewardEventScanPerBlock`     | `128` |
| `MaxRewardAccountsPerAssetEpoch` | `256` |
| `MaxClaimEpochsPerCall`          | `16`  |

Receipt lifecycle in the runtime is implemented through `pallet-assets::force_create` + `force_set_metadata`.
For `$NTVE`, the runtime uses explicit metadata `Staked Native Token / stNTVE / 12`.

## Complexity and Growth Pressure

1. `Native backing now has a bounded hot path with bounded dirty-cache repair`
   Candidate ranking no longer rescans `NativeBindings` in its hot path. Instead it reads cached per-operator shares refreshed by bounded event ingress, and truncation now triggers a bounded clear/rebuild repair loop while ranking stays on the exact fallback path.

2. `Receipt-event ingress currently does runtime-side base-asset discovery by pool scan`
   In `RuntimeRewardSnapshotEventIngress`, receipt-asset events are mapped back to a base staking asset by iterating `Pools::<Runtime>::iter_keys()` and matching `staked_asset_id(base_asset_id)`. This is a runtime-adapter growth point, not a pallet-storage flaw, but it is a real current cost surface.

3. `Touched-account saturation is bounded but not loudly signaled`
   `RewardEpochTouchedAccounts[(epoch, asset_id)]` is capped by `MaxRewardAccountsPerAssetEpoch`. When a brand-new account no longer fits, `note_reward_touch(...)` simply returns `false`; there is currently no dedicated saturation event for that specific condition.

4. `Reward rollover cost scales with touched accounts, not all holders`
   This is the intended win of the sparse snapshot model, but it also means ingress quality and touch coverage are part of correctness, not only performance.

5. `Hybrid ownership remains a deliberate complexity tax`
   As long as legacy `Positions` survive, unstake rights, query math, and reward snapshots all depend on the mixed ownership surface rather than a single pure receipt ledger.

6. `Batch reward claims scale with requested epochs`
   `claim_reward_batch(...)` is bounded by `MaxClaimEpochsPerCall`, but still intentionally linear in the number of epochs swept.

## Validation Surface

The implementation is backed by three layers of evidence:

- Pallet unit/regression tests in `template/pallets/staking/src/tests.rs`
- Runtime integration tests in `template/runtime/src/tests/staking_integration_tests.rs`
- FRAME v2 benchmarks plus runtime weight bridge in `template/runtime/src/weights/pallet_staking.rs`

Coverage includes:

- Share-vault math
- Receipt lifecycle and metadata
- Legacy pool conversion
- Transferred-receipt exits
- Native binding and rebinding
- Reward snapshot lag semantics
- Reward ingress truncation behavior
- Same-asset auto-compound claims

## Operator Checklist

### Legacy-drain readiness

Inspect these surfaces together:

- `pool(asset_id).active_staker_count`
- `position(asset_id, account)`
- `staked_receipt_balance(asset_id, account)`
- `stake_value(asset_id, account)`
- `LegacyPositionConverted`

The migration is only truly finished when legacy positions are gone, not merely when receipt mode exists.

### Reward-rollout readiness

Before enabling live reward ingress for an already-active pool, operators should verify:

- Receipt mode is initialized for the pool
- Target live holders have been bootstrapped via `bootstrap_reward_snapshot(...)`
- `reward_active_weight_snapshot(asset_id, account)` exists for the holders that need warm-up
- The current epoch has not already frozen `RewardEpochTotalWeights[(asset_id, epoch)]`
- No `RewardIngressTruncated` event has marked the target epoch incomplete

### Native-binding sanity checks

For native security posture, inspect:

- `native_binding(account)`
- `passive_native_stake_value(account)`
- `delegated_native_stake_value(account)`
- `NativeBindingSet` / `NativeBindingCleared`

That gives the minimal live picture of whether `stNTVE` is currently passive or contributing backing to a trusted collator.

## Current Watchpoints

1. `Legacy bridge still exists`
   `Positions` and `active_staker_count` cannot be deleted until real chain state proves the bridge is drained

2. `Native-delegation truncation now degrades safely while repair catches up`
   If native binding / `stNTVE` ingress truncates, the runtime marks the cache dirty, ranking falls back to the exact `delegated_native_backing(operator)` path, and a bounded two-phase repair loop clears stale cache state before rebuilding live bindings. The remaining pressure is performance evidence and larger regression matrices, not missing repair semantics

3. `Zero-exposure cleanup is now automatic, not user-driven`
   Once a previously delegated account reaches zero native exposure, cache refresh clears the inert binding automatically. That keeps the live binding surface honest, but it also means zero-balance prebinding is not treated as durable delegation state

4. `Reward ingress is bounded, not exhaustive`
   scan truncation intentionally preserves safety by making the epoch unclaimable rather than pretending accounting stayed complete

5. `Live-chain bootstrap is operationally mandatory`
   already-live holders must be bootstrapped with `bootstrap_reward_snapshot(...)` before enabling reward ingress for that epoch

6. `Reward-claim discovery is intentionally off-chain today`
   canonical on-chain state verifies claimability for a supplied epoch, but discovering which epochs a wallet should inspect next is currently an indexed/materialized concern rather than a bounded pallet projection

7. `Generic native automation is intentionally missing`
   the pallet has `stake_native(amount, operator)`, but higher-level automation such as AAA-native staking still needs an operator-aware surface

8. `Non-native staking remains isolated from consensus security`
   this is deliberate and should stay explicit in runtime/UI/operator assumptions

## Conclusion

`pallet-staking` is no longer just a minimal share-vault.
It is now the live convergence point of four concerns:

- Pool accounting
- Tokenized ownership
- Native security binding
- Governance-conditioned reward settlement

The implementation remains anchored to the original O(1) share-vault invariant, but it now carries the real-world transitional and operational machinery required to move from a clean math model to a launchable runtime.

---

- `Version`: 0.1.0
- `Last Updated`: March 2026
- `Author`: LLB Lab
- `License`: MIT
