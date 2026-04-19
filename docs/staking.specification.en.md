# Staking Specification: Multi-Asset Share-Vault Pools

> Contract maps: [`staking.architecture.en.md`](./staking.architecture.en.md), [`governance.architecture.en.md`](./governance.architecture.en.md)
>
> This document defines the staking contract: the conceptual model, invariants, public capability surface, and extension direction. Implementation status, runtime bindings, migration state, and operator rollout details belong in the paired architecture docs.

---

## 0. Specification Maintenance Meta-Layer

This specification MUST stay at or below **720 lines** (formatting-preserving count), add new normative content only with equal-or-greater removal of obsolete content, state rules as positive executable behavior unless a negative safety-critical constraint is required, keep normative facts single-sourced with references instead of duplication, preserve mandatory blank-line separation above and below numbered headings, and ensure every line carries normative meaning, traceability, or required implementation context.

---

## 1. Purpose

The TMCTOL standard on DEOS requires a real staking contract rather than an ad-hoc weighting bridge.
The required contract is not a classic era/slashing NPoS design.
It is a **multi-asset share-vault staking system** where each registered asset owns its own staking pool and its own sovereign input channel.

The key product property is:

> If funds arrive at the sovereign account of asset `A`, they must become claimable by the stakers of `A` proportionally to stake ownership, without iterating over all stakers.

---

## 2. Canonical Model

For each registered staking asset `asset_id`, the pallet maintains:

- One deterministic `pool_account(asset_id)`
- One `PoolState(asset_id)`
- Many `Position(asset_id, account)` entries

Ownership is represented by **shares**, not by per-inflow writes.
Incoming funds increase the value of each share.
They do not trigger one storage write per staker.

---

## 3. Storage Surface

### 3.1 Pool

Each staking asset has exactly one pool:

```text
Pools[asset_id] = PoolState {
  total_shares,
  accounted_balance,
  active_staker_count,
}
```

Fields:

- `total_shares`: total outstanding pool shares for the asset
- `accounted_balance`: underlying asset amount already recognized by the pool accounting
- `active_staker_count`: number of non-zero positions

### 3.2 Position

Each staker position is just shares:

```text
Positions[(asset_id, account)] = StakePosition {
  shares,
}
```

This is the canonical efficient ownership representation.

---

## 4. Sovereign Input Channel

For each registered asset, the pallet derives a deterministic sovereign account:

```text
pool_account(asset_id) = PalletId + asset_id
```

Any funds transferred to that account in the same asset become pool backing.
This is the asset's input channel.

---

## 5. Distribution Rule

Incoming funds are distributed by **share-price appreciation**.

The pallet does **not** do this:

- Receive inflow
- Iterate every staker
- Update every account's balance or reward storage

Instead it does this:

- Pool backing increases
- `total_shares` stays constant
- Each share becomes worth more underlying

A staker's current claim is:

```text
stake_value(account, asset_id) = shares(account, asset_id) * accounted_balance(asset_id) / total_shares(asset_id)
```

A staker's ownership fraction is:

```text
stake_fraction(account, asset_id) = shares(account, asset_id) / total_shares(asset_id)
```

This gives efficient proportional ownership lookup without fan-out writes.

---

## 6. Lazy Pool Sync

Pool accounting is synchronized lazily.

For a registered asset:

```text
actual_balance = asset balance of pool_account(asset_id)
```

Then:

```text
if actual_balance > accounted_balance:
  inflow_delta = actual_balance - accounted_balance
  accounted_balance = actual_balance
```

This realizes external inflow distribution without touching positions.

A conforming implementation SHOULD prefer lazy sync on explicit touchpoints:

- `stake`
- `unstake`
- Explicit `sync_pool`

It must not require an every-block full scan of all staking pools.

---

## 7. Stake Math

### 7.1 First Stake into Empty Pool

If:

- `total_shares == 0`
- `accounted_balance == 0`

