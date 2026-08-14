# DEOS Router Specification

## Status and Ownership

This document defines the intended reusable contract of `pallet-deos-router`. Code and tests own executable conformance. Package architecture documents describe the shipped implementation. Concrete DEOS adapters, accounts, indices, parameters, and cross-pallet composition belong in integration documentation.

The Cargo package identity is `pallet-deos-router`. The Rust crate and runtime pallet identity remain `pallet_deos_router`. This pre-launch line retains that identity explicitly and introduces no alias, duplicate pallet, migration shim, or partial rename.

## Purpose

DEOS Router prepares and executes one bounded economically honest route across current runtime liquidity. It compares viable candidates by recipient outcome, applies caller-authored protection, executes atomically, and reports the same route truth through events and outcomes.

The Router is a decision and execution mechanism. It does not own treasury policy, System Actor reference policy, market-quality policy, external fair-price claims, arbitrary graph search, solver competition, or historical analytics.

## Normative Vocabulary

| Term | Meaning |
| --- | --- |
| `Intent` | Exact-input or exact-output request with its caller-authored protection bound. |
| `Projection` | Non-mutating quote from one identified runtime state; never executable authority. |
| `PreparedRoute` | Bounded current-state route value admitted inside the execution transaction. |
| `Leg` | One actual market operation: XYK swap or direct TMC mint. |
| `XYK leg` | One executed pool swap with ordered input and output assets. |
| `Route family` | Direct XYK, direct TMC mint, or Native-anchored XYK. |
| `Outcome` | Bounded factual result returned and emitted after committed execution. |
| `Reference check` | Local Oracle deviation check applied to one actual XYK leg. |
| `Weight class` | Measured execution envelope selected from the prepared route family and intent. |

## Mandatory Invariants

- One `PreparedRoute` owns route identity from transactional preparation through protection, execution, Oracle publication, event emission, outcome return, and Weight classification.
- Route and leg collections use compile-time or runtime-bounded types. No consensus path accepts an unbounded `Vec`.
- External quote bytes never authorize execution. Execution prepares again from current state inside its rollback boundary.
- Exact-input selection maximizes recipient output after the caller-aware Router fee.
- Exact-output selection minimizes total caller input, including the Router fee.
- Candidate enumeration order cannot affect the selected route.
- Caller protection applies to the full economic intent, not to synthetic per-leg bounds.
- Reference checks and Oracle publication apply only to actual XYK legs.
- Direct TMC mint publishes no XYK observation.
- Fee routing, publication, Actor ingress, market mutation, balance changes, Router events, and outcome construction commit or roll back together.
- Event and outcome route identity, amounts, fees, legs, and Weight class remain identical.
- Unknown adapter or execution failures classify as Permanent at the Actor boundary.

## Supported Assets and Route Families

`AssetKind` is the shared asset identity. `NativeAsset` is host configuration. A route contains at most two legs under the current contract.

| Family | Legs | Admission |
| --- | --- | --- |
| `DirectXyk` | One XYK leg `from -> to` | Canonical pool exists and the intent is quotable. |
| `DirectMint` | One TMC mint leg `collateral -> token` | Curve exists, collateral is supported, and the intent is exact-input. |
| `NativeAnchoredXyk` | Two XYK legs `from -> Native -> to` | Neither endpoint is Native, both canonical pools exist, and the intent is quotable. |

No other route family exists. The Router rejects repeated adjacent assets, identical request endpoints, more than two legs, and a Native-anchored route whose endpoint is Native.

## Canonical Route Types

The public ABI may use equivalent generic and bounded aliases, but it must preserve this semantic shape and field order.

