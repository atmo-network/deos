# pallet-aaa

`pallet-aaa` is the DEOS deterministic account-abstraction actor pallet.

## SDK baseline

This pallet is maintained against the current DEOS `Polkadot SDK 2606 / node 1.24.0` line.
The 2606 upgrade did not require pallet-local semantic changes here; the relevant fallout landed in runtime/parachain-system/asset-conversion integration surfaces rather than in `pallet-aaa` core logic.

## Scope

The current kernel/runtime slice provides:

- User and system AAA creation with deterministic sovereign accounts
- Bounded execution plans over adapter-driven tasks (`Transfer`, `Swap`, `AddLiquidity`, `Stake`, `Unstake`, `DonateLiquidity`, etc.)
- Monotonic paged FIFO scheduler state (`QueueHead`, `QueueTail`, bounded `QueuePages`) plus time-ordered wakeup storage
- Timer, manual, and `OnAddressEvent` triggers, where matched asset ingress can function as a trigger-message
- Bounded `on_idle` execution with sparse Healthy/Starving/Alerted state and one-time detection/recovery events
- Fee admission, lifecycle controls, pause/resume, and pure prechecked terminal cleanup
- Runtime-configured adapters for assets, DEX, staking, liquidity donation, fee collection, direct ingress, and weights
- Genesis provisioning of System actors through runtime configuration

## Key rule

AAA is a **bounded deterministic actor runtime**, not a general-purpose smart-contract VM.
Actors execute declarative plans against runtime adapters under explicit queue, scheduler, fee, weight, and lifecycle limits. Event-driven triggers such as matched asset ingress are one important part of that model, but they live alongside deterministic scheduling and bounded execution rather than replacing them.

## Reconfiguration rule

Within the existing task and adapter language, a large class of protocol changes should be expressed by reconfiguring actors, triggers, and graphs of asset flows rather than by rewriting the runtime.
Runtime upgrades are reserved for extending primitives, adapter surfaces, or safety invariants.

## Scheduler rule

Readiness and execution must stay deterministic and bounded:

- Future eligibility goes through the wakeup layer rather than ad hoc scans
- Hot-path execution happens only under configured per-block limits
- Timer readiness uses deterministic cadence and bounded actor-stable jitter; AAA exposes no probability or entropy contract
- `on_idle` does useful work only with remaining block budget

## Runtime-as-Config rule

The pallet must stay generic.
Concrete chain policy belongs in runtime configuration, including:

- `AssetOps`, `DexOps`, `StakingOps`, and `LiquidityDonationOps`
- Fee conversion, fee collection, and task weight classes
- Ingress hooks and genesis System AAA topology
- Governance/system origins and operational bounds

## External runtime embedding checklist

A runtime can reuse `pallet-aaa` without adopting the full DEOS/TMCTOL topology by providing the bounded configuration surface only. The full host-runtime contract lives in [`docs/aaa.embedding.en.md`](../../../docs/aaa.embedding.en.md). Executable portability evidence lives in [`template/aaa-embedding-runtime`](../../aaa-embedding-runtime/README.md); that fixture is not a second product or a normative topology.

Minimal checklist:

- Implement asset, optional domain, fee-collection, direct-ingress, benchmarking, and task-weight adapters for local runtime types.
- Bind governance/system origins, owner-slot limits, queue/wakeup bounds, fee constants, task weight classes, and native asset identity.
- Decide which tasks are allowed for User vs System actors and keep any chain-specific policy in adapters or genesis actor configuration, not in pallet core.
- Provide deterministic genesis System AAA definitions only for actor roles the runtime actually wants to ship.
- Treat example execution plans as reusable task-language patterns; treat the DEOS/TMCTOL System AAA catalog as one runtime's topology, not as the pallet's required deployment shape.
- Validate adapter failure atomicity with runtime-local tests when adapters perform multi-step mutations.

## Non-goals of the current slice

The current kernel does not yet include:

- Arbitrary user code execution
- Hidden off-chain nondeterminism as a correctness dependency
- Unbounded task graphs or unmetered loops
- Direct pallet-specific business logic embedded into AAA core

See `docs/aaa.architecture.en.md` and `docs/aaa.specification.en.md` for the current contract.
