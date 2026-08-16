# DEOS Staking Specification: Yield-Bearing Native Liquidity and Governance Power

> Contract maps: [`architecture.en.md`](./architecture.en.md), [Governance architecture](../../governance/docs/architecture.en.md), [DEOS Router architecture](../../router/docs/architecture.en.md)
>
> This document defines the normative staking contract: the conceptual model, economic invariants, public capability surface, and governance-power semantics. Runtime wiring and operational constraints belong in the paired architecture documents.

---

## 1. Purpose

DEOS staking is not a classic era/slashing NPoS design. It is an economic coordination layer that must connect four protocol functions without relying on transferable-balance event tracking:

- Yield-bearing native liquid staking
- Zero-fee native receipt liquidity
- Collator nomination through locked useful liquidity
- Governance-conditioned rewards and voting power

The launch contract centers on `$NTVE` and its native staking receipt `stNTVE`.

The core product property is:

> `$NTVE` staking creates a yield-bearing `stNTVE` receipt; `NTVE/stNTVE` liquidity is strengthened by protocol donation; collator nomination is backed by locked `NTVE/stNTVE` LP; governance-conditioned nomination rewards are settled by bounded epoch accounting.

Generic liquid staking for arbitrary `XXX/stXXX` pairs is not part of this launch contract.

---

## 2. Canonical Launch Scope

The launch staking surface is native-first:

```text
NTVE -> stNTVE -> NTVE/stNTVE LP -> locked LP nomination -> governance-conditioned reward
```

The contract intentionally focuses on `NTVE/stNTVE` because this pair reinforces the DEOS Router and `$NTVE` value loop:

```text
native staking -> liquid receipt liquidity -> router volume -> route fees -> NTVE burn/value support
```

Future ecosystems MAY add non-native liquid-staking markets, but that is a separate opt-in extension. A future non-native model would have only two reward flows by default:

1. `XXX` staking yield through the `XXX` staking pool
2. `XXX/stXXX` LP farming through protocol donation

It would not participate in native collator nomination unless a future governance contract explicitly delegates such authority.

---

## 3. Reward Flows

The reference activation policy is owned by one runtime-code `NativeSecurityMode::{TrustedSet, LpBackedSelection}` decision:

- `TrustedSet` uses trusted permissioned collators, wires LP-donation funding through Actors #14, bridges staking-yield native-balance holdings into staking pool truth after donation execution, and preserves settlement of retained reward and custody obligations
- `LpBackedSelection` enables permissionless collators, explicit LP nomination, new retained snapshots/pots, certified funding, and bounded atomic compound

`LpBackedSelection` is an explicit runtime-upgrade boundary, not mutable storage or a launch-time governance parameter. One operation-availability classifier MUST derive session selection, candidate eligibility, new nomination, redelegation, certified funding, compound, and Trusted contraction from this mode owner; liquid claims and custody exits remain mode-independent. `TrustedSet` rejects every operation that creates a new LP-backed security obligation while preserving liquid settlement and custody exits. `native_security_view()` MUST own mode, readiness, current and Planned epoch identity, and retained-settlement presence. Readiness MUST classify inactive mode, native pool/receipt absence, canonical liquidity-pool absence or LP mismatch, zero reserves, zero LP issuance, unavailable valuation, inconsistent bounded indexes, absent positively backed candidate operators, duplicate candidate identity, and ready state without optimistic fallback. Each LP-backed planning attempt MUST overwrite one bounded diagnostic containing the planned `SecurityEpoch` and transition outcome; block hooks MUST NOT create diagnostic history or redefine it. A non-ready LP-backed session boundary MUST return no replacement set before ranking or candidate cleanup. Candidate order MUST use conservative native-equivalent locked LP descending and canonical account order ascending as the sole tie breaker; a candidate admission deposit MUST NOT establish operator eligibility, become security backing, or become a ranking input. Mode contraction MUST still allow nomination unlock requests and matured withdrawals so deactivation never traps custody.

Upstream collection sends 100% of transaction, Actors, governance-opening, and XCM-execution fees into Fee Sink without an immediate author split; DEOS Router trading fees remain on the Burn Actor path. The reference permissioned phase divides available native balance 50/50 between staking ingress and liquidity provisioning. LP-backed mode uses exact 34/33/33 integer shares for certified security funding, staking ingress, and liquidity provisioning; the security leg fails closed unless the current retained pot is open and coherent.