```rust
pub enum SwapIntent<Balance> {
  ExactInput { amount_in: Balance, min_amount_out: Balance },
  ExactOutput { amount_out: Balance, max_total_amount_in: Balance },
}

pub enum PreparedLeg<PoolId, Balance> {
  Xyk {
    pool_id: PoolId,
    asset_in: AssetKind,
    asset_out: AssetKind,
    quoted_amount_in: Balance,
    quoted_amount_out: Balance,
  },
  TmcMint {
    token_asset: AssetKind,
    collateral_asset: AssetKind,
    quoted_collateral_in: Balance,
    quoted_recipient_out: Balance,
  },
}

pub enum RouteFamily {
  DirectXyk,
  DirectMint,
  NativeAnchoredXyk,
}

pub struct PreparedRoute<Legs, Balance> {
  family: RouteFamily,
  legs: Legs,
  total_amount_in: Balance,
  router_fee: Balance,
  routed_amount_in: Balance,
  recipient_amount_out: Balance,
  weight_class: RouteWeightClass,
}
```

`Legs` is bounded to `MaxRouteLegs = 2`. Pool identity uses the canonical ordered representation supplied by the host adapter. Informational price-impact fields may appear only on quote projections; they do not enter prepared identity, selection, protection, or execution.

## Canonical Outcome

```rust
pub struct RouterOutcome<Legs, Balance> {
  family: RouteFamily,
  legs: Legs,
  total_amount_in: Balance,
  router_fee: Balance,
  routed_amount_in: Balance,
  recipient_amount_out: Balance,
  weight_class: RouteWeightClass,
}
```

For exact-input, `total_amount_in` equals the authored input. For exact-output, it equals actual caller spend and cannot exceed `max_total_amount_in`. `recipient_amount_out` is the recipient balance delta and must meet the authored output bound.

`routed_amount_in = total_amount_in - router_fee`. `router_fee` is zero for a fee-exempt caller. No field changes meaning between route families, intents, quote projection, event, runtime API, or Actor consumption.

## Quote Projection

Quote projection accepts the caller, assets, and one typed intent. It enumerates only supported families and returns the selected projected route plus informational metadata. It does not mutate state, publish Oracle observations, route fees, reserve liquidity, or promise later execution.

Each quote binds to one state identity suitable for the consumer surface, such as finalized block hash and runtime code identity. A consumer must label quote truth as current-state canonical projection rather than committed execution.

Exact-output projection excludes direct TMC mint until the TMC contract can guarantee an exact recipient output under a total collateral ceiling. The Router must not emulate exact-output mint through over-minting, refunds, synthetic redemption, or iterative guesses.

## Transactional Preparation

Execution starts one storage transaction before any fee, publication, ingress, market, balance, event, or Router storage mutation.

Preparation performs these steps in order:

- Validate endpoints, amount, deadline, and intent bounds.
- Derive the caller-aware Router fee policy.
- Enumerate supported candidates from current pools and curves.
- Quote every actual leg from current state.
- Construct a bounded `PreparedRoute` for each viable candidate.
- Select one route using the canonical comparator.
- Validate the full authored intent against the selected route.
- Validate each actual XYK leg against its directional reference when available.
- Preflight every fallible fee, publication, ingress, market, balance, and event-dependent condition that the adapter contract exposes.

No execution step may rediscover a different route. If current state changes within an adapter in a way that invalidates the prepared route, execution fails and rolls back rather than replanning.

## Deterministic Comparator

Candidate comparison is total and independent of insertion order.

Exact-input candidates compare by this tuple:

```text
recipient_amount_out descending,
router_fee ascending,
route_family_rank ascending,
canonical_leg_identity lexicographically ascending
```

Exact-output candidates compare by this tuple:

```text
total_amount_in ascending,
recipient_amount_out descending,
route_family_rank ascending,
canonical_leg_identity lexicographically ascending
```

Family rank is `DirectXyk = 0`, `DirectMint = 1`, `NativeAnchoredXyk = 2`. The rank resolves economic ties only; it never overrides a better recipient outcome. Canonical leg identity compares family, canonical pool or curve identity, ordered assets, and leg position without using storage insertion order.

