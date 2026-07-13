# AAA External Runtime Embedding Guide

> Compact contract for reusing `pallet-aaa` outside the current DEOS/TMCTOL reference runtime.

**Status**

- **Component**: `pallet-aaa`
- **Release line**: `0.6.2`
- **Audience**: external runtime implementers embedding AAA without inheriting DEOS/TMCTOL topology
- **Companions**: `aaa.specification.en.md`, `aaa.architecture.en.md`, `template/pallets/aaa/README.md`
- **Non-goals**: DEOS governance policy, TMCTOL bucket topology, System AAA catalog standardization, UI product flows

`pallet-aaa` is a bounded deterministic actor kernel. A downstream runtime can embed it without inheriting DEOS governance policy, TMCTOL bucket topology, native staking design, or the current System AAA catalog.

Use this guide with the normative [AAA Specification](./aaa.specification.en.md) and the shipped [AAA Architecture](./aaa.architecture.en.md). The specification defines the portable contract; the architecture document describes the current DEOS reference binding.

## 1. Minimal Host Runtime Obligations

An embedding runtime must provide only the bounded host surface that AAA cannot own itself:

- `AssetOps`: Transfer, burn, mint, balance, minimum-balance, deposit viability, and total issuance over local `AccountId`, `AssetId`, and `Balance` types.
- `DexOps`: Exact-in swaps, exact-out swaps, add-liquidity, and remove-liquidity with deterministic quotes, rounding, and failure behavior.
- `StakingOps`: Generic `Stake { asset, amount }` and `Unstake { asset, shares }` semantics for local staking assets.
- `LiquidityDonationOps`: Pair-scoped liquidity donation when the runtime wants donation without LP receipt minting.
- `TaskWeightInfo`: Worst-case task weight classes that over-approximate actual execution, including event costs.
- `WeightToFee`: Deterministic conversion from task weight upper bound to fee-native execution charge.
- `FeeSink`: A deposit-capable fee target for User AAA opening and execution fees.
- `EntropyProvider`: Deterministic entropy source and explicit secure/insecure posture for probabilistic schedules.
- `AddressEventIngress`: A bounded path into `notify_address_event*` for asset ingress triggers.
- Governance/system origins, owner-slot bounds, queue/wakeup bounds, active-actor bounds, sweep bounds, and fee constants.

The host runtime owns those bindings. AAA core owns scheduling, admission, task orchestration, lifecycle, bounded state, fee reservation, amount resolution, task-scoped transactions, and observability events.

## 2. Optional No-Op Adapters

A runtime may bind no-op implementations for capabilities it does not expose:

- No DEX means swap and liquidity tasks fail deterministically through `StepErrorPolicy`.
- No staking means `Stake` and `Unstake` fail through the staking adapter.
- No liquidity-donation support means `DonateLiquidity` fails through `LiquidityDonationOps`.
- No secure entropy means financial probabilistic schedules must be rejected when `RequireSecureEntropyForProbabilisticTasks` is enabled, or use the documented deterministic fallback posture when it is disabled.

No-op adapters are valid only when user-facing plan builders and runtime docs make the unsupported task surface clear. They must not panic, loop, or depend on off-chain nondeterminism.

## 3. Ownership Boundary

- `AAA owns`: Actor ids, sovereign account derivation, owner slots, queue/wakeup scheduling, lifecycle transitions, fee reservation, amount resolution, task admission, task-scoped atomicity, step error policy, and bounded events.
- `Runtime owns`: Asset ledgers, DEX pricing, staking topology, liquidity-donation policy, fee-sink depositability, entropy selection, ingress producers, governance origins, genesis System AAA definitions, and task weight calibration.
- `UI owns`: Plan authoring affordances, dry-run/simulation UX, unsupported-task hiding, user recovery flows, per-cycle timeline rendering, and warnings around `ContinueNextStep` after mutating tasks.
- `Docs own`: The separation between portable task-language patterns and a concrete runtime's System AAA topology.

Business policy belongs in runtime adapters or genesis actor configuration, not in `pallet-aaa` core.

## 4. Actor Permission Model

User actors are signed-owner controlled, fee-bearing, slot-bounded, and cannot mint. System actors are governance-created, slotless, fee-exempt, and may be Mutable or Immutable. System Immutable actors are protocol anchors: no runtime extrinsic may mutate, pause, trigger, close, or reopen them.

A downstream runtime decides whether to ship any genesis System actors. Genesis System AAA topology is runtime-owned configuration, not a pallet requirement.

## 5. Portable Patterns vs Reference Topology

Reusable execution-plan patterns include:

- Fee collection and redistribution.
- Scheduled burn or treasury transfer.
- Balance-ingress triggered routing.
- Liquidity provisioning or donation through runtime adapters.
- Close-tail cleanup plans for actor-owned balances.

