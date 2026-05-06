# Staking: Share-Vault, Native LP Nomination, and Epoch Reward Architecture

> **On-Chain Namespace**
>
> - Pallet: `pallet-staking`
> - PalletId: `staking0`
> - Deterministic per-asset accounts:
>   - `pool_account(asset_id) = PalletId.into_sub_account_truncating(asset_id)`
>   - `reward_account(asset_id) = blake2_256((PalletId, "reward", asset_id))`
> - Current runtime native staking asset id: `0` (`$NTVE`)
> - Native/local staking receipts use the `0x5...` namespace (`stNTVE`, `stXXX`)
> - Foreign staking receipts use the `0x6...` namespace
> - Native collator nomination is backed by locked canonical `NTVE/stNTVE` LP, not by liquid `stNTVE`

## Executive Summary

`pallet-staking` is the DEOS reference runtime's multi-asset share-vault staking implementation plus the native `$NTVE` security and reward surface used by the current launch line.

The common staking kernel remains simple:

> Backing inflow belongs to the pool and raises receipt value without iterating all holders

The current native implementation adds a separate security layer on top of that kernel:

- `stake_native(amount)` mints liquid, yield-bearing `stNTVE`
- The canonical `NTVE/stNTVE` AMM is the liquidity surface for native security
- Collator nomination requires explicit custody of `NTVE/stNTVE` LP through `lock_native_lp_for_collator`
- Native `NativeVotePower` comes from explicit custody sources, not transferable receipt movement
- Native nomination rewards are recognized by epoch reconciliation and claimed from `reward_account(NTVE)`
- Remaining event scanning is scoped as legacy non-native reward compatibility

This document describes shipped implementation truth. The broader normative target lives in `docs/staking.specification.en.md`.

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
5. `Dual-inflow separation`
   Backing inflow and governance-conditioned reward inflow are separate sovereign channels.
6. `Epoch reward truth`
   Native reward funding is recognized by reward-account balance reconciliation at epoch rollover.
7. `Runtime-as-Config`
   Receipt lifecycle, operator validation, LP validation, reward base weight, governance coefficients, compound routing, and read-model valuation are runtime-provided.

### System Architecture

```mermaid
graph TD
    User[User] -->|stake_native(amount)| NativePool[NTVE share-vault]
    NativePool -->|mint liquid receipt| StNTVE[stNTVE]
    User -->|add liquidity| Amm[Zero-fee NTVE/stNTVE AMM]
    AAA[System AAA LP farmer] -->|DonateLiquidity NTVE/stNTVE| Amm
    Amm -->|mint LP to user| Lp[NTVE/stNTVE LP]
    User -->|lock_native_lp_for_collator| CollatorLock[Collator LP lock custody]
    CollatorLock -->|conservative native value| Session[Collator session ranking]
    CollatorLock -->|reward base| RewardSnapshots[Native reward snapshots]
    GovLocks[Governance LP/asset locks] -->|NativeVotePower| Governance[Governance]
    Governance -->|winning vote touch hook| RewardSnapshots
    RewardSource[Reward source] -->|fund reward_account NTVE| RewardAccount[reward_account(NTVE)]
    RewardAccount -->|epoch reconciliation| RewardEpochs[Reward accrual/liability]
    User -->|claim_nomination_reward| LiquidPayout[liquid NTVE payout]
    User -->|claim_and_compound_nomination_reward| Compound[stake + add LP + lock]
```

## Account and Asset Topology

### Launch reward phases

The reward architecture is phase-aware. Phase 1 uses trusted permissioned collators and keeps user nomination economics disabled: Fee Sink redistribution targets only staking-pool backing inflow and native LP donation. Phase 2 may enable permissionless collators, explicit LP nomination to a collator, and claimable native nomination rewards weighted by locked collator LP plus GovXP.

Phase 2 is treated as a runtime-upgrade boundary. The current launch line should not expose a governance parameter that can silently enable LP nomination or claimable nomination rewards while collator selection is still permissioned.

The intended outer collection rule is unified across block rewards and eligible fees once implemented: 20% to the author / collator and 80% to Fee Sink. Fee Sink then fans out to the active staking ingress channels for the current phase.

### Per-asset sovereign channels

Each registered staking asset has three deterministic ingress accounts:

| Account | Role |
| :------ | :--- |
| `pool_account(asset_id)` | Holds backing for the share-vault and receives stake deposits, external backing inflows, and same-asset auto-compound settlement for non-native rewards |
| `lp_reward_account(asset_id)` | Holds LP-donation funding before an actor/runtime path converts it into balanced AMM donation |
| `reward_account(asset_id)` | Holds claimable reward funding and backs reward liabilities without directly changing pool share price |