Permutation tests must prove the same selected route for every ordering of the same candidate set.

## Intent Protection

Exact-input protection requires:

```text
actual recipient_amount_out >= min_amount_out
```

Exact-output protection requires:

```text
actual recipient_amount_out >= amount_out
actual total_amount_in <= max_total_amount_in
```

The exact-output ceiling covers Router fee plus all market input. The Router cannot present post-fee input, first-leg input, or quoted input as total caller spend.

Per-leg `min_amount_out` or `max_amount_in` values may exist only as mechanically derived execution guards that ensure the prepared full-intent bound. They are not independent public policy and cannot weaken the authored bound.

## Reference Checks

The Router applies a reference check to each actual XYK leg using that leg's ordered `asset_in`, `asset_out`, quoted input, and quoted output. Missing or uninitialized references follow the host's explicitly documented Router baseline; they never synthesize a fair price.

Direct TMC mint receives no XYK reference check. Native-anchored execution checks both actual pools independently. System Actor reference-deviation policy remains in the Actor adapter and may reject a Router request before execution; the Router does not absorb or reinterpret that policy.

## Oracle Publication

Immediately before each XYK leg mutates its pool, the Router derives the pre-execution directional spot sample from that exact pool and publishes it for `asset_in -> asset_out`.

Publication order equals execution order. Direct XYK publishes one observation. Native-anchored XYK publishes first-leg then second-leg observations. Direct TMC mint publishes none.

A failure before, during, or after any publication rolls back every publication revision, dirty-Actor ingress effect, market mutation, fee transfer, balance delta, Router event, and Router storage mutation in the transaction.

## Execution

Execution consumes the selected `PreparedRoute` without route rediscovery.

- Route the caller-aware fee once at ingress.
- Execute legs in canonical route order.
- Pass only bounded path or leg values to adapters.
- Measure actual caller spend and recipient balance delta.
- Verify the actual values against the authored intent and prepared route envelope.
- Construct one `RouterOutcome` from actual committed facts.
- Emit events whose route and economic fields equal the outcome.

User preservation policy and fee-exempt System account policy remain explicit host inputs. A route cannot recursively tax the Router, Fee Sink, Burn Actor, or configured exempt accounts.

## Atomic Rollback Matrix

The conformance suite must inject failure at every row and prove no partial state survives.

| Failure point | Required result |
| --- | --- |
| Fee preflight or transfer | No publication, market mutation, ingress, balance delta, or Router event. |
| First XYK publication | No fee, publication revision, pool mutation, ingress, balance delta, or event. |
| First XYK execution | No fee, publication, pool mutation, ingress, balance delta, or event. |
| Second XYK publication | First-leg publication and mutation also roll back. |
| Second XYK execution | Both publications and first-leg mutation also roll back. |
| TMC distribution | Fee, mint, distribution, ingress, balances, and events roll back. |
| Actor ingress | Fee, publication, market mutation, balances, and events roll back. |
| Actual-bound verification | Every prior effect rolls back. |
| Event-dependent finalization | Every prior effect rolls back. |

Tests must compare exact pre/post storage and balances for Router, Oracle, Actors, pools, TMC, fee recipient, caller, and recipient.

## Events

The canonical success event carries the complete `RouterOutcome`, either directly or through fields with identical order and meaning. A separate fee event may remain only when its asset, amount, source, and collector equal the outcome and committed transfer.

Governance fee updates emit old and new fee values. Failed execution emits no success or fee event. Runtime-level failure surfaces may report dispatch failure without persisting Router state.

## Failure Taxonomy

Router failures expose stable classes for direct callers and Actor mapping.