### 3.1 Staking yield

Staking yield belongs to the native staking pool:

```text
staking_yield -> pool_account(NTVE)
```

When recognized:

```text
accounted_balance(NTVE) increases
total_shares(NTVE) stays constant
stNTVE appreciates against NTVE
```

This reward is not separately claimable. It is received through the higher redemption value and market value of `stNTVE`.

### 3.2 LP farming through protocol donation

DEOS AMM pools have `0%` LP fee by default, so swap volume does not by itself accumulate fees into pool reserves.

LP farming for `NTVE/stNTVE` is therefore a protocol donation flow:

```text
Actors funding -> router/zap -> balanced donation into NTVE/stNTVE reserves
```

The intended result is:

```text
AMM reserves increase
LP total supply stays constant
LP token value increases
AMM price ratio stays within tolerance
```

This reward is not separately claimable. Existing LP holders receive it through appreciation of each LP token's underlying claim.

### 3.3 Governance-conditioned nomination reward

Nomination reward is the selective claimable `LpBackedSelection` flow. It belongs only to accounts that lock `NTVE/stNTVE` LP for collator nomination and maintain useful governance activity.

```text
locked_lp_native_value * governance_coefficient -> nomination_reward_weight
```

The LP-backed contract admits reward funding only through a typed certified Fee Sink operation into one retained `SecurityEpoch` pot. Unsolicited reward-account balance creates no pot, liability, accrual, or claim right. `TrustedSet` keeps new funding and compound inactive while preserving liquid claims against retained Finalized obligations; `LpBackedSelection` enables certified funding and bounded atomic compound.

---

## 4. Native Staking Pool

The native staking pool is the canonical source of intrinsic `stNTVE` value.

```text
pool_account(NTVE)
PoolState { total_shares, accounted_balance }
```

The exchange rate is:

```text
staking_exchange_rate = accounted_balance / total_shares
```

### 4.1 Stake

Native staking mints yield-bearing `stNTVE` shares.

For an empty pool:

```text
minted_shares = amount_in
```

For a non-empty pool:

```text
minted_shares = amount_in * total_shares / accounted_balance
```

The deposited `$NTVE` becomes pool backing and the staker receives `stNTVE`.

### 4.2 Unstake

Native unstaking burns `stNTVE` shares and redeems `$NTVE` backing.

```text
amount_out = shares_out * accounted_balance / total_shares
```

### 4.3 Transfer

`stNTVE` is a transferable yield-bearing receipt. A transfer of `stNTVE` changes only liquid receipt ownership.

It MUST NOT change:

- Collator backing
- Nomination reward eligibility
- Governance coefficient
- Frozen vote power
- Epoch reward snapshots

A transfer of a non-locked `NTVE/stNTVE` LP token changes only liquid LP ownership. It MUST NOT change collator backing, nomination reward eligibility, governance coefficient, frozen vote power, or epoch reward snapshots. Only locked LP positions carry those properties.

---

## 5. `NTVE/stNTVE` Zero-Fee AMM Pool

`NTVE/stNTVE` is the canonical launch liquidity pair for native liquid staking.

It provides:

- Instant liquid entry and exit
- Market price discovery
- Router routes between `$NTVE` and `stNTVE`
- The LP asset used by collator nomination

The staking pool defines intrinsic value:

```text
staking_price(stNTVE) = accounted_balance / total_shares
```

The AMM defines market price:

```text
xyk_price(stNTVE) = reserve_NTVE / reserve_stNTVE
```

The AMM price MAY diverge from intrinsic staking value. Router and arbitrage activity SHOULD pull the market price toward intrinsic value, but the AMM price is not staking truth.

---

## 6. LP Farming Donation

A protocol donation increases LP token value without minting new LP supply to the donor.

Let:

```text
reserve_NTVE = X
reserve_stNTVE = Y
lp_total_supply = L
```

A balanced donation satisfies:

```text
delta_NTVE / X = delta_stNTVE / Y
```

Then:

```text
reserve_NTVE increases
reserve_stNTVE increases
lp_total_supply stays constant
pool ratio stays constant
LP token value increases
```

