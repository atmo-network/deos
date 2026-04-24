# Staking Specification: Yield-Bearing Native Liquidity and Governance Power

> Contract maps: [`staking.architecture.en.md`](./staking.architecture.en.md), [`governance.architecture.en.md`](./governance.architecture.en.md), [`axial-router.architecture.en.md`](./axial-router.architecture.en.md)
>
> This document defines the target staking contract: the conceptual model, economic invariants, public capability surface, and governance-power semantics. Implementation status, migration state, runtime wiring, and operator rollout details belong in the paired architecture documents.

---

## 0. Specification Maintenance Meta-Layer

This specification MUST stay at or below **720 lines**. New normative content SHOULD replace obsolete content instead of expanding the document indefinitely. State rules as executable positive behavior, keep implementation status out of the specification, and single-source shipped realization details in architecture documents.

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

The contract intentionally focuses on `NTVE/stNTVE` because this pair reinforces the axial-router and `$NTVE` value loop:

```text
native staking -> liquid receipt liquidity -> router volume -> route fees -> NTVE burn/value support
```

Future ecosystems MAY add non-native liquid-staking markets, but that is a separate opt-in extension. A future non-native model would have only two reward flows by default:

1. `XXX` staking yield through the `XXX` staking pool
2. `XXX/stXXX` LP farming through protocol donation

It would not participate in native collator nomination unless a future governance contract explicitly delegates such authority.

---

## 3. Three Reward Flows

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
AAA funding -> router/zap -> balanced donation into NTVE/stNTVE reserves
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

Nomination reward is the selective claimable flow. It belongs only to accounts that lock `NTVE/stNTVE` LP for collator nomination and maintain useful governance activity.

```text
locked_lp_native_value * governance_coefficient -> nomination_reward_weight
```

Reward funding stays outside the staking pool until epoch settlement:

```text
reward_account(NTVE) -> epoch pot -> claim_nomination_reward
```

A conforming implementation MAY expose a compound path that turns claimed nomination reward into more `NTVE/stNTVE` LP and locks it to an explicit collator target chosen at claim time. The launch contract intentionally keeps nomination rewards account-scoped rather than per-collator-scoped so compound settlement does not require historical per-operator attribution.

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

### 6.2 AAA donation actor

The donation actor may start with `$NTVE` funding only.

Baseline flow:

```text
AAA has NTVE
runtime computes the stake-vs-donate split from current reserves and staking exchange rate
stake the required NTVE side into stNTVE
donate balanced NTVE + stNTVE into AMM pool
```

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

The mutation surface is explicit and operator-scoped so one account may maintain bounded independent collator positions without hidden global nomination state:

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

A nomination reward epoch uses locked LP value and governance activity.

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

Reward funding is recognized by epoch balance reconciliation, not by per-block event ingress.

At epoch boundary:

```text
new_nomination_reward_inflow(epoch) =
  live_balance(reward_account(NTVE))
  - accounted_nomination_reward_liability(NTVE)
```

A positive delta becomes that epoch's reward pot.

This gives the desired boundedness:

- No transfer/deposit event tracking for `reward_account(NTVE)`
- No dependency on block event replay
- No dependency on `stNTVE` or LP transfers
- No full LP-holder scan

---

## 13. Nomination Reward Settlement

The baseline claim path MAY be simple liquid payout:

```text
claim_nomination_reward(epoch) -> receive NTVE
```

The preferred extension is compound settlement:

```text
claim_nomination_reward(epoch)
  -> deterministic stake/zap into NTVE + stNTVE
  -> add/mint LP position
  -> lock resulting LP to an explicit collator target
```

A conforming implementation MAY expose:

```text
claim_and_compound_nomination_reward(epoch, operator)
```

The `operator` argument is explicit because nomination rewards are account-scoped in the launch contract. The compound path MUST validate the target like ordinary LP nomination and MUST NOT infer historical per-collator reward ownership from expired snapshots.

This extension reinforces the native loop:

```text
governance activity -> nomination reward -> more liquidity -> stronger locked LP backing
```

---

## 14. Public Capability Surface

A conforming launch implementation SHOULD expose bounded capabilities for:

### 14.1 Native liquid staking

- `stake_native(amount)`
- `unstake(NativeStakingAssetId, shares)` or an equivalent `unstake_native(shares)` alias
- `sync_pool(NativeStakingAssetId)` or an equivalent `sync_native_pool()` alias

### 14.2 Native AMM and donation support

- Governance-controlled initialization of `NTVE/stNTVE`
- A bounded runtime or actor donation path that computes the stake-vs-donate split and donates without minting LP to the donor
- A public quote surface MAY be added when direct user-facing donation becomes a product flow; AAA-only donation does not require a separate public quote call

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

- `claim_nomination_reward(epoch)`
- `claim_nomination_reward_batch(epochs)`
- `claim_and_compound_nomination_reward(epoch, operator)` as an extension

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
- Known-epoch nomination reward claimability

Indexed / materialized views SHOULD cover:

- Historical staking exchange-rate charts
- Historical LP donation / LP farming APY
- AMM discount / premium history
- Router volume and burn impact
- Long-range nomination reward history
- Wallet PnL
- Operator leaderboards beyond current bounded state
- Search across expired reward epochs

---

## 16. Bounded Maintenance Contract

The staking system SHOULD NOT be an event-stream orchestrator.

The launch contract removes the need for:

- `stNTVE` transfer/mint/burn event ingress
- LP token transfer event ingress
- Reward-account transfer event ingress
- Cache repair based on transferable balances
- Per-block reward touch scanning

Remaining maintenance SHOULD be bounded and epoch-oriented:

- Lazy native pool sync on explicit touchpoints
- Epoch-close nomination reward recognition
- Bounded LP value snapshot finalization
- Bounded nomination denominator finalization
- Bounded claim expiry / cleanup

`on_idle` SHOULD perform at most one maintenance class per pass and resume unfinished epoch work before starting lower-priority cleanup.

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
- AAA donation strengthens zero-fee `NTVE/stNTVE` liquidity
- Locked LP strengthens collator backing
- Governance activity gates selective nomination rewards
- Router usage and route fees reinforce the `$NTVE` burn/value loop

The important simplification is explicit:

> Transferable `stNTVE` and transferable LP tokens are liquid economic assets, not hidden governance or security triggers. Security, voting power, and rewards arise only from explicit locks and epoch snapshots.

---

_End of specification._