| Class | Examples | Actor mapping |
| --- | --- | --- |
| `InvalidRequest` | Identical assets, zero amount, invalid bound, deadline passed, unsupported intent. | Permanent |
| `NoViableRoute` | Missing pool or curve, unsupported collateral, unquotable family. | Temporary only when current market availability can change; otherwise Permanent by typed reason. |
| `ProtectionRejected` | Output floor, total-input ceiling, or local reference deviation exceeded. | Temporary |
| `LiquidityUnavailable` | Insufficient reserves, arithmetic domain, pool execution rejection. | Temporary when state-dependent. |
| `FeeRejected` | Fee preflight, preservation, or routing failure. | Temporary only for state-dependent funding; configuration failures are Permanent. |
| `PublicationRejected` | Oracle admission, capacity, producer, or publication failure. | Temporary only for explicitly recoverable availability; configuration failures are Permanent. |
| `IngressRejected` | Certified Actor ingress preflight or notification failure. | Typed recoverable failures may be Temporary; unknown is Permanent. |
| `InvariantViolation` | Prepared/executed mismatch, impossible leg, outcome mismatch, unknown adapter failure. | Permanent |

The public error enum may retain finer variants. Every variant maps exhaustively to one class. New variants fail compilation or conformance until classified. Unknown errors remain Permanent.

Router execution preserves one internal typed value until the signed-dispatch boundary:

```rust
pub enum RetryDisposition { Permanent, RetryLater }

pub struct AdapterFailure {
    dispatch_error: DispatchError,
    failure_class: FailureClass,
    retry_disposition: RetryDisposition,
}

pub enum ExecutionError<T> {
    Router(Error<T>),
    Adapter(AdapterFailure),
}
```

`ExecutionError::failure_class()` and `ExecutionError::retry_disposition()` delegate to the concrete Router variant or adapter value independently. Conversion to `DispatchError` occurs only in the signed extrinsic; pallet-to-pallet callers receive `ExecutionError`. Unknown external failures construct `AdapterFailure` with `InvariantViolation` and `Permanent`, never a temporary wildcard.

Quote-time and execution-time adapter failures use the same cause constructors. An adapter must select a typed cause before returning across the Router host boundary; Router and Actors do not reconstruct retry policy from an erased `DispatchError`.

## Route Weight Classes

```rust
pub enum RouteWeightClass {
  ExactInputDirectXyk,
  ExactInputDirectMint,
  ExactInputNativeAnchoredXyk,
  ExactOutputDirectXyk,
  ExactOutputNativeAnchoredXyk,
}
```

Each class measures worst-case preparation, protection, publication, ingress, execution, verification, event, and rollback-relevant storage access for that supported shape. No average-case quote count or insertion order may select Weight.

Actor admission uses the maximum Router Weight class reachable by the authored swap task and intent. The Actor package does not quote a route to choose a cheaper class before execution.

## Adapter Contracts

`AssetConversionApi` exposes canonical pool identity, current reserves, exact-input and exact-output one-pool quotes, and execution of one identified pool leg returning actual spend and output. Low-level helpers are package-private unless their names explicitly state single-pool primitive semantics.

`TmcInterface` exposes curve existence, collateral support, exact-input recipient quote, and mint execution returning actual recipient output. It does not expose redemption or exact-output mint.

`PriceOracle` exposes directional current reference validation and publication for one actual XYK leg. It does not own route selection or broad market truth.

`FeeRoutingAdapter` exposes preflight when the host can provide it and one transactional fee transfer. `AddressEventIngress` remains the certified host boundary for any balance movement that claims Actor ingress.

Every fallible adapter method that can participate in quote or execution returns `AdapterFailure` rather than bare `DispatchError`. The adapter selects the concrete boundary and retry disposition from host truth; state-dependent absence/capacity may return `RetryLater`, configuration/invariant failures return `Permanent`, and unrecognized host errors fail closed as Permanent. Read-only absence represented by `Option` remains valid only where absence itself has one unambiguous policy.

## Storage Contract

Router consensus storage remains bounded to:

| Storage | Contract |
| --- | --- |
| `RouterFee` | One fee rate bounded by `MaxRouterFee`. |
| `LpPairByTokenId` | Bounded reverse index to one canonical ordered pool pair. |

Package `try_state` proves the fee bound, canonical pair ordering, and one-to-one LP-token/pair ownership visible in Router storage. Runtime integration checks own existence and exact agreement with the host pool registry; the package does not claim cross-pallet proof. The Router stores no route cache, quote cache, unbounded path, execution history, or Oracle history.

## Public Calls and Runtime APIs

The signed swap call preserves its existing call index while evolving to return or emit canonical outcome truth. Governance fee update preserves its existing call index and bounded origin contract.

Pallet-facing execution APIs accept caller, recipient, assets, and typed intent. Exact-input and exact-output share preparation, comparator, protection, execution, and outcome owners rather than parallel implementations.

Bounded quote runtime APIs return the current projection shape and state identity. Archive, search, volume, route-quality history, and longitudinal analytics remain materialized-provider work.

## Compatibility and Upgrade Contract

The pre-launch contract retains the `pallet_deos_router` Rust crate and runtime pallet identity, pallet index, call indices, and storage prefixes. `PreparedRoute` is a public Rust conformance type for deterministic package tooling but remains absent from calls, events, storage, and runtime APIs. Public semantic changes land as one coherent ABI before launch; no deprecated alias or dual event/API surface is added.

A downstream launched chain owns migrations and monotonic runtime-version changes. This repository's pre-launch baseline may reset storage versions and generated metadata coherently.

## Conformance Vectors

Generated vectors cover every supported family and intent, including:

- Direct XYK exact-input and exact-output.
- Direct TMC exact-input.
- Native-anchored exact-input and exact-output.
- Fee-exempt and fee-paying callers.
- Output-floor and total-input-ceiling boundaries at below, equal, and above values.
- Economic ties and complete candidate-order permutations.
- One- and two-leg reference checks.
- Exact Oracle publication set and order.
- Actual spend and recipient-delta outcomes.
- Every public error variant and failure class.
- Every `RouteWeightClass`.

Vectors bind specification hash, runtime metadata identity, route type encoding, and generated Weight identity.

## Adversarial Corpus

The deterministic corpus covers stale projections, candidate permutations, first and later publication failures, Actor ingress rejection, fee failure, direct XYK failure, TMC failure, Native-anchored first and second leg failure, exact-input recipient delta, exact-output total spend, and prepared/executed mismatch rejection.

Every case states pre-state, request, prepared route or preparation failure, injected fault, expected error class, expected events, expected publications, expected balances, expected storage, and expected Weight class.

## Cross-Domain Equality

For every successful case, conformance proves:

```text
prepared route
= protected route
= executed route
= event route
= outcome route
= Weight class route
```

A quote projection may equal the later prepared route only when its bound state identity still matches. Quote equality is evidence, never an execution precondition or authority.

## Validation Gates

- Package unit tests cover route construction, total comparison, bounds, protection, taxonomy, outcomes, storage invariants, and rollback.
- External runtime tests prove the package needs no DEOS-specific account or policy.
- DEOS runtime tests cover concrete pools, TMC, Oracle, Actors, fees, XCM-facing asset identities, and transaction rollback.
- Benchmarks measure RefTime and ProofSize for every route Weight class at worst-case bounded occupancy.
- Metadata and client checks reject stale route types, errors, events, outcomes, and Weight identities.
- Workspace Clippy passes for all targets with warnings denied.

## Non-Goals

- Arbitrary graph routing or unrestricted path length.
- External DEX aggregation, solver competition, CoW settlement, or intent marketplaces.
- New liquidity mechanisms or route families.
- TMC redemption or synthetic exact-output minting.
- External fair-price guarantees, ordering protection, or MEV immunity.
- Router-owned history, analytics, policy scoring, or System Actor market policy.
- Compatibility aliases for a partial rename.
