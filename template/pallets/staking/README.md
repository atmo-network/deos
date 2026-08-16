# pallet-deos-staking

`pallet-deos-staking` is the DEOS multi-asset share-vault staking pallet in the current reference runtime.

## SDK baseline

This pallet is maintained against the current DEOS `Polkadot SDK 2606 / node 1.24.0` line.
The 2606 upgrade did not require pallet-local semantic changes here; the relevant fallout landed in runtime/parachain-system/asset-conversion integration surfaces rather than in `pallet-staking` core logic.

## Scope

The current core/runtime slice provides:

- Governance registration of staking assets
- No automatic staking-pool creation from asset-registration hooks
- One deterministic sovereign pool account per registered asset
- Per-asset pool state (`total_shares`, `accounted_balance`)
- Per-account share positions
- Tokenized receipt mode for native/local (`0x5...`) and foreign (`0x6...`) staking assets
- Lazy `sync_pool` against actual sovereign balance
- `stake` and `unstake` over `pallet-assets` / `fungibles`
- No generic reward identity, rollover cursor, reward-account ingress, bootstrap snapshot, or claim surface
- Retained atomic session-native security snapshots and one certified Fee Sink funding path with exact pot/liability accounting
- Efficient ownership lookup through shares rather than per-inflow writes
- Liquid native `$NTVE -> stNTVE` staking through generic `stake(NativeStakingAssetId, amount)` without operator binding
- Locked `NTVE/stNTVE` LP nomination lifecycle (`lock_native_lp_for_collator`, `request_unlock_native_lp`, `withdraw_unlocked_native_lp`, `redelegate_native_lp`)
- One generic `stake_value(asset_id, account)` query over transferable receipt ownership, without synthetic passive/delegated exposure

The native security-reward channel owns certified funding, retained session pots, exact liabilities, mode-independent liquid/batch claims, atomic claim-and-compound into canonical locked LP, duplicate markers, and session-owned atomic bounded expiry return and state removal with permissionless recovery.

## Key rule

External inflow to a pool sovereign account is distributed by share-price appreciation, not by iterating all stakers.

Future native security rewards must remain separate from this rule. The shipped baseline freezes bounded LP value, eligible operators, governance coefficients, account weights, and a total denominator atomically at a session boundary; it exposes no block-based rollover or generic non-native claim engine.

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
Concrete governance participation policy belongs in runtime configuration rather than hardcoded pallet logic. Staking consumes one runtime-provided coefficient only while opening a native security snapshot.

## Non-goals of the current slice

The current kernel does not yet include:

- Slashing
- Operator/delegator payout routing
- A stronger per-slot weighted author lottery inside a fixed authority set
- Advanced staking UX beyond the native security path

See [`docs/specification.en.md`](./docs/specification.en.md) for the contract, [`docs/architecture.en.md`](./docs/architecture.en.md) for the current implementation map, and [`docs/embedding.md`](./docs/embedding.md) for host-runtime obligations.