The account split is essential: backing inflow changes `stXXX` value, LP reward inflow strengthens AMM reserves, and claimable reward inflow stays claimable through bounded epoch accounting. Phase 1 uses `pool_account` plus `lp_reward_account`; Phase 2 additionally activates `reward_account` for native nomination rewards.

### Receipt asset lifecycle

Receipt ids are runtime-resolved by `StakedAssetIdResolver`:

- `$NTVE` and local assets derive into the local staked namespace (`TYPE_STAKED = 0x5000_0000`)
- Foreign assets derive into `TYPE_STAKED_FOREIGN = 0x6000_0000`
- Receipt classes are local `pallet-assets` assets

At `register_staking_asset(asset_id)` the runtime lifecycle hook creates the receipt class when the id is resolvable. For `$NTVE`, metadata is currently `Staked Native Token` / `stNTVE` / `12` decimals.

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
| :------ | :--- | :---- |
| `Pools[asset_id]` | Share-vault totals | `total_shares`, `accounted_balance`, `active_staker_count` |
| `LiveStakedAssetBaseAssets[staked_asset_id]` | Reverse receipt lookup | Bounded direct lookup for receipt -> base |
| `Positions[(asset_id, account)]` | Legacy share ownership | Compatibility bridge for pre-receipt positions |
| `OperatorCommissions[operator]` | Operator commission | Bounded by runtime `MaxOperatorCommission` |

`Positions` is retained only for legacy compatibility. Fresh ownership is receipt-based.

### Native LP security state

| Storage | Role |
| :------ | :--- |
| `NativeLpLocks[(account, operator)]` | Collator-specific locked LP position |
| `OperatorNativeLpLocked[operator]` | Aggregate LP backing for session ranking |
| `AccountNativeLpLocked[account]` | Aggregate account LP custody for NativeVotePower |
| `AccountNativeCollatorLpLocked[account]` | Collator-locked LP only, used for native nomination rewards |
| `TotalNativeLpLocked` | Aggregate native LP custody |
| `PendingNativeLpUnlocks[(account, operator)]` | Delayed withdrawal request after collator backing removal |

Unlock requests immediately remove LP from backing and reward/governance aggregates, then delay token withdrawal by `NativeLpUnlockDelay`.

### Native governance custody state

| Storage | Role |
| :------ | :--- |
| `NativeGovernanceLpLocks[account]` | Standalone `NTVE/stNTVE` LP locked for NativeVotePower only |
| `PendingNativeGovernanceLpUnlocks[account]` | Delayed standalone LP withdrawal |
| `NativeGovernanceAssetLocked[(account, asset_id)]` | Locked `$NTVE` or `stNTVE` for NativeVotePower |
| `TotalNativeGovernanceAssetLocked[asset_id]` | Aggregate locked native-governance asset amount |
| `PendingNativeGovernanceAssetUnlocks[(account, asset_id)]` | Delayed native-governance asset withdrawal |

Standalone governance LP feeds NativeVotePower but does not feed `AccountNativeCollatorLpLocked`, so it cannot earn nomination rewards.

### Reward accounting and snapshot state

| Storage | Role |
| :------ | :--- |
| `RewardEpochAccruals[(asset_id, epoch)]` | Reward funding attributed to one epoch |
| `RewardLiabilityBalances[asset_id]` | Unsettled reward liability still held by `reward_account(asset_id)` |
| `RewardEpochTouchedAccounts[(epoch, asset_id)]` | Sparse touch set rolled into the next epoch snapshot |
| `RewardActiveWeightSnapshots[(asset_id, account)]` | Current reward snapshot for an account |
| `RewardActiveTotalWeights[asset_id]` | Current denominator candidate |
| `RewardEpochTotalWeights[(asset_id, epoch)]` | Frozen claim denominator |
| `RewardEpochWeightSnapshots[((asset_id, epoch), account)]` | Frozen per-account claim numerator |
| `RewardClaims[((asset_id, epoch), account)]` | Claim-consumption marker and claimed amount |
| `LastProcessedRewardEpoch` | Last fully completed reward epoch rollover |
| `PendingRewardEpochRollover` | Bounded rollover cursor from the prior epoch to the target epoch |
| `LastRewardIngressTruncatedEpoch` | Latest legacy event-scan truncation signal |
| `RewardTruncatedEpochs[epoch]` | Incomplete epoch marker; claims are rejected |

