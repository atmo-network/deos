---
page_type: glossary
title: Core Terms
summary: A compact glossary for the most important DEOS and TMCTOL terms. Use this page first when project vocabulary, abbreviations, or framework-versus-standard naming feels ambiguous.
locale: en
canonical_page_id: core-terms
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../README.md
  - ../../docs/README.md
  - ../../docs/tmctol.specification.en.md
  - ../../docs/core.architecture.en.md
  - ../../docs/aaa.specification.en.md
  - ../../docs/governance.specification.en.md
  - ../../docs/governance.architecture.en.md
  - ../../docs/read-model.contract.en.md
status: active
audience: newcomer
tags:
  - glossary
  - terminology
related:
  - DEOS Framework Overview
  - TMCTOL Standard
  - Token-Driven Automation
  - Governance Overview
  - Governance Domains
  - Read-Model Split
  - Newcomer FAQ
last_compiled: 2026-04-20
confidence: 0.95
---

# Core Terms

## Summary

This glossary keeps the most important project terms in one place. It is especially useful because the repository draws a hard line between framework identity, standard identity, runtime execution vocabulary, governance vocabulary, and read-model terminology.

## Terms

### DEOS

`Deterministic Economic Operating System`. The framework identity for the repository and reference stack.

### TMCTOL

`Token Minting Curve + Treasury-Owned Liquidity`. The current flagship economic standard running on DEOS.

### TMC

`Token Minting Curve`. The mint-only linear issuance mechanism that defines the current price ceiling for new supply.

### TOL

`Treasury-Owned Liquidity`. The policy layer that routes mint output into protocol-controlled liquidity and segments that liquidity into buckets.

### AAA

`Account Abstraction Actors`. In DEOS, this names the full runtime system: the pallet, scheduler, lifecycle rules, and execution environment.

### AA-Actor

One concrete bounded runtime instance inside the broader AAA system.

### Axial Router

The routing pallet that compares bounded execution paths, chooses the better route, and captures its trading fee for the protocol flow.

### Balance Ingress

The token-driven trigger where assets arriving in the system can drive the next deterministic state transition.

### Governance Domain

One typed governance cell inside the larger governance system. It binds together the governed subject, power surfaces, valid payload families, cadence, and execution authority.

### Primary Track

The lane that answers the proposal itself. Depending on the domain and payload family, it may be binary (`Aye / Nay`) or invoice-shaped (`Amplify / Approve / Reduce / Nay`).

### Protection Track

The constitutional `Veto / Pass` lane that can block or procedurally accelerate protected governance flows. It is separate from the primary track rather than a hidden alias of ordinary `Nay`.

### Proposal Payload Kind

The typed description of what kind of governance action a proposal wants to authorize, such as strategic Root action, tactical treasury spend, tactical parameter change, same-domain intent, or tactical signal toward the strategic layer.

### Execution Authority

The authority level that a successful governance payload may actually reach when enacted. In DEOS, tactical domains do not automatically inherit strategic or Root-equivalent power.

### GovXP

The bounded governance-participation signal exported by the governance subsystem for later reward and reputation logic. On the current line it is counters-first rather than a live vote-power multiplier.

### Canonical Projection

A bounded on-chain view intended for direct client consumption as part of the live protocol contract.

### Materialized View

An indexed or externally reconstructed view used for archive, search, dashboards, or analytics rather than bounded consensus truth.

### Native / `$NTVE`

The sovereign base token of the current reference line.

### `$VETO`

The protection token used for the strategic constitutional surface on the current line.

### `$BLDR`

The flagship tactical governance and builder-coordination token on the current line.

### `stXXX`

Transferable staking receipts representing share-based ownership in staking pools.

## Related

- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [TMCTOL Standard](../concepts/tmctol-standard.en.md)
- [Token-Driven Automation](../concepts/token-driven-automation.en.md)
- [Governance Overview](../overview/governance-overview.en.md)
- [Governance Domains](../concepts/governance-domains.en.md)
- [Read-Model Split](../concepts/read-model-split.en.md)
- [Newcomer FAQ](../faq/newcomer-faq.en.md)

## Sources

- `README.md`
- `docs/README.md`
- `docs/tmctol.specification.en.md`
- `docs/core.architecture.en.md`
- `docs/aaa.specification.en.md`
- `docs/governance.specification.en.md`
- `docs/governance.architecture.en.md`
- `docs/read-model.contract.en.md`