then:

```text
minted_shares = amount_in
```

This initializes the pool at price `1 share = 1 underlying unit`.

### 7.2 Stake into Non-Empty Pool

If the pool already has shares and backing:

```text
minted_shares = amount_in * total_shares / accounted_balance
```

Then:

```text
total_shares += minted_shares
accounted_balance += amount_in
```

The ownership unit MAY be represented directly by shares or by a transferable receipt whose supply tracks outstanding shares.
Any compatibility bridge between legacy share storage and transferable receipts MUST preserve the same economic ownership result.

---

## 8. Unstake Math

If a staker burns `shares_out`:

```text
amount_out = shares_out * accounted_balance / total_shares
```

Then:

```text
total_shares -= shares_out
accounted_balance -= amount_out
```

When ownership is tokenized, the burned ownership unit is the transferable staking receipt.
Any compatibility bridge MUST preserve unstake rights and claim-value equivalence while ownership sources are converging.
The underlying is transferred from `pool_account(asset_id)` back to the staker.

---

## 9. Critical Edge Case: Unowned Inflow Before First Share

The staking contract MUST reject the dangerous case:

- `total_shares == 0`
- `accounted_balance > 0`

This means assets entered the sovereign account before any staker owned pool shares.
If the first staker were allowed to mint against this state, they could capture unowned inflow for free.

Therefore the contract requires an explicit invalid-state recovery path:

- `recover_unowned_pool(asset_id, beneficiary)` may be called only when the pool has zero outstanding shares and zero active stakers
- It transfers the full current sovereign balance of the pool to `beneficiary`
- It resets `accounted_balance` to zero
- The pool then returns to a clean empty state and first stake becomes possible again

---

## 10. Public Capability Surface

A conforming implementation SHOULD expose a bounded public surface sufficient for:

- Governance-explicit staking-pool onboarding
- Lazy pool synchronization on touchpoints
- Stake entry and exit
- Explicit recovery of prefunded no-owner pool state
- Bounded reward bootstrap and claim settlement for the reward-inflow line
- Bounded read helpers for live ownership, exposure, receipt value, and known-epoch claim verification

Pool onboarding MUST remain explicit.
Registering an asset MUST NOT silently create a staking pool or another economic surface as an accidental side effect.
If governance wants a staking pool for a registered asset, it MUST opt in explicitly through a dedicated staking-onboarding path.

### 10.1 Read-model contract

The staking query contract MUST distinguish bounded canonical on-chain projections from indexed/materialized views.

`Canonical on-chain staking projections` SHOULD cover live position and settlement truth:

- Pool state, sovereign accounts, and current share-vault ownership/exposure reads
- Native binding state and current delegated/passive split
- Reward coefficient and known-epoch reward claimability
- Receipt identity and current receipt-backed value

`Indexed / materialized staking views` SHOULD carry the heavier surfaces:

- APY charts and long-range performance series
- Historical claim timelines and wallet PnL history
- Search/filter across many past epochs or pools
- Dashboard/leaderboard analytics beyond current bounded state

Reward-claim discovery across an open-ended historical horizon MUST remain an `Indexed / Materialized View` unless the contract first introduces an explicit bounded horizon, expiry rule, or another equally honest cap.

So the product contract is:

- `known epoch claim verification` -> canonical on-chain
- `which epochs should this wallet try next?` -> explicit indexed/materialized discovery

---

## 11. Non-Goals of This Specification Version

This specification version does **not** require:

- Slashing
- Era rewards
- Validator election
- Reward tokens separate from the staked asset
- Tree-based weighted sampler structures

Those belong to later evolution layers.

---

## 12. Why This Model

This model is preferred because it gives:

- O(1) inflow recognition per touched pool
- O(1) per-staker ownership lookup
- No per-inflow writes across all stakers
- Deterministic sovereign input channels per asset
- A clean bridge from generic economic staking to native security-specific evolution

