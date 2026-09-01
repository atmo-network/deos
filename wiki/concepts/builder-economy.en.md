---
type: concept
title: $BLDR Builder Economy
description: The reference builder pattern coordinates proven work through public invoices, tactical governance, protocol-owned liquidity, and bounded treasury payouts without making founder privilege a framework entitlement.
locale: en
canonical_page_id: builder-economy
translation_status: source
available_locales:
  - en
  - ru
sources:
  - resource: ../../docs/README.md
  - resource: ../../docs/builder-economy.contract.en.md
  - resource: ../../docs/framework-instance.contract.en.md
  - resource: ../../docs/tmctol.specification.en.md
  - resource: ../../docs/manifesto.en.md
  - resource: ../../template/pallets/governance/docs/specification.en.md
  - resource: ../../template/pallets/governance/docs/architecture.en.md
  - resource: ../../template/pallets/actors/docs/architecture.en.md
  - resource: ../../docs/core.architecture.en.md
  - resource: ../../template/pallets/tmc/docs/architecture.en.md
status: stable
audience: newcomer
tags:
  - concept
  - bldr
  - builders
  - governance
  - treasury
  - labor
related:
  - Token Surfaces
  - Governance Domains
  - Physics-First vs Politics-First
  - TMCTOL Standard
  - Actors System
  - TOL Bucket Scenarios
---

# `$BLDR` Builder Economy

## Summary

The `$BLDR` builder economy is the flagship tactical L2 domain in the DEOS reference line. It coordinates completed useful work rather than granting permanent economic privilege for founder or team status.

`$BLDR` holders evaluate invoices, Native economic power protects the domain boundary, and the BLDR Treasury executes approved payouts. This keeps tactical labor funding separate from the L0 Economic Physics and L1 strategic authority that protect the wider system.

## Founder as the First Worker

The builder pattern makes no founder rent a possible instance policy, not a mandatory DEOS law. A downstream economy still chooses whether founders receive an allocation, a fee share, neither, or another explicit arrangement.

When an instance chooses no founder allocation and no personal fee share, the founder enters the economy as its first worker:

```text
completed work
  -> public invoice
  -> domain evaluation
  -> bounded treasury payout
```

The same path can serve later contributors, teams, and agents. Status alone creates no claim; the reference principle is `No allocation for status. Reward for proven value.`

## Invoice Governance

A tactical treasury invoice declares:

- A beneficiary;
- A payout asset;
- A base amount;
- The explicit governance-approved BLDR Treasury sovereign account;
- A bounded canonical CIDv1 for the content-addressed invoice document and evidence.

The `$BLDR` primary track evaluates the invoice through four options:

- `Amplify` — target `2.0x` the base amount, with only the premium above `1.0x` capacity-capped;
- `Approve` — require and pay `1.0x`;
- `Reduce` — require and pay `0.5x`;
- `Nay` — reject with no payout.

Native staking power forms the separate `Pass / Veto` protection track. It protects the constitutional boundary but does not price the work. If an above-base target is `2.0x` and enactment-time capacity is `1.5x`, the complete `1.5x` pays atomically. Capacity below `1.0x` fails with no payout. Targets at or below base require their complete amount and never clip further.

BLDR Treasury is a Mutable System Actor treasury. Its own bounded Actor Contract or an earlier invoice may spend from the same sovereign custody between submission and enactment, and proposals create no balance reservation. Execution order may therefore reduce only an above-base premium or cause a below-floor failure requiring a new vote after replenishment. Governance may debit only the approved Builder treasury account and does not mutate the treasury's Actor Contract or gain authority over BLDR Anchor.

## Economic Wiring

The `$BLDR` domain is a second-order TMCTOL: its TMC mints `$BLDR` against first-order `$NTVE`, while its TOL component has two independent capital owners. BLDR Anchor holds all protocol-created `$NTVE/$BLDR` LP as a sealed dormant Immutable System identity whose consensus freeze admits inbound LP and blocks every debit or LP-class destruction; BLDR Treasury funds invoices. Technically the former is an immutable Anchor-type bucket. Because that TOL topology has no peer lettered family, its human name remains `BLDR Anchor` rather than acquiring an artificial `Bucket A` qualifier. It is never called Builder Bucket: Bucket Builder (`B`) is the separate first-order spendable lane, while BLDR Anchor signals immutable second-order LP custody.

```text
buyer pays $NTVE
  -> TMC mints $BLDR
  -> about 1/3 to recipient
  -> about 1/3 to immutable BLDR Anchor liquidity
  -> about 1/3 to BLDR Treasury
```

TMC sends two thirds of minted `$BLDR` to the BLDR Splitter. The splitter divides that protocol allocation equally between the BLDR Liquidity Actor and BLDR Treasury. All incoming `$NTVE` collateral is directed to the liquidity lane, and every resulting LP token enters BLDR Anchor; unmatched buffer remains liquidity-lane custody rather than counted LP reserves.

The parent first-order Bucket Builder (`B`) lane supplies a second capital path. Bucket B gradually releases LP to Treasury B, which receives both `$NTVE` and foreign reserves and routes both into `$BLDR`. Half of recipient output burns and half enters BLDR Treasury. An XYK route buys existing `$BLDR` and contracts issuance by the burned half; a TMC route creates full issuance, separately funds anchor and direct-treasury lanes, and burns only half of its recipient allocation, so it may remain net expansionary.

The Builder Economy contract defines this dual-reserve 50/50 bridge analytically; the simulator retains only the inherited project-independent TMCTOL mathematics and does not model the Builder composition. The shipped runtime currently realizes the `$BLDR` mint, split, and anchor/treasury path but its optional Treasury B plan still spends only Native and burns all acquired `$BLDR`; runtime convergence and production evidence remain open.

## A Federated Domain, Not an Isolated Economy

The builder domain has its own token, treasury, governance, liquidity lane, and System Actors, but it does not stand alone:

- `$NTVE` is its TMC collateral and liquidity pair;
- Native economic locks protect its governance domain;
- L1 capital can support `$BLDR` buyback and burn;
- `$BLDR` governance cannot directly rewrite TMC launch physics, global Actors controls, staking administration, or asset registration.

This is the Fractal Federation pattern: a tactical domain remains autonomous inside its declared competence while sharing capital, protection, and infrastructure with the parent economy. It behaves like a bounded organ of DEOS rather than a sovereign replacement for it.

## What the Pattern Does Not Guarantee

The framework provides bounded invoice, treasury, governance, liquidity, and automation mechanisms. It does not guarantee:

- Demand for `$BLDR`;
- High-quality work or fair social judgment;
- Product-market fit for the downstream ecosystem;
- A specific founder-allocation policy;
- Profit, price appreciation, or uninterrupted buyback execution.

A production instance must still choose its launch allocation, create and fund the `$NTVE/$BLDR` economy, activate the relevant plans, and build products that make builder coordination valuable.

## Related

- [Token Surfaces](token-surfaces.en.md)
- [Governance Domains](governance-domains.en.md)
- [Physics-First vs Politics-First](../comparisons/physics-vs-politics.en.md)
- [TMCTOL Standard](tmctol-standard.en.md)
- [Actors System](../overview/actor-system.en.md)
- [TOL Bucket Scenarios](tol-bucket-scenarios.en.md)
