# pallet-staking

`pallet-staking` is the DEOS multi-asset share-vault staking pallet in the current reference runtime.

## SDK baseline

This pallet is maintained against the current DEOS `Polkadot SDK 2603 / node 1.22.3` line.
The 2603 upgrade did not require pallet-local semantic changes here; the relevant fallout landed in runtime/session/XCM integration surfaces rather than in `pallet-staking` core logic.

## Scope

The current core/runtime slice provides:

- Governance registration of staking assets
- No automatic staking-pool creation from asset-registration hooks
- One deterministic sovereign pool account per registered asset
- One deterministic future reward-account helper per registered asset (`reward_account_for(asset_id)`) for the planned governance-conditioned reward channel
- Per-asset pool state (`total_shares`, `accounted_balance`, `active_staker_count`)
- Per-account share positions
- Tokenized receipt mode for native/local (`0x5...`) and foreign (`0x6...`) staking assets
- Lazy `sync_pool` against actual sovereign balance
- `stake` and `unstake` over `pallet-assets` / `fungibles`
- Governance bootstrap of live reward snapshots through `bootstrap_reward_snapshot(asset_id, account)`
- Same-asset auto-compound reward settlement through `claim_reward(asset_id, epoch)` and bounded sweeping via `claim_reward_batch(asset_id, epochs)`
- Efficient ownership lookup through shares rather than per-inflow writes
- Liquid native `$NTVE -> stNTVE` staking through `stake_native(amount)` without operator binding
- Locked `NTVE/stNTVE` LP nomination lifecycle (`lock_native_lp_for_collator`, `request_unlock_native_lp`, `withdraw_unlocked_native_lp`, `redelegate_native_lp`)
- Per-operator commission configuration (`set_operator_commission`, `OperatorCommissions`, `MaxOperatorCommission`)
- Explicit native query helpers (`native_stake_value`, `passive_native_stake_value`, `delegated_native_stake_value`)

The currently intended future direction keeps the pallet generic and runtime-configurable while adding a second, governance-conditioned reward inflow channel per staking asset.

## Key rule

External inflow to a pool sovereign account is distributed by share-price appreciation, not by iterating all stakers.

Future reward inflow must remain separate from this rule: governance-conditioned reward inflow should use a dedicated reward account, epoch-scoped reward weights, and same-asset auto-compound claim semantics instead of changing share price directly. The current scaffold now includes `reward_account_for(asset_id)`, per-block aggregated reward-account ingress into `note_reward_inflow(asset_id, amount)` through a weight-accounted `on_idle` scan, sparse one-epoch-lag reward-weight snapshots driven by staking touches, `stXXX` transfer ingress, governance-memory events, an explicit governance bootstrap surface for already-live holders before an epoch's denominator is fixed, and concrete settlement paths through both `claim_reward(asset_id, epoch)` and bounded `claim_reward_batch(asset_id, epochs)`.

## Security isolation rule

Multi-asset staking is generic, but only native `$NTVE` participates in the canonical authoring security path for the current trusted invulnerable collator set.
Other staking assets are economic-only and must not silently affect block production or randomness security.

## Current edge-case rule

If a pool receives backing before any shares exist, the first staker must not be allowed to capture that unowned balance for free. The current kernel therefore rejects `stake` when:

- `total_shares == 0`
- `accounted_balance > 0`

The pallet now also provides explicit governance recovery:

- `recover_unowned_pool(asset_id, beneficiary)` drains the full unowned pool backing
- The pool returns to a clean empty state
- Normal first stake becomes possible again

## Native nomination rule

Native `$NTVE` staking mints liquid `stNTVE`; it does not bind liquid receipts to operators.
Collator nomination is represented by explicitly locked canonical `NTVE/stNTVE` LP, with targets validated against the trusted invulnerable collator set in the current runtime phase.
Permissionless collators stay inactive until a relay-beacon-backed design is ready.

## Runtime-as-Config rule

The pallet must stay generic.
Concrete reward-politics decisions such as governance-domain mapping, winning-vote lookback length, coefficient formula, and reward-claim mode belong in runtime configuration rather than in hardcoded pallet logic. The current helper surface already reflects this rule through runtime-resolved `reward_governance_domain(asset_id)`.

## Non-goals of the current slice

The current kernel does not yet include:

- Slashing
- Operator/delegator payout routing
- A stronger per-slot weighted author lottery inside a fixed authority set
- Advanced staking UX beyond the native security path

See [`docs/staking.specification.en.md`](../../../docs/staking.specification.en.md) for the contract and [`docs/staking.architecture.en.md`](../../../docs/staking.architecture.en.md) for the current implementation map.