Native reward snapshots use `RuntimeRewardBaseWeightProvider`, which replaces the default share-balance base with conservative collator-locked LP value.

## Core Execution Flows

### 1. Pool registration

`register_staking_asset(asset_id)`:

1. Requires `AdminOrigin`
2. Requires base asset existence
3. Creates receipt asset through `StakedAssetLifecycle` when supported
4. Indexes receipt -> base in `LiveStakedAssetBaseAssets`
5. Creates `Pools[asset_id]` with current pool-account backing as `accounted_balance`
6. Emits `StakingAssetRegistered`

If the pool account is prefunded before registration, that balance becomes accounted backing immediately rather than dust.

### 2. Liquid staking

Public staking calls:

- `stake(asset_id, amount)` for non-native assets
- `stake_native(amount)` for `$NTVE`

Generic native staking through `stake(0, amount)` is rejected with `NativeStakeRequiresDedicatedCall`.

Both staking paths use the same share formula:

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
- Receipt mode mints `stXXX`
- Legacy mode writes `Positions`
- Successful stake touches reward snapshot state for the next epoch

For native `$NTVE`, staking is liquid and passive: it creates `stNTVE`, not collator security backing.

### 3. Unstake

`unstake(asset_id, shares)` works against the migration-era effective ownership surface:

```text
available_shares = live stXXX balance + legacy Position shares
amount_out = shares * accounted_balance / total_shares
```

Burn order:

1. Burn receipt balance first
2. Burn residual `Positions` only if needed

Native unstake is therefore an exit from liquid `stNTVE` value, not an exit from collator nomination. Collator nomination exits use the LP unlock lifecycle.

### 4. Native LP collator nomination

`lock_native_lp_for_collator(lp_asset_id, amount, operator)`:

1. Ensures `operator` is valid through `NativeOperatorValidator`
2. Ensures `lp_asset_id` is the canonical `NTVE/stNTVE` LP
3. Transfers LP from the user into `native_lp_lock_account()`
4. Updates `NativeLpLocks[(account, operator)]`
5. Updates `OperatorNativeLpLocked`, `AccountNativeLpLocked`, `AccountNativeCollatorLpLocked`, and `TotalNativeLpLocked`
6. Touches native reward snapshots when the native pool exists
7. Emits `NativeLpLocked`

During the current trusted-collator phase, session sets use the configured invulnerables directly. Collator-locked LP remains the authoritative backing surface for nomination rewards, NativeVotePower, and bounded valuation. If permissionless candidate ranking is enabled later, candidate ordering uses conservative collator-locked LP native-equivalent value; the removed native-binding compatibility path no longer affects candidate ordering.

### 5. Unlock and redelegation lifecycle

`request_unlock_native_lp(operator, amount)`:

- Requires no active governance lock horizon for the account
- Removes backing immediately from operator/account/collator aggregates
- Creates or updates `PendingNativeLpUnlocks[(account, operator)]`
- Touches native reward snapshots
- Emits `NativeLpUnlockRequested`

`withdraw_unlocked_native_lp(operator)` transfers the LP back after `NativeLpUnlockDelay`.

`redelegate_native_lp(from_operator, to_operator, amount)` moves locked LP between operators without releasing custody to the account. It validates the new operator and updates operator aggregates, while preserving account-level locked LP totals.

### 6. Governance-only native custody

`lock_native_lp_for_governance(lp_asset_id, amount)` locks canonical `NTVE/stNTVE` LP for NativeVotePower without collator nomination.

`lock_native_asset_for_governance(asset_id, amount)` locks either `$NTVE` or `stNTVE` for NativeVotePower.

Governance unlock requests are blocked while `NativeGovernanceLockProvider::lock_until(account)` is active. The runtime provider reads `pallet-governance::GovernanceLocks`.

### 7. Conservative native-equivalent LP valuation

Runtime valuation is centralized in `DelegationWeightedCollatorSessionManager::conservative_native_lp_value(locked_lp)`:

```text
native_equivalent = 2 * min(reserve_NTVE, reserve_stNTVE * staking_exchange_rate) * locked_lp / lp_supply
```

This value is used by:

- Permissionless candidate ranking through operator locked LP when that runtime phase is enabled
- Native nomination reward base through account collator-locked LP
- Governance NativeVotePower through aggregate account locked LP
- Read-model views through `RuntimeNativeStakingReadModelProvider`

The formula intentionally ignores optimistic value from an unbalanced pool side.

## Reward Architecture

### Native reward funding recognition

Native `$NTVE` reward funding is not detected by transfer-event scanning. Instead, epoch rollover calls:

```text
reconcile_reward_inflow(NativeStakingAssetId)
```

The reconciliation compares live `reward_account(NTVE)` balance against `RewardLiabilityBalances[NTVE]` and records only the positive unaccounted delta. Repeated reconciliation without new funding is idempotent.

### Bounded epoch rollover

`on_initialize` closes reward epochs through a persisted `PendingRewardEpochRollover` cursor. Each block processes at most `MaxRewardRolloverAssetsPerBlock` touched asset buckets from the source epoch, rolls their bounded account sets into the target epoch snapshot, and leaves the cursor in storage if more buckets remain. `LastProcessedRewardEpoch` advances only after the cursor is empty; native reward reconciliation also waits for that completion point so epoch close remains resumable instead of collecting all touched assets in one block.

### Reward snapshot base

Default non-native reward snapshots still use effective share balance.

Native `$NTVE` reward snapshots use runtime-provided reward base:

```text
native reward base = conservative_native_lp_value(AccountNativeCollatorLpLocked[account])
```

This excludes:

- Liquid `stNTVE`
- Standalone governance-only LP
- Locked `$NTVE` / `stNTVE` governance assets

Only collator-locked LP earns native nomination rewards.

### Reward touch sources

Native reward snapshots are touched by explicit state transitions:

- Native stake path
- Collator LP lock
- Collator LP unlock request
- Governance winning-vote record/eviction hook
- Bootstrap helper before denominator freeze

Native reward snapshots do not depend on:

- `$NTVE` reward-account transfer events
- `stNTVE` transfer events
- Governance event scanning

### Legacy non-native reward ingress

`RuntimeLegacyRewardSnapshotEventIngress` remains wired as compatibility support for non-native share-vault rewards. It scans current block events with bounded budget and emits truncation state when the scan cap is hit.

Native filters deliberately exclude:

- `$NTVE` reward-account inflows
- Native `stNTVE` receipt transfers
- Native governance-domain expansion

The caveat is operationally important: legacy event ingress only sees current block events via `System::read_events_no_consensus()`. It is not a historical event processor.

### Claim paths

`claim_reward(asset_id, epoch)` and `claim_reward_batch(asset_id, epochs)` remain the generic same-asset auto-compound paths for non-native staking assets. They move reward amount from `reward_account(asset_id)` into pool backing and mint fresh `stXXX` receipts. Passing the native staking asset is rejected so `$NTVE` nomination rewards cannot escape through the legacy auto-compound surface.

Native nomination rewards use separate entrypoints:

- `claim_nomination_reward(epoch)` pays liquid `$NTVE` to the caller
- `claim_nomination_reward_batch(epochs)` pays multiple closed native nomination reward epochs as liquid `$NTVE`
- `claim_and_compound_nomination_reward(epoch, operator)` claims liquid `$NTVE`, routes it through the runtime compound helper, mints `NTVE/stNTVE` LP, and locks the minted LP to `operator`

All native nomination claim paths share `RewardClaims[((NTVE, epoch), account)]`, so the same epoch cannot be claimed twice through different surfaces.

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

| Runtime adapter | Role |
| :-------------- | :--- |
| `RuntimeNativeOperatorValidator` | Accepts trusted invulnerables and enabled permissionless collator candidates |
| `RuntimeNativeStakingLpAssetValidator` | Validates canonical `NTVE/stNTVE` LP token |
| `RuntimeStakedAssetIdResolver` | Resolves base asset -> receipt asset id |
| `RuntimeStakedAssetLifecycle` | Creates receipt assets and metadata |
| `RuntimeRewardGovernanceDomainResolver` | Maps reward asset to governance domain |
| `RuntimeRewardEpochProvider` | Uses block number as reward epoch on the current line |
| `RuntimeRewardCoefficientProvider` | Reads governance reward-memory coefficient |
| `RuntimeRewardBaseWeightProvider` | Overrides native reward base to collator-locked LP value |
| `RuntimeNativeNominationRewardCompounder` | Routes claim+compound into Asset Conversion and LP locking |
| `RuntimeNativeStakingReadModelProvider` | Exposes native pool/LP valuation for bounded views |
| `RuntimeLegacyRewardSnapshotEventIngress` | Legacy non-native event scanner |
| `RuntimeNativeGovernanceLockProvider` | Reads governance lock horizon |

## AAA and Asset Conversion Integration

`pallet-aaa` remains tokenomics-agnostic. It exposes generic:

```text
Task::DonateLiquidity { asset_a, asset_b, amount, max_ratio_error }
```