### 6.1 No add/remove-liquidity farming

Ordinary `add_liquidity` mints LP tokens and therefore does not farm existing LP holders. Ordinary `remove_liquidity` burns LP and withdraws reserves.

LP farming donation MUST be realized as one of:

- Direct balanced transfer into the AMM pool account
- A runtime helper that donates reserves without minting LP tokens

It MUST NOT be modeled as `add_liquidity -> remove_liquidity`.

### 6.2 Actors donation actor

The donation actor may start with `$NTVE` funding only.

Baseline flow:

```text
Actors has NTVE
runtime computes the stake-vs-donate split from current reserves and staking exchange rate
stake the required NTVE side into stNTVE
donate balanced NTVE + stNTVE into AMM pool
```

The donation MUST be skipped or deferred when:

- The computed stake side exceeds available Actors `$NTVE` balance
- Current reserves would require a split outside the configured ratio tolerance
- The `NTVE/stNTVE` pool is not yet created or has zero reserves on either side

The donation operation SHOULD enforce configured ratio tolerance and emit a donation event suitable for wallets, analytics, and route-quality accounting. Swap or mixed-route acquisition MAY be added later only as an explicit policy extension when reserve divergence proves the deterministic stake-acquisition baseline insufficient.

---

## 7. Collator Nomination Through Locked LP

Collator nomination uses locked `NTVE/stNTVE` LP tokens, not locked `stNTVE`.

```text
lock_lp_for_collator(lp_asset_id, operator, lp_amount)
```

Locked LP:

- Backs a collator
- Creates nomination reward eligibility
- Retains exposure to staking yield through the `stNTVE` reserve side
- Receives LP farming through AMM donation
- Cannot be transferred until unlocked

Ordinary LP-token transfer MUST NOT affect collator backing or nomination reward eligibility.

The mutation surface is explicit and operator-scoped. One bounded active-participant index and one bounded per-account operator index make the complete session snapshot enumerable without scanning position keys. Admission of the first account position or a new operator position MUST fail before LP transfer when `MaxNativeSecurityParticipants` or `MaxNominationsPerAccount` is full; existing-position top-ups do not consume another slot.

- `lock_lp_for_collator(lp_asset_id, operator, lp_amount)`
- `request_unlock_lp(operator, lp_amount)`
- `withdraw_unlocked_lp(operator)`
- `redelegate_locked_lp(from_operator, to_operator, lp_amount)`

---

## 8. LP-Backed Collator Weight

Raw LP amount is not a stable backing unit. Collator backing SHOULD use a conservative native-equivalent value.

Preferred balanced value:

```text
balanced_pool_native_value =
  2 * min(
    reserve_NTVE,
    reserve_stNTVE * staking_exchange_rate
  )
```

Then:

```text
locked_lp_native_value(account) =
  locked_lp_amount(account) / lp_total_supply
  * balanced_pool_native_value
```

`staking_exchange_rate` is defined only when `total_shares > 0`. When the native staking pool is empty, no `stNTVE` exists, therefore no `NTVE/stNTVE` LP can be created and this formula is never evaluated against an empty staking pool.

Using `min` prevents excess on one side of a skewed pool from inflating backing power. The weight rewards useful two-sided liquidity rather than raw reserve size.

Reward and governance accounting SHOULD use epoch snapshots of this value rather than live per-block recalculation.

---

## 9. NativeVotePower

`NativeVotePower` is the normalized governance unit for native economic exposure. It is not a token. It is a frozen value computed from explicitly locked positions.

The launch sources are:

- Locked `$NTVE`
- Locked `stNTVE`
- Locked `NTVE/stNTVE` LP
- LP already locked for collator nomination and additionally used for governance

Liquid balances do not vote by default. A position must be explicitly locked or already locked in an eligible lock surface before it can produce `NativeVotePower`.

### 9.1 Source formulas

Locked `$NTVE`:

```text
power = locked_NTVE * ntve_vote_multiplier
```

Locked `stNTVE`:

```text
power = locked_stNTVE
  * staking_exchange_rate_at_vote
  * stNTVE_vote_multiplier
```

Locked `NTVE/stNTVE` LP:

