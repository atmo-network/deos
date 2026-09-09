# Builder Economy Contract

- **Component**: DEOS reference Builder economy
- **Status**: Durable reference-economy contract
- **Audience**: economic designers, governance contributors, runtime integrators, client authors, and downstream fork teams

This document is the canonical entry point for the DEOS reference Builder economy. It composes the project-independent TMCTOL standard with DEOS Governance into one optional second-order economic domain without moving Builder-specific policy into either dependency.

## 1. Authority and Dependencies

This contract owns the cross-system composition of `$BLDR`, Builder governance, Builder invoices, BLDR Anchor, BLDR Treasury, and the parent-capital bridge.

Its two primary normative dependencies are:

- [`tmctol.specification.en.md`](./tmctol.specification.en.md), which owns TMC pricing, mint conservation, TOL reserve mathematics, XYK floor analysis, burn compression, and floor-reporting preconditions.
- [`template/pallets/governance/docs/specification.en.md`](../template/pallets/governance/docs/specification.en.md), which owns governance domains, primary and protection tracks, vote power, lifecycle, invoice resolution, payout execution, boundedness, and observability.

Supporting ownership remains separate:

- [`framework-instance.contract.en.md`](./framework-instance.contract.en.md) owns the mechanism-versus-instance-policy boundary.
- [`core.architecture.en.md`](./core.architecture.en.md) and [`actors.integration.en.md`](./actors.integration.en.md) describe the shipped DEOS composition and any implementation gap from this contract.
- [`../simulator/`](../simulator/) owns executable project-independent TMCTOL formulas, conservation checks, floor/compression scenarios, and parameter hypotheses; it does not model the Builder composition or its runtime execution.
- Runtime code and tests own executable behavior; Wiki pages are explanatory projections rather than normative owners.

If this document conflicts with TMCTOL mathematics, the TMCTOL specification wins for the standard mechanism. If it conflicts with governance lifecycle or vote semantics, the Governance specification wins. This document decides only how those two contracts compose for the reference Builder domain.

## 2. Purpose

The Builder economy is an optional L2 tactical domain for coordinating and funding completed useful work.

Its reference loop is:

```text
useful completed work
→ bounded public invoice
→ $BLDR primary evaluation
→ Native protection gate
→ transactional payout from BLDR Treasury
```

The mechanism separates economic participation from permanent status. Founder, employee, team, or agent identity creates no protocol claim by itself; any compensation policy must enter through an explicit allocation or the same bounded funding path available to the declared contributor class.

This contract defines mechanics and invariants. It does not define what work is valuable, who should perform it, how teams organize, what invoice etiquette is socially acceptable, or what demand strategy a downstream product should use.

## 3. Second-Order TMCTOL Composition

The `$BLDR` economy specializes TMCTOL as a second-order contour: its TMC accepts the first-order minted asset `$NTVE` as collateral, and its protocol-owned liquidity pairs `$BLDR` with `$NTVE`.

`Second-order` classifies the complete TMCTOL economy because `$BLDR` issuance begins in its TMC and depends on collateral issued by first-order TMCTOL. TOL names only the liquidity and strategic-capital component inside that economy. The order does not grant execution priority, governance superiority, or authority over the first-order economy.

The Builder contour inherits these TMCTOL properties rather than redefining them:

- Unidirectional TMC issuance with deterministic curve pricing.
- Transaction-local mint conservation.
- Protocol-owned XYK liquidity and live pro-rata reserve accounting.
- Explicit separation of spot price, stress-floor analysis, and public guarantee claims.
- Burn effects that remain conditional on funded and live execution.
- Deterministic rounding and ledger conservation.

The Builder contour adds one concrete allocation and custody topology described below.

## 4. `$BLDR` Mint and Custody Topology

For total `$BLDR` issuance `M`, recipient output `U`, and protocol output `T = M - U`, the reference split is:

```text
direct_treasury = floor(T / 2)
anchor_issuance = T - direct_treasury
U + anchor_issuance + direct_treasury = M
```

With the reference TMCTOL one-third/two-thirds mint split, this yields approximately:

```text
1/3 M → recipient
1/3 M → BLDR Anchor liquidity lane
1/3 M → BLDR Treasury
```

All `$NTVE` collateral paid into the `$BLDR` TMC is anchor-directed. The collateral is sent to the BLDR Liquidity Actor rather than divided with BLDR Treasury.

The two capital owners are independent:

- `BLDR Anchor` owns every protocol-created `$NTVE/$BLDR` LP token as a sealed dormant Immutable System Actor. Its runtime LP freeze admits incoming LP while exposing zero reducible balance to ordinary, admin-forced, internal transfer, burn, and LP-class destruction paths. Only its live pro-rata LP reserve claim counts as anchor support.
- `BLDR Treasury` owns spendable capital used by Builder governance, including invoice funding. Treasury spending cannot withdraw or mutate BLDR Anchor custody.

The `$BLDR` topology instantiates one technical Anchor-type bucket. Because it has no sibling A/B/C/D family, its human name is simply `BLDR Anchor`; `BLDR Treasury` is a separate treasury instance rather than a sub-position of that bucket. `Builder Bucket` is not an alias: Bucket Builder (`B`) is the distinct first-order spendable lane that funds `$BLDR` acquisition, whereas BLDR Anchor names immutable second-order LP custody.

Unmatched balances waiting in the liquidity lane remain visible custody but do not count as deployed anchor reserves until represented by live LP.

## 5. Relationship to First-Order TMCTOL

The first-order and Builder TMCTOL contours share one reference invariant: approximately one third of total issuance is directed toward immutable protocol-owned anchor liquidity.

They do not share a collateral percentage:

- First-order Bucket Anchor (`A`) receives half of first-order collateral under the reference four-bucket split.
- BLDR Anchor receives all `$NTVE` collateral paid into the `$BLDR` TMC.

No conclusion about second-order collateral may be inferred solely from the first-order bucket percentages. Every floor report must use the live reserves and sellable-pressure assumptions of the contour being reported.

## 6. Parent Capital Bridge

The reference economy may recycle first-order Bucket Builder (`B`) capital into the Builder contour through the paired Treasury B lane.

The target bridge preserves this sequence:

```text
Bucket B releases a bounded LP portion
→ Treasury B receives both pro-rata reserve assets
→ each reserve asset routes independently into $BLDR
→ recipient $BLDR splits between burn and BLDR Treasury
```

For recipient acquisition output `Q`:

```text
burn = floor(Q / 2)
recycled_treasury = Q - burn
burn + recycled_treasury = Q
```

Route accounting must remain explicit:

- An XYK or other market route acquires existing `$BLDR`; net issuance changes by `-burn`.
- A `$BLDR` TMC route creates full issuance `M`; net issuance changes by `M - burn`, while its ordinary direct-treasury and anchor allocations remain separately attributable.
- A report must not describe both branches as uniformly deflationary.

Runtime execution must advance bounded portions and preserve retained custody across retries. Analytical evaluation may collapse the sequence into one economic round only when it preserves reserve release, route choice, mint or market acquisition, burn, treasury funding, and anchor effects independently; the current simulator carries no dedicated Builder bridge model.

## 7. Governance Topology

The reference Builder domain is an L2 tactical governance domain protected by L1 Native economic power.

Its two tracks are:

- `Primary track`: `$BLDR` domain governance power. Under the Governance specification this is the runtime-resolved same-domain staking surface, not an unqualified free-balance vote.
- `Protection track`: Native `Veto / Pass` power resolved from the runtime-declared `NativeVotePower` surface.

This topology means Builder participants evaluate tactical work with `$BLDR`-backed power, while Native economic stakeholders retain a constitutional brake over the domain they collateralize and protect.