This model is already useful on its own as a generic multi-asset staking substrate.

The important isolation rule is explicit:

- Multi-asset staking remains a generic economic substrate
- Only native `$NTVE` participates in the canonical collator/operator security path
- Non-native staking pools are economic-only and must not silently influence block production or operator weighting

---

## 13. Tokenized Share-Receipt Direction (`stXXX`)

The next target design tokenizes pool shares as standard yield-bearing staking receipts:

- Native and local staking receipts derive deterministically via `TYPE_STAKED = 0x5000_0000`
- Foreign staking receipts derive deterministically via the dedicated `TYPE_STAKED_FOREIGN = 0x6000_0000` namespace
- Staking receipt supply tracks outstanding pool shares
- Share price rises when pool backing grows while receipt supply stays constant
- `XXX / stXXX` pools remain technically composable, even though the protocol does not yet assign them a special economic role

This tokenization direction now applies to native, local, and foreign staking assets.
The dedicated foreign receipt namespace is a deliberate answer to the current 32-bit `[type:4 | index:28]` constraint: there is no free injective arithmetic remap that keeps both full-width local receipts and full-width foreign receipts inside the single `0x5...` staking namespace, so foreign receipt derivation uses its own local namespace rather than another naive nibble swap.

The preferred lifecycle is explicit creation at staking-pool onboarding time:

- `register_staking_asset(asset_id)` deterministically reserves/creates the corresponding `stXXX` receipt asset when the namespace is currently resolvable
- The pool sovereign account is the receipt asset admin/owner
- Deterministic receipt metadata is assigned at registration time
- The first user stake should mint supply, not create the asset class lazily

This keeps the staking surface governance-explicit, removes first-stake races, and lets wallets/indexers see the receipt asset before user funds arrive.

Ownership coherence remains part of the contract:

- Once `stXXX` becomes transferable, legacy per-account share storage can no longer remain the only source of unstake rights or stake valuation
- A conforming implementation MAY keep a bounded compatibility bridge from legacy ownership storage to receipt-balance-based ownership while preserving stake value, unstake rights, and reward eligibility
- During such a bridge, any compatibility-only counters or legacy ownership stores MUST NOT become the hidden economic source of truth once receipt ownership exists
- Runtime cleanup MUST NOT remove the compatibility bridge until the implementation can prove that no surviving legacy ownership depends on it

Rollout sequencing, operator acceptance checks, and shipped migration state belong in [`staking.architecture.en.md`](./staking.architecture.en.md).

---

## 14. Unified Native Staking Target (`$NTVE` -> `stNTVE`)

Native `$NTVE` is the special case.
The target design keeps **one pallet** (`pallet-staking`) and **one native staking receipt** (`stNTVE`).
It deliberately rejects both a separate `nomXXX` tier and a standalone `pallet-delegation` unless later evidence proves they are necessary.

The target native flow is:

- A user may not passively stake `$NTVE` without nominating a trusted collator
- Native stake entry must include an explicit collator choice
- Successful native staking mints transferable `stNTVE`
- `stNTVE` remains the only native staking receipt
- Transferred `stNTVE` is economically valid but must not continue contributing backing to the sender's old collator binding

So native staking combines two properties in one surface:

- `stNTVE` is a liquid yield-bearing receipt
- Collator backing must still reflect real skin in the game

That implies the core invariant for the native path:

> Effective operator backing must follow the account's **live `stNTVE` position** so transfers or unstakes cannot leave stale votes behind

The intended default consequence is:

- If `stNTVE` leaves an account, the corresponding collator backing must fall with it
- The received `stNTVE` remains passive until the recipient explicitly binds it to a trusted collator

### 14.1 Canonical Native User Surface

The target surface should be explicit rather than implicit.
The preferred contract is:

- `stake(asset_id, amount)` for non-native whitelisted assets only
- `stake_native(amount, operator)` for `$NTVE`
- `bind_native(operator)` for a holder of already-owned `stNTVE` to attach their live balance to a trusted collator
- `clear_native_binding()` only if the runtime still wants to permit voluntarily passive `stNTVE` after acquisition
- `unstake(asset_id, shares)` remains the generic exit path, including native receipt burn

A conforming implementation SHOULD expose equivalent operator-aware entry and rebinding paths so native stake cannot enter the security surface without an explicit operator choice.

The essential rule is that native entry and native rebinding are operator-aware actions, while receipt transfers remain normal asset transfers.

### 14.2 Canonical Native Storage Contract

The minimum target storage contract is:

- One generic pool per asset, exactly as in the share-vault model
- One native binding map `account -> operator` for accounts that actively back a collator with their current `stNTVE`
- No separate `nomXXX` receipt layer
- No second pallet-level custody layer for native nomination

`stNTVE` itself is the only bearer receipt.
The operator binding is metadata attached to the account, not a second asset.

### 14.3 Canonical Backing Formula

The preferred source of truth is read-derived live balance, not stale historical share assignment.

```text
effective_native_backing(operator)
  = Σ live_stNTVE_balance(account)
    for all accounts where native_binding(account) == operator
```

Where:

```text
live_stNTVE_balance(account) = pallet-assets balance of stNTVE held by account
```

This means:

- A transfer out lowers the sender's effective backing automatically
- An unstake lowers effective backing automatically because `stNTVE` is burned
- An inbound transfer does not create operator backing until the recipient explicitly binds

Because `pallet-assets` does not provide an especially rich per-transfer callback surface for this design, the default target should prefer **read-derived backing** over eager per-transfer cached accounting until benchmark evidence proves cached counters are necessary.

---

## 15. Architecture Boundary for Native Binding

Implementation status, trusted-collator-phase wiring, runtime target validation, operator-commission policy, and launch-line security posture belong in [`staking.architecture.en.md`](./staking.architecture.en.md), not in this contract document.

---

## 16. Future Reward Inflow Layer (`pallet-staking` + `pallet-governance`)

The next evolution line keeps **two pallets**, not one merged super-pallet:

- `pallet-staking` owns stake receipts, pool backing, reward inflow accounting, and reward settlement
- `pallet-governance` owns winning-vote memory, GovXP / SBT state, cumulative participation/authorship counters, and the exported reward coefficient

The architectural goal is to reward governance quality **without** corrupting the base share-vault invariant.

### 16.1 Dual Inflow Contract Per Staking Asset

For each staking asset, the future contract uses two sovereign input channels:

- `pool_account(asset_id)` = backing inflow channel
- `reward_account(asset_id)` = governance-conditioned reward inflow channel

Their semantics are intentionally different:

1. **Backing inflow**
   - always belongs to the pool as common backing
   - always increases `accounted_balance`
   - always raises `stXXX` share price when recognized
   - may continue to use lazy pool sync (`actual_balance - accounted_balance`) because backing distribution is path-independent

2. **Reward inflow**
   - never changes share price directly
   - must be attributed to a concrete reward epoch at ingress time
   - must not be recovered later through a lazy reward-balance delta, because reward distribution depends on epoch-scoped weights

This is the core invariant:

> Backing inflow changes receipt value for all share holders. Reward inflow changes only reward entitlements for accounts whose governance-weighted epoch share deserves it.

### 16.2 Canonical Reward Weight

For a reward epoch `e`, the intended effective weight is:

```text
reward_weight(account, asset_id, e)
  = staked_receipt_snapshot(account, asset_id, e)
    × governance_reward_coefficient(account, governance_domain(asset_id), e)
```

Where:

- `staked_receipt_snapshot` is the epoch snapshot of live `stXXX` ownership for that staking asset
- `governance_domain(asset_id)` maps the staking asset into the relevant governance domain
- `governance_reward_coefficient` is exported by `pallet-governance`, not recomputed ad hoc inside `pallet-staking`