```text
power = locked_lp_amount / lp_total_supply_at_vote
  * 2 * min(
      reserve_NTVE_at_vote,
      reserve_stNTVE_at_vote * staking_exchange_rate_at_vote
    )
  * lp_vote_multiplier
```

Runtime policy MAY use multipliers or haircuts per source. The specification requires that all source conversions be explicit and deterministic.

### 9.2 No double counting

The same economic claim MUST NOT produce multiple simultaneous voting powers across source classes.

Therefore:

- `NTVE` deposited into staking no longer votes as liquid `$NTVE`
- `stNTVE` deposited into an LP no longer votes as standalone `stNTVE`
- LP locked for collator may be reused as governance power only through an explicit governance-use record
- Transferable balances outside a lock do not vote

An account MUST NOT simultaneously hold the same token units in both a governance lock and an LP position. Governance custody withdrawal is blocked while the aggregate `lock_until` horizon is active, which naturally prevents double counting: token units must first exit governance custody before they can be deposited into an LP.

---

## 10. Governance Lock Contract

Governance uses an aggregate account-level lock, not per-referendum locks.

A conforming contract SHOULD model:

```text
GovernanceLock(account) {
  locked_sources,
  total_native_vote_power,
  lock_until,
}
```

When the account votes with new sources:

1. Convert selected balances into `NativeVotePower` using current rates/reserves
2. Freeze that `NativeVotePower` for the vote being cast
3. Lock or mark the selected positions as governance-used
4. Extend `lock_until` to the referendum's enactment horizon if it is later

```text
lock_until = max(current_lock_until, referendum_enactment_end)
```

The lock may cover multiple referenda. There is no separate unlock ledger per referendum in the baseline contract.

### 10.1 Frozen vote records

Each vote stores the power used at cast time:

```text
Vote(referendum_id, account) {
  vote_side,
  native_vote_power,
}
```

Later changes to `stNTVE` exchange rate, AMM reserves, LP farming donations, or staking yield MUST NOT change already cast vote power.

This protects governance outcomes from non-voting economic state changes.

### 10.2 Collator-locked LP used for governance

LP already locked for collator nomination may be used as governance power without transferring it into a second custody layer.

The governance lock MUST extend the effective unlock horizon:

```text
effective_unlock = max(collator_unlock_epoch, governance_lock_until)
```

This preserves both obligations: collator nomination and referendum voting.

---

## 11. Governance-Conditioned Nomination Rewards

A `SecurityEpoch` uses locked LP value and governance activity. `SecurityEpoch` is an exact alias of the host session owner's `SessionIndex`; block-number cadence and maintenance progress cannot change it. At a ready LP-backed planning boundary, Staking atomically retains one complete Planned snapshot containing the future epoch, bounded participants, eligible operators, conservative values, governance coefficients, account reward weights, and total denominator. The current Open snapshot and funding pot remain unchanged until `start_session(epoch)` atomically promotes the plan and finalizes the prior pot. Any validation, bound, valuation, arithmetic, or activation failure leaves prior active state intact.

Eligibility requires:

- Locked `NTVE/stNTVE` LP
- A collator target
- A positive governance coefficient for the epoch

Weight:

```text
nomination_reward_weight(account, epoch) =
  locked_lp_native_value_snapshot(account, epoch)
  * governance_coefficient(account, epoch)
```

The governance coefficient is exported by governance logic. Staking MUST NOT hardcode the formula.

Epoch-lag rule:

- LP locks created in epoch `E` affect reward from `E + 1`
- Governance activity in epoch `E` affects reward from `E + 1`
- Unlock requests immediately remove active collator backing and future nomination-reward weight, while custody withdrawal remains delayed until the configured unlock block
- Already-finalized epoch snapshots and claim rights remain unchanged by later unlock requests

---

## 12. Nomination Reward Funding

Native nomination reward funding uses one typed certified operation for the current open `SecurityEpoch`. The pallet call `fund_native_security_reward(amount)` requires `SecurityRewardFundingOrigin`, derives the current session identity internally, transfers `$NTVE` only from `SecurityRewardFundingSource`, and atomically increases the retained pot and exact outstanding liability. The runtime Fee Sink transfer adapter uses the same source-checked preflight and certification boundary when LP-backed allocation is active. Funding fails before mutation when mode, epoch identity, retained pot state, source authority, amount, or liability arithmetic is invalid.