The protection track is not a second positive-governance lane:

- `Veto` can cancel under the Governance specification's immediate or final protection rules.
- `Pass` permits the primary track to decide and may authorize urgent handling only where an explicit domain/payload policy allows it.
- Native protection never prices an invoice and never substitutes for `$BLDR` primary approval.

Builder governance has no direct Root-equivalent authority over L1. A Builder-domain request for strategic action must use the Governance specification's bounded L2-to-L1 signal surface unless L1 explicitly delegates the target parameter or authority.

## 8. Invoice Contract

A Builder invoice selects the Governance specification's `L2TreasurySpend` payload and `Invoice` primary family. The Governance specification exclusively owns that family's choices, scalars, tie handling, protection interaction, and resolution order.

For Builder composition, a successful positive result determines one scalar target from the submitted base amount, while primary rejection produces no payout. Native `Veto / Pass` remains the separate protection track and never prices the work.

Every invoice payload declares:

- Beneficiary.
- Payout asset.
- Base amount.
- Treasury account.
- Required bounded IPFS CID for the canonical invoice document and supporting evidence.

The payload commits the content-addressed CID, not a mutable web URL. The referenced document may contain human-readable rationale and links to GitHub or other work evidence, but changing that document changes its CID and therefore requires a different proposal payload. IPFS availability remains an external materialization concern rather than canonical on-chain storage.

The treasury account is explicit but not arbitrary. The runtime composition must resolve it to a governance-approved sovereign account of a Mutable System Actor in the same Builder domain. Admission rejects an ordinary user account, an Immutable Actor such as BLDR Anchor, an unregistered Actor, or a treasury belonging to another governance domain.

BLDR Treasury is a Mutable System Actor treasury. It may hold an independently governed Actor Contract that spends bounded portions through ordinary economic flows. Invoice settlement is a separate governance-authorized debit of the same sovereign account: it requires no Actor-owner signature or Actor Step and does not mutate, pause, replace, or bypass the treasury's Actor Contract.

The Builder domain selects the Governance specification's `BaseFloorCapped` invoice-settlement policy. That specification exclusively owns target calculation, minimum payable capacity, bounded clipping, atomic transfer, failure, and receipt semantics.

For a scalar target above `1.0x`, enactment may pay less than the target only when the treasury can still cover at least the complete submitted base amount. Thus an `Amplify` target of `2.0x` may settle at `1.5x`, while capacity below `1.0x` fails with zero payout. Targets at or below `1.0x` require their complete target and never clip further.

Treasury capacity is evaluated from authoritative state at enactment because the Mutable Treasury Actor or an earlier invoice may spend between proposal submission, voting, and execution. Opening an invoice creates no balance reservation. The proposer must inspect treasury capacity and accept that concurrent spending can cause settlement failure or reduce only an above-base premium within the range authorized by the selected Governance policy.

Builder clients must distinguish target from actual payout and primary approval from successful enactment. Before execution they must not present either the target or current treasury capacity as a guaranteed payout.

## 9. Economic and Governance Invariants

A conforming Builder realization preserves all of these invariants:

- `Mint conservation`: recipient, anchor-directed, and direct-treasury `$BLDR` allocations conserve full issuance after deterministic rounding.
- `Collateral direction`: all `$BLDR` TMC `$NTVE` collateral remains assigned to the anchor-liquidity lane.
- `Anchor custody`: every protocol-created `$NTVE/$BLDR` LP token belongs to the sealed Immutable BLDR Anchor; treasury payout and every ledger debit path observe zero reducible Anchor LP balance.
- `Floor honesty`: only live LP reserve claims count as anchor support.
- `Bridge conservation`: every released parent reserve asset is consumed by a declared route or remains visibly attributed to Treasury B.
- `Acquisition conservation`: recipient `$BLDR` output conserves exactly across burn and recycled treasury destinations.
- `Route honesty`: market and TMC acquisition branches report distinct issuance effects.
- `Track separation`: `$BLDR` primary evaluation and Native protection retain independent ballots, power surfaces, and meanings.
- `Treasury authorization`: the payload-selected account resolves to an approved Mutable System Actor treasury in the Builder domain and never to BLDR Anchor or an arbitrary account.
- `Invoice settlement`: the Builder domain uses the Governance specification's `BaseFloorCapped` policy; above-base premium may clip to spendable capacity, while capacity below the complete base floor produces explicit execution failure and zero payout.
- `Invoice atomicity`: the one Governance-computed actual amount transfers completely or remains absent; no Builder adapter independently clips, prorates, reconstructs, or weakens that result.
- `Actor independence`: governance debit changes treasury custody only and does not mutate the treasury Actor Contract or lifecycle.
- `Authority containment`: Builder governance receives domain-local authority over declared treasury custody but cannot synthesize L1 or general Root-equivalent authority.
- `Boundedness`: invoice CID, proposal, ballot, execution, retry, projection, and retained-history surfaces obey their owning subsystem bounds.

## 10. Read and Evidence Surfaces

Canonical on-chain Builder truth should expose the bounded current state needed to verify:

- `$BLDR` curve and issuance state.
- BLDR Anchor account, LP ownership, and live reserve claim.
- BLDR Treasury account and spendable balances.
- Active and recent Builder governance lifecycle.
- Primary and protection track identity, power profiles, ballots, and tallies.
- Invoice CID, treasury account, base amount, target amount, actual amount, capacity-limited status, outcome, enactment or execution-failure status, and successful execution receipt.
- Current burn and capital-bridge liveness or explicit unavailability where the runtime supports those plans.

Permanent invoice archives, contributor histories, searchable work records, and long-range governance analytics are materialized views. Product clients must not present those surfaces as direct canonical-chain history.

Conformance evidence is distributed by truth owner:

- Simulator tests prove the inherited project-independent TMCTOL curve, reserve, conservation, floor, and compression mathematics; the Builder-specific allocation and bridge equations remain analytical contract claims until a dedicated executable owner is introduced.
- Governance pallet and runtime tests prove shipped track semantics, invoice resolution, transactional payout, authority, and bounded lifecycle; the architecture document identifies the remaining invoice-settlement gap from this contract.
- Runtime integration tests prove concrete accounts, asset routing, System Actor topology, and cross-pallet composition.
- Client tests prove provenance labels, current-state projection, transaction feedback, and explicit materialized dependencies.

## 11. Framework and Instance Boundary

DEOS owns the reusable Builder mechanism and the reference composition described here. A downstream instance decides whether to enable it and owns:

- Builder-domain name and token symbol.
- Accepted work and evidence norms.
- Invoice etiquette, review culture, and payout appetite.
- Founder, team, and contributor allocation policy.
- Demand generation, product loops, and market narrative.
- Any parameter changes that remain inside declared governance and economic bounds.

A fork may replace `$BLDR`, invoices, or the entire Builder domain without violating DEOS. It must stop claiming this contract when its replacement no longer preserves the stated topology and invariants.

## 12. Non-Goals and Non-Guarantees

This contract does not define:

- The general TMCTOL standard or its project-independent mathematics.
- The internal Governance pallet lifecycle, tally algorithms, or storage topology.
- A universal labor philosophy or objective measure of useful work.
- A founder entitlement, payroll promise, or mandatory contributor structure.
- Guaranteed demand, appreciation, liquidity, invoice quality, or governance wisdom.
- Unbounded on-chain work records, reputation history, or invoice archives.
- Automatic activation of optional capital-bridge plans.

The Builder economy is a bounded reference mechanism for funding useful work. Its economic outcomes remain conditional on utility, participation, treasury funding, market liquidity, correct execution, and honest governance.
