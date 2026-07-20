---
page_type: concept
title: $BLDR Builder Economy
summary: The reference builder pattern coordinates proven work through public invoices, tactical governance, protocol-owned liquidity, and bounded treasury payouts without making founder privilege a framework entitlement.
locale: en
canonical_page_id: builder-economy
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/README.md
  - ../../docs/manifesto.en.md
  - ../../docs/framework-instance.contract.en.md
  - ../../docs/governance.specification.en.md
  - ../../docs/governance.architecture.en.md
  - ../../docs/aaa.architecture.en.md
  - ../../docs/core.architecture.en.md
  - ../../docs/tmc.architecture.en.md
status: active
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
  - AAA System
  - TOL Bucket Scenarios
last_compiled: 2026-07-20
confidence: 0.9
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
- The governance-declared BLDR Treasury funding source.

The `$BLDR` primary track evaluates the invoice through four options:

- `Amplify` — pay `2.0x` the base amount;
- `Approve` — pay `1.0x`;
- `Reduce` — pay `0.5x`;
- `Nay` — reject with no payout.

Native staking power forms the separate `Pass / Veto` protection track. It protects the constitutional boundary but does not price the work. Approved execution is transactional: the full bounded payout succeeds or the runtime records an explicit failure without a partial payment.

## Economic Wiring

The reference `$BLDR` TMC flow gives labor funding its own capital circuit:

```text
buyer pays $NTVE
  -> TMC mints $BLDR
  -> about 1/3 to buyer
  -> about 1/3 to $NTVE/$BLDR protocol-owned liquidity
  -> about 1/3 to BLDR Treasury
```

TMC sends two thirds of minted `$BLDR` to the BLDR Splitter. The splitter divides that protocol allocation equally between the BLDR Liquidity Actor and BLDR Treasury, while routing the incoming `$NTVE` collateral to liquidity provisioning. The resulting LP accumulates in immutable BLDR Bucket A.

A separate L1 Building lane can unwind its own LP into Treasury B, gradually buy `$BLDR` on the market, and burn it. Builder payouts and buyback/burn therefore remain distinct flows: one funds useful work, while the other applies bounded market demand and supply reduction when its execution plan remains live.

## A Federated Domain, Not an Isolated Economy

The builder domain has its own token, treasury, governance, liquidity lane, and System AAA actors, but it does not stand alone:

- `$NTVE` is its TMC collateral and liquidity pair;
- Native economic locks protect its governance domain;
- L1 capital can support BLDR buyback and burn;
- `$BLDR` governance cannot directly rewrite TMC launch physics, global AAA controls, staking administration, or asset registration.

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
- [AAA System](../overview/aaa-system.en.md)
- [TOL Bucket Scenarios](tol-bucket-scenarios.en.md)