Planning creates at most one Planned zero-credit pot. In `LpBackedSelection`, session start promotes it to Open and finalizes the prior Open pot. On transition to `TrustedSet`, session start finalizes the retained Open pot without changing credit or liability, removes the unactivated zero-credit Planned snapshot/pot, and clears active identity; resulting Finalized claims and expiry rights remain live. Reopening an epoch, planning while an older plan exists, planning while an overdue epoch remains, or growing retained pots beyond `SecurityRewardClaimHorizon + current + one planned` is rejected. A Planned future pot cannot receive funding, and planning it cannot redirect funding from the current Open epoch. The deterministic reward account is custody, not an inference surface: unsolicited balance creates no pot, liability, accrual, claim, carry-forward, or funding event. Funding depends on neither block event replay, `stNTVE` transfer tracking, LP transfer tracking, nor a holder scan.

---

## 13. Nomination Reward Settlement

Claims consume only Finalized retained session pots and frozen account weights in either security mode. `claim_native_security_reward(epoch)` and bounded `claim_native_security_reward_batch(epochs)` share duplicate-first prevalidation, horizon checks, account eligibility, floor allocation, exact claimed-total/liability accounting, transactional native payout, and claim markers. Future, Planned, Open, missing, expired, zero-weight, zero-pot, duplicate-epoch, duplicate-claim, or ineligible-account requests fail without mutation.

A liquid claim pays `$NTVE`. `claim_and_compound_native_security_reward(epoch, operator, min_lp_out)` remains one transaction from claim consumption through deterministic stake/liquidity composition, `stNTVE` minting, canonical LP minting, measured caller minimum output, and lock to an explicit validated operator. Runtime ratio/debit protection bounds pool movement; stale state, invalid operators, insufficient output, or intermediate failure rolls back every economic and claim effect.

`SecurityRewardClaimHorizon` is measured in sessions and enforced during claim and expiry admission. At each session start, runtime progression considers the oldest overdue epoch, which is exactly the epoch crossing the horizon during ordinary progression. It invokes the same settlement transition available through permissionless `expire_native_security_reward(epoch)` recovery. A failed boundary cancels its unactivated zero-credit plan, blocks newer planning while the overdue obligation remains, and retries the oldest due epoch at the next boundary.

Settlement works in either security mode. It atomically proves reward custody excluding its persistent ED anchor covers exact liability, transfers `credited - claimed` including rounding dust plus any uncredited excess to Fee Sink, reduces liability only by the accounted epoch remainder, verifies retained custody equals the remaining liability, clears at most `MaxNativeSecurityParticipants` claim markers, and removes the retained snapshot and pot. No intermediate expired state exists, and repeated expiry cannot transfer again.

---

## 14. Public Capability Surface

A conforming launch implementation SHOULD expose bounded capabilities for:

### 14.1 Native liquid staking

- `stake(NativeStakingAssetId, amount)`
- `unstake(NativeStakingAssetId, shares)`
- `sync_pool(NativeStakingAssetId)`

### 14.2 Native AMM and donation support

- Governance-controlled initialization of `NTVE/stNTVE`
- A bounded runtime or actor donation path that computes the stake-vs-donate split and donates without minting LP to the donor
- A public quote surface MAY be added when direct user-facing donation becomes a product flow; Actors-only donation does not require a separate public quote call

### 14.3 Collator LP nomination

- `lock_lp_for_collator(lp_asset_id, operator, lp_amount)`
- `request_unlock_lp(operator, lp_amount)`
- `withdraw_unlocked_lp(operator)`
- `redelegate_locked_lp(from_operator, to_operator, lp_amount)`

### 14.4 Governance voting

- `lock_and_vote(referendum_id, vote, selected_sources)`
- `vote_with_existing_lock(referendum_id, vote, native_vote_power)`
- `extend_lock_and_vote(referendum_id, vote, additional_sources)`
- `unlock_governance()` once `lock_until` has passed

### 14.5 Nomination rewards

- A certified funding operation for the current open `SecurityEpoch`
- `claim_nomination_reward(epoch)` after retained-pot finalization
- `claim_nomination_reward_batch(epochs)` over the same implementation
- `claim_and_compound_native_security_reward(epoch, operator, min_lp_out)` as a bounded atomic extension