The canonical weight contract is **one-epoch lag**:

- Stake / unstake / transfer changes during epoch `E` affect reward weight only from `E + 1`
- Governance-coefficient changes during epoch `E` affect reward weight only from `E + 1`

This keeps reward semantics stable and avoids same-epoch gaming.

A conforming implementation SHOULD realize this with bounded sparse snapshots or another equally honest bounded mechanism.
If a legacy ownership bridge still exists, reward-weight accounting MUST continue to include every ownership source that still carries valid unstake and claim rights until the bridge is retired.
Bootstrap or warm-up paths for already-live holders are allowed, but they MUST be explicit and MUST NOT rewrite an epoch whose reward denominator has already been fixed.

### 16.3 Winning-Vote Sliding Window in `pallet-governance`

The future governance-side coefficient should come from a bounded sliding window of **winning-vote counters**, not from raw vote extrinsic count and not from mere participation.

Canonical rules:

- A counted event is a vote whose account-side final position matches the final winning side of the resolved referendum / proposal
- One governance item may contribute at most one counted winning-vote point per account
- The lookback horizon is runtime-configured (e.g. `WinningVoteLookbackEpochs`)
- Per-epoch counted votes MUST be bounded by runtime configuration (e.g. `MaxWinningVotesCountedPerEpoch`)
- Governance storage for an account MUST be deleted once the sliding-window rolling sum falls to zero

A conforming governance implementation MUST realize that sliding-window coefficient through bounded per-account winning-memory, bounded item-scoped uniqueness inside the live horizon, and bounded expiry / zero-sum eviction.
The staking pallet should consume only the exported coefficient / snapshot surface; it should not hardcode a specific governance formula internally.

This gives the desired sparse-memory invariant:

> if the sum of all stored winning-vote counters for an account becomes zero, that account leaves governance reward memory entirely

### 16.4 Canonical Reward Claim Path

The preferred reward settlement path is not liquid payout.
It is **same-asset auto-compound into fresh `stXXX`**.

For a claimable reward amount `reward_out` in the same asset as the staking pool:

1. transfer `reward_out` from `reward_account(asset_id)` into `pool_account(asset_id)`
2. mint new pool shares against the pre-compound pool price
3. issue the corresponding `stXXX` directly to the claimant

Conceptually this is:

```text
claim_reward(asset_id, epochs...) = claim + immediate stake of the same asset
```

A conforming implementation SHOULD expose a bounded claim surface for one closed epoch and MAY expose a bounded batch-claim surface for multiple closed epochs.

So the canonical user-visible result is:

- Reward entitlement is consumed
- Pool backing increases by the claimed amount
- The account receives freshly minted `stXXX`
- No intermediate liquid payout surface is required for the baseline path

### 16.5 Runtime-as-Config Boundary

This evolution must remain generic at the pallet layer.
The concrete policy belongs in runtime configuration.

The runtime-as-config boundary SHOULD provide deterministic helpers or equivalent adapters for:

- Reward-account derivation
- Governance-domain resolution per staking asset
- Governance-coefficient export
- Reward-epoch source
- Reward-ingress attribution
- Bounded reward bootstrap when already-live holders need explicit initialization
- Bounded claim settlement under the same-asset auto-compound contract

Any alternative claim mode remains future work and MUST NOT silently replace the baseline same-asset auto-compound contract.

`pallet-governance` should expose runtime-configured bounds / formulas for surfaces such as:

- Winning-vote lookback length
- Per-epoch counted-vote cap
- Coefficient formula and caps
- GovXP / SBT contribution to the exported coefficient

This preserves forkability:

- The pallets remain reusable framework components
- Each runtime can tune the reward-politics model without rewriting staking core math
- Future ecosystems may replace the exact governance formula while keeping the same dual-inflow staking contract

---

_End of specification._
