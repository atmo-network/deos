# pallet-governance

`pallet-governance` is the DEOS bounded governance reward-memory kernel for the current TMCTOL standard.

The pallet source is now split into responsibility-scoped internal modules while keeping `lib.rs` as the FRAME macro surface:

- `lib.rs` — pallet shell: storage, events, errors, extrinsics, public views
- `reward_memory.rs` — rolling-window accounting, GovXP counters, reward-coefficient internals
- `proposal_resolution.rs` — tally, resolution-state, rejection logic
- `proposal_execution.rs` — payload authority, execution bookkeeping, finalized outcome helpers
- `epoch_service.rs` — epoch servicing, maturing proposals, pending enactment, expiry helpers

## SDK baseline

This pallet is maintained against the current DEOS `Polkadot SDK 2603 / node 1.22.3` line.
The 2603 upgrade did not require pallet-local semantic changes here; the relevant fallout landed in runtime/session/XCM integration surfaces rather than in `pallet-governance` core logic.

## Scope

The current kernel slice provides:

- Bounded winning-vote sliding windows per `(domain, account)`
- Runtime-configured lookback length and per-epoch vote cap
- Bounded item-scoped uniqueness memory across the live reward window, including one-time resolved-item ingress within that horizon
- Bounded active proposal lifecycle per domain via `submit_proposal(domain, item_id, proposer, cadence_mode, payload_kind, payload_hash)`, `resolve_proposal`, and `reject_proposal`
- Bounded signed ballot casting through `cast_vote(domain, item_id, vote)` plus vote-derived finalization through `resolve_proposal_from_votes(domain, item_id)`
- First live dual-mode `Veto` track backed by the well-known `$VETO` protocol asset balance + issuance surface, with the runtime creating the `$VETO` asset class at genesis using deterministic metadata
- Policy-aware admin early finalization through `force_resolve_proposal_from_votes(domain, item_id)`
- Runtime-configured ordinary proposal vote weight, veto threshold/power source, voting period, approval threshold, and minimum turnout for the current vote-derived resolution slice
- Runtime-queryable governance-domain policy declaration, backed by `template/runtime/src/configs/governance_config.rs`, that centralizes the current launch hierarchy (`$NTVE + $VETO` for protocol governance, `$BLDR + $NTVE` for the canonical tactical domain)
- Bounded automatic proposal finalization through epoch-keyed maturity buckets, with stale entries ignored and retry deferral on failed automatic settlement
- Sparse memory with zero-sum eviction once a window fully decays
- Epoch expiry buckets that touch only accounts whose old winning votes are due to expire
- Runtime-queryable reward coefficient derived from the live winning-vote window
- Runtime-queryable GovXP input counters derived from rolling winning memory plus cumulative total / winning participation totals and cumulative authored / successful-authored proposal totals
- Admin-only `record_winning_vote(domain, item_id, account)` plus bounded `record_winning_vote_batch(domain, item_id, accounts)` for low-level ingress, backed by non-origin `ingest_winning_vote_resolution*` helpers and a higher-level proposal-resolution lifecycle
- Transactional winner-ingress helpers so late cap / expiry-bucket failures do not leave partial reward memory behind
- Explicit proposal rejection reasons for the current narrow lifecycle (`AdminRejected`, `NoVotes`, `VoteTie`, `TurnoutBelowMinimum`, `ApprovalThresholdNotMet`)
- Public query helpers for bounded active proposal discovery, bounded recent-finalized proposal discovery, current weighted proposal tally, runtime-declared vote-power profile identity per live track, live resolution state, unified proposal status, and recent finalized proposal outcomes
- Bounded retention/expiry for finalized proposal outcomes instead of unbounded history growth
- Benchmark-derived runtime weights for the current `record_winning_vote*`, proposal lifecycle, ballot-casting, and expiry-servicing kernel slice

## Key rule

The exported staking reward coefficient still tracks **winning votes**, not raw vote extrinsic count and not mere participation.
One governance item should contribute at most one winning-vote point per account inside the bounded live tail, the same resolved `(domain, item_id)` should not be ingested twice while it remains inside the live reward window, and ballot strength should flow through the runtime-configured vote-weight provider surface rather than being hardcoded inside the pallet.

Separately, the pallet now also keeps cumulative GovXP input totals for:

- Total proposal participation
- Total winning-side participation
- Total authored proposals
- Total successful authored proposals

Those monotonic totals do not replace the sparse winning-vote tail; they complement it. The authored / successful-authored counters are exported on-chain so future proposer-quality policy can evolve without needing archive reconstruction, while the v1 runtime surface itself remains counters-first rather than shipping a separate live GovXP multiplier contract.

## Memory discipline rule

If an account's winning-vote rolling sum reaches zero after expiry processing, its storage entry is deleted.
The pallet should keep active governance reward memory sparse rather than retaining inert zero-state accounts forever.

## Runtime-as-Config rule

Lookback length, per-epoch cap, epoch source, vote-weight surface, voting period, approval threshold, minimum turnout, and the eventual proposal-resolution wiring belong in runtime configuration or higher-level governance integration rather than hardcoded pallet policy.

## Current launch-policy rule

For the current launch line, the bounded runtime policy is intentionally frozen to:

- Ordinary `Aye / Nay` ballots now store a bounded ballot-time epoch and use runtime-configured Declining Power on top of same-domain `Staking::stake_value(domain, account)`, so early and late ordinary votes on the same proposal can carry different effective weight
- The separate protection track still uses `Veto` plus `Pass` ballots, allows one ordinary vote plus one protection-track vote per account on the same referendum, lets a protection-track vote switch sides by replacing the account's prior `Veto`/`Pass` choice, applies the same ballot-time Declining Power kernel to protection-track `Veto` / `Pass` tallies, and stays open until the configured protection close
- Protocol / network governance backs that protection track with the well-known `$VETO` protocol asset
- `$BLDR` tactical governance backs that protection track with native `$NTVE` staking weight instead of the `$VETO` asset
- The emergency immediate-cancellation gate still checks raw live protection weight against raw total eligible protection supply and cancels only when `Veto` is **strictly greater** than the runtime threshold; if that immediate gate does not fire, the final protection gate first requires raw `Veto` turnout to reach the runtime `1%` dust floor and then stays fail-closed unless weighted protection-track `Pass` strictly outweighs weighted `Veto`
- Narrow admin recovery through `reject_proposal`, policy-aware `force_resolve_proposal_from_votes`, and `requeue_proposal_for_auto_finalization`
- Bounded recent finalized-outcome retention rather than permanent in-kernel archival history

## Non-goals of the current slice

The current kernel does not yet include:

- A richer class-aware domain-policy surface beyond the current bounded public/query contract, including future class families and execution-authority wiring
- Richer multi-track policy beyond the current protocol `$NTVE + $VETO` and `$BLDR + $NTVE` launch hierarchy
- Permanent proposal-history archival inside the kernel pallet
- Richer GovXP / identity policy beyond the current counters-first slice, including delegation, any later bounded multiplier policy, and broader SBT / reputation layers

See [`docs/governance.specification.en.md`](../../../docs/governance.specification.en.md) for the intended governance contract, [`docs/governance.architecture.en.md`](../../../docs/governance.architecture.en.md) for the current implementation map, plus [`docs/staking.specification.en.md`](../../../docs/staking.specification.en.md) and `BACKLOG.md` for the broader two-pallet reward trajectory.
