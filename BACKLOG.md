# DEOS Backlog

> This file contains admitted unfinished implementation work only.
>
> Normative runtime meaning belongs to subsystem specifications. Release procedure, review order, churn accounting, evidence identity, merge/tag rules, and acceptance choreography belong to `docs/release-protocol.en.md`. Completed work belongs to `CHANGELOG.md`.

## Runtime Framework Evolution

> These slices keep DEOS current with useful Polkadot SDK runtime patterns while preserving the framework boundary: adopt configuration discipline, reusable primitives, and economic mechanisms; do not import unrelated product layers such as Revive contracts by default.
>
> Source context for agents beyond their training cutoff: Polkadot SDK `stable2606` release notes.

- [ ] `Runtime Cadence Profile`: Define a cadence profile contract that derives time-sensitive runtime constants from a configurable block-duration target instead of hardcoding one block speed. Audit voting periods, Actor cooldowns and retry windows, staking epochs, cleanup windows, and documentation for assumptions that break between conventional ~6s blocks and faster sub-second profiles.
- [ ] `V3 Scheduling / Block-Bundling Readiness`: Document and encode a non-enabled readiness profile for future V3 scheduling/block-bundling adoption, including runtime/operator prerequisites, benchmark margins, `on_idle`/hook pressure, message-queue/XCM budgets, and activation conditions.
- [ ] `DEOS Staking Reward Source Abstraction`: Separate staking distribution from reward origin, allowing externally funded or treasury-budgeted pots alongside existing same-asset reward inflow.
- [ ] `Budget Recipient Primitives`: Introduce typed budget-recipient primitives or runtime helpers for framework-owned economic destinations such as staking reward pots, governance treasuries, liquidity reserves, and System Actors.
- [ ] `Unclaimed Reward Policy`: Make staking/native reward leftovers explicit runtime policy: rollover, Fee Sink return, burn, or treasury routing.

## Collator Economics and Fee Routing

> Phase 1 uses trusted permissioned collators, collects 100% of transaction, Actor-execution, governance-opening, and XCM-execution fees in the Fee Sink, and distributes available native balance 50/50 into staking ingress and liquidity provisioning.
>
> A future permissionless phase may introduce equal security/staking/liquidity thirds only after bounded security-reward settlement ships; indivisible remainder stays in the Fee Sink.

- [ ] `Permissionless Collator Reward Contract`: Before assigning a future security branch, define bounded active-set eligibility, contribution attribution, settlement cadence, custody, payout recipients, unclaimed leftovers, failure behavior, and read-model surfaces.