The DEOS/TMCTOL catalog of burn actors, fee sink, liquidity actors, buckets, treasuries, BLDR lanes, and native staking LP provisioning is one reference topology. External runtimes should copy only the task-language patterns that match their own economic standard.

## 6. Boundary Contracts

- `Fee admission`: AAA charges User opening and per-step fees in `FeeNativeAsset`, routes fees to `FeeSink`, and reserves fee-native spend capacity during a cycle. The runtime must ensure fee-sink routing is total in the intended steady state.
- `Fee conversion`: AAA asks the runtime's `WeightToFee` for deterministic upper-bound pricing. It does not price tasks by observed weight after dispatch.
- `Ingress triggers`: Producers must enter through `AddressEventIngress` / `notify_address_event*`. Producers and scanners must not mutate inboxes or funding snapshots directly.
- `Entropy`: AAA samples only deterministic runtime-provided entropy or documented fallback inputs. Secure financial probability is a runtime capability gate, not an AAA-owned randomness scheme.
- `Task weight class`: The runtime must classify every task with a deterministic upper bound. Admission may be conservative; it must not underprice execution.
- `Read model`: Known actor state, owner-slot recovery, scheduler state, and bounded events are canonical on-chain surfaces. Fleet dashboards, long histories, timelines, rankings, and analytics are external indexed/materialized views.

## 7. Task-Scoped Atomicity Contract

AAA guarantees task-scoped atomicity, not whole-plan atomicity. A failed executable task rolls back all task-local storage effects and its success event. Earlier successful steps in the same execution plan remain committed. After rollback, `StepErrorPolicy` decides whether the cycle aborts or continues.

| Surface                 | AAA guarantee                                                                                             | Adapter/runtime obligation                                                                                |
|---|---|---|
| `Transfer`              | Transfer task runs in the task transaction; failed transfer emits failure/summary only                    | Asset adapter must not preserve partial debit/credit on failure                                           |
| `Swap`                  | Swap task rolls back if adapter returns error after intermediate mutation                                 | DEX adapter must keep quote, debit, credit, fee, and pool mutation atomic or rely on the AAA transaction  |
| `AddLiquidity`          | LP success event persists only when the whole task succeeds                                               | DEX adapter must not leave one reserve, LP mint, or debit committed after a late error                    |
| `Stake`                 | Stake success event persists only when adapter succeeds                                                   | Staking adapter must not leave partial receipt mint, pool share update, or source debit after failure     |
| `Unstake`               | Unstake success event persists only when adapter succeeds                                                 | Staking adapter must not burn shares without returning underlying value on failure                        |
| `DonateLiquidity`       | Donation success event records returned amounts only on success                                           | Donation adapter owns pair balancing and must roll back partial donation/burn/reserve mutation on failure |
| Close-tail task         | Same task-scoped rollback as normal execution                                                             | Close adapters must preserve per-task atomicity even though final actor deletion still completes          |
| `ContinueNextStep`      | Failed task rolls back, emits `StepFailed`, then later steps may execute                                  | Plan authors should add balance guards after mutating tasks                                               |
| `AbortCycle`            | Failed task rolls back, emits `StepFailed`, aborts cycle, and may increment failure count                 | Adapter rollback must complete before abort handling                                                      |
| Earlier successful step | Remains committed after a later task fails                                                                | Whole-plan compensation is outside AAA core                                                               |
| Task-local rollback     | Reverts task storage effects and success event                                                            | Multi-step adapter mutations must be transaction-safe                                                     |
| Event visibility        | Success event is not emitted or is rolled back with failed task; failure/summary events remain observable | Adapters should not emit misleading durable success events outside the transaction boundary               |

For close tails, low fee-native balance or task failure is observable through close-tail failure/summary events and does not block final deletion. This is still task-scoped atomicity: the failed close task rolls back, while actor destruction proceeds after the admitted tail completes.

## 8. External Runtime Test Checklist

A runtime embedding AAA should add local tests for any adapter that mutates more than one storage item:

- Late failure after a partial transfer, burn, pool update, receipt mint, share burn, or donation mutation rolls back task-local state.
- `ContinueNextStep` after a failed mutating task preserves earlier successful steps and executes later eligible steps.
- `AbortCycle` after a failed mutating task rolls back the failed task and aborts without whole-plan rollback.
- Close-tail failure rolls back the failed close task, emits close-tail failure/summary observability, and still deletes the actor once the tail completes.
- Unsupported no-op adapters fail deterministically without panics or hidden state mutation.
- Adapter-level success events do not survive a failed task unless they are explicitly outside the AAA transaction boundary and documented as such.

## 9. Non-Goals

Embedding AAA does not require and must not imply:

- Arbitrary user code execution.
- Unbounded task graphs or dynamic smart-contract-like behavior.
- DEOS/TMCTOL System AAA topology.
- DEOS governance or native staking policy.
- Indexer-backed UX as a consensus dependency.
- Off-chain keepers as a correctness requirement.