The runtime-specific `TmctolLiquidityDonationOps` maps the `NTVE/stNTVE` pair to:

```text
AssetConversionAdapter::donate_native_staking_liquidity_from_ntve
```

That helper:

1. Reads current `NTVE/stNTVE` reserves and staking exchange rate
2. Computes the native stake-vs-donate split needed for balanced donation
3. Stakes the required `$NTVE` leg to mint `stNTVE`
4. Donates both legs directly into the pool account without minting LP
5. Rolls back transactionally on any failure

This is the protocol LP-farming path: System AAA strengthens existing LP holders by increasing reserves per LP token rather than minting claimable rewards.

The runtime also exposes:

```text
AssetConversionAdapter::compound_native_nomination_reward_to_locked_lp(account, operator, amount)
```

This is user-compound oriented: it stakes part of liquid `$NTVE`, adds balanced liquidity through Asset Conversion, receives newly minted LP, and locks that LP to the requested collator.

## Read-Model Classification

### Bounded authoritative on-chain projections

The current runtime exposes bounded view/query surfaces for:

- `native_staking_exchange_rate()`
- `native_staking_liquidity_pool()`
- `native_locked_lp_position(account)`
- `native_collator_lp_position(account, operator)`
- `native_governance_custody_position(account, asset_id)`
- `native_nomination_reward_claimable(epoch, account)`
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
- All holders sorted by locked LP or reward claimability

Do not move unbounded history or sorted dashboards into consensus state.

## Operational Watchpoints

### Native AMM availability

Native LP nomination, AAA donation, read-model valuation, and claim+compound require the canonical `NTVE/stNTVE` pool to exist and be non-empty. The local development preset now registers the native staking asset and `stNTVE` receipt at genesis and seeds the LP asset-id namespace, while `scripts/07-seed-web-client-state.sh` can create/fund the local `NTVE/stNTVE` pool after the chain starts.

### Production/operator NTVE/stNTVE bootstrap flow

Outside the local-dev preset, the canonical pool should be launched through an explicit operator/governance sequence rather than hidden genesis assumptions:

1. Register the local native staking asset with `pallet-staking` so the staking pool exists and the `stNTVE` receipt asset is initialized.
2. Ensure the Asset Conversion LP namespace is seeded into `TYPE_LP | 1` before creating the pool so the LP token cannot collide with local, staked, or foreign assets.
3. Create the canonical `AssetKind::Local(NTVE) / AssetKind::Local(stNTVE)` pool through the runtime/governance-approved pool-creation path.
4. Seed balanced initial liquidity from a designated bootstrap account; this mints the initial LP supply to that account and makes read-model valuation non-empty.
5. Run readiness checks before enabling dependent flows: `native_staking_liquidity_pool()` returns a pool, both reserves are non-zero, LP total issuance is non-zero, and `RuntimeNativeStakingLpAssetValidator` accepts the LP id. Operators can use `scripts/bootstrap-native-staking-local.sh check` as the local read-only readiness probe for this phase.
6. Activate the Native Staking LP Farmer System AAA only after the readiness checks pass; activation remains guarded by `activate_native_staking_lp_farming` so donation execution cannot start against a missing or empty pool.
7. If any step after pool creation fails, leave the actor inactive and treat remediation as an operator/governance action; do not silently fall back to liquid `stNTVE` balances or transfer-event-derived backing.

### Governance locks block vote-power withdrawal

Unlock requests for native governance custody and collator LP check the account governance lock horizon. A user may be unable to reduce NativeVotePower until the relevant proposal's enactment horizon expires.

### Native reward funding must happen before epoch reconciliation

Funding `reward_account(NTVE)` after the epoch rollover will be recognized in a later epoch. Tests intentionally fund before the transition that reconciles the reward account.

### Truncated legacy reward epochs are unclaimable

If legacy event scanning exceeds the bounded scan cap, the epoch is marked incomplete and claims for that epoch are rejected.

### Pre-fork storage baseline

This repository is still the forkable framework line. Storage versions are current baseline markers rather than deployed-chain migration history. Downstream live forks own explicit migrations once launched.

## Current Limitations and Remaining Work

- Legacy non-native reward event scanning still exists until non-native reward surfaces are migrated away from the older share-vault event model
- Architecture docs should be revisited after any future replacement of non-native reward ingress
- Browser staking surfaces are now wired for the bounded native views and first signed action paths, but product-grade onboarding copy and guided flows can still improve
- Runtime staking weights have been regenerated from the expanded benchmark set; production forks should rerun benchmarks on their target hardware and runtime profile before launch