---

## 15. Read-Model Contract

The staking query contract MUST distinguish bounded canonical on-chain projections from indexed/materialized views.

Canonical on-chain projections SHOULD cover:

- Native pool state and `stNTVE` receipt identity
- Current staking exchange rate and redeem estimate
- Current `NTVE/stNTVE` reserves
- LP token identity and total supply
- Current LP native-equivalent estimate
- Locked LP nomination state
- Operator locked LP and backing estimate
- Governance lock state and frozen vote power
- Retained `SecurityEpoch` reward status and account claimability once settlement ships

Indexed / materialized views SHOULD cover:

- Historical staking exchange-rate charts
- Historical LP donation / LP farming APY
- AMM discount / premium history
- Router volume and burn impact
- Long-range nomination reward history
- Wallet PnL
- Operator leaderboards beyond current bounded state
- Search across expired `SecurityEpoch` values

---

## 16. Bounded Maintenance Contract

The native staking path MUST NOT be an event-stream orchestrator.

The native launch contract removes the need for:

- `stNTVE` transfer/mint/burn event ingress
- LP token transfer event ingress
- Reward-account transfer event ingress
- Cache repair based on transferable balances
- Per-block reward touch scanning

Generic non-native share-vault yield remains represented by receipt appreciation from pool backing. The pallet exposes no current-block reward event-ingress contract: it does not scan reward-account inflows, receipt transfers, or governance events for generic claimable rewards. Native nomination rewards likewise MUST NOT depend on event replay.

Native security work is bounded and session-oriented:

- Lazy native pool sync occurs only on explicit touchpoints
- Atomic LP value, eligibility, coefficient, and denominator planning precedes session-start activation
- Certified funding, claims, and atomic expiry consume retained session pots under the exact horizon plus current and at-most-one-planned bound

Block-number hooks MUST NOT open, promote, fund, claim, expire, or finalize a `SecurityEpoch`. Expiry is permissionless, bounded by `MaxNativeSecurityParticipants`, and does not redefine `SecurityEpoch` progression.

---

## 17. Invariants

### 17.1 Yield-bearing receipt

```text
stNTVE represents native staking shares and may appreciate against NTVE
```

### 17.2 AMM truth boundary

```text
staking exchange rate is intrinsic value; AMM price is market value
```

### 17.3 Zero-fee LP farming

```text
NTVE/stNTVE trades do not grow LP value through LP fees
```

### 17.4 Donation farming

```text
LP farming increases AMM reserves without increasing LP total supply
```

### 17.5 Ratio preservation

```text
balanced donation must not move AMM price beyond configured tolerance
```

### 17.6 Security primitive

```text
collator backing depends on locked NTVE/stNTVE LP, not stNTVE balance
```

### 17.7 Governance power freeze

```text
NativeVotePower is computed at lock/vote time and frozen for that vote
```

### 17.8 Aggregate lock

```text
governance unlock time is the maximum enactment horizon of votes using the lock
```

### 17.9 Transfer isolation

```text
transfer(stNTVE) and transfer(LP_NTVE_stNTVE) do not affect security, reward, or frozen voting state
```

### 17.10 Flow separation

```text
staking yield -> staking pool
LP farming -> AMM donation
nomination reward -> epoch claimable side channel
```

---

## 18. Non-Goals

This launch specification does not require:

- Slashing
- Era rewards
- Validator election
- Generic liquid staking for every `XXX/stXXX` pair
- LP-fee accumulation in AMM pools
- Per-referendum source-specific lock ledgers
- Dynamic vote-power recalculation after a vote is cast
- Raw LP-token voting without native-equivalent normalization
- Full holder scans for reward or governance accounting

---

## 19. Why This Model

This model is preferred because it aligns the economic roles of the native stack:

- Staking yield strengthens `stNTVE`
- Actors donation strengthens zero-fee `NTVE/stNTVE` liquidity
- Locked LP strengthens collator backing
- Governance activity gates selective nomination rewards
- Router usage and route fees reinforce the `$NTVE` burn/value loop

The important simplification is explicit:

> Transferable `stNTVE` and transferable LP tokens are liquid economic assets, not hidden governance or security triggers. Security, voting power, and rewards arise only from explicit locks and epoch snapshots.

---

_End of specification._
