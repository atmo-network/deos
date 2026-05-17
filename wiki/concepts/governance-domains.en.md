---
page_type: concept
title: Governance Domains
summary: A governance domain is one typed governance cell inside the larger governance system. It binds the governed subject, voting and protection surfaces, valid payload families, cadence, and execution authority.
locale: en
canonical_page_id: governance-domains
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/governance.specification.en.md
  - ../../docs/governance.architecture.en.md
  - ../../docs/staking.specification.en.md
status: active
audience: newcomer
tags:
  - concept
  - governance
  - domains
related:
  - Governance Overview
  - DEOS Framework Overview
  - TMCTOL Standard
  - Staking Pools
  - Read-Model Split
  - Physics-First vs Politics-First
  - Core Terms
last_compiled: 2026-05-17
confidence: 0.93
---

# Governance Domains

## Summary

A governance domain is one typed governance cell inside DEOS Governance. It tells the runtime and the user what is governed, whose voting power counts, which protection surface can intervene, which proposal families are valid, and how far successful execution may reach.

[Governance Overview](../overview/governance-overview.en.md) explains the whole subsystem. This page explains the unit that keeps that subsystem from collapsing into one flat voting market.

## Domain Contract

A domain binds six things together:

- Governed subject;
- Primary voting surface;
- Protection voting surface;
- Valid payload kinds;
- Cadence rules;
- Maximum execution authority.

That contract turns governance from a vague social process into typed policy. It also keeps the four governance axes concrete: `GovernanceDomain`, `CadenceMode`, `ProposalPayloadKind`, and `ProtectionTrack`.

Two domains can therefore differ by who votes, who protects, which payloads are valid, whether the primary track is `Binary` or `Invoice`, whether signed public submission is open, whether fees or urgent handling apply, and which authority the payload can actually reach.

## Current Reference Shape

The current line makes two domain pairs especially visible:

- `Native + $VETO` for protocol and network strategy;
- `$BLDR + Native` for the flagship tactical domain.

Strategic proposals are protected by `$VETO`. The flagship tactical domain is protected by native staking weight. Those pairings are not symbolic: they define voting power, protection power, and legitimate execution reach.

The ordinary public cadence is currently shared:

- `3 day` lead-in;
- `7 day` protection window;
- `7 day` primary voting window;
- `3 day` enactment delay.

The signed public path remains intentionally bounded: `Intent` across domains, tactical `$BLDR` `L2SignalToL1`, and tactical `$BLDR` `L2TreasurySpend`.

## Tracks, Payloads, and Authority

Primary track shape is domain-owned. Some domains use a binary `Aye / Nay` family. The canonical tactical `$BLDR` treasury domain uses an invoice-shaped family: `Amplify`, `Approve`, `Reduce`, `Nay`. Tactical spending may need a payout scalar, not only yes/no approval.

Protection is also domain-shaped. A domain decides which protection surface is eligible, which veto thresholds matter, whether `Pass` can accelerate urgent handling, and how final protection gating is interpreted.

Execution authority is constrained by the domain. `L1RootAction` is strategic and Root-equivalent. `L2TreasurySpend` is domain-local treasury execution. `L2ParameterChange` must stay inside genuinely delegated domain-owned surfaces. `Intent` and `L2SignalToL1` stay advisory by contract.

Some tempting surfaces remain outside tactical-domain ownership: TMC launch physics, staking admin onboarding/recovery, AAA global controls, and asset-registry registration or migration. A tactical domain must use an explicit handoff such as `L2SignalToL1` instead of pretending it already owns those areas.

## Live Read Model

Domains also shape governance read-model output. Domain-aware runtime views expose bounded live truth such as proposal status, timing, tally interpretation, execution authority, submission authority, opening-fee truth, payload availability, and recent finalized detail.

That makes domains visible both in the constitutional model and in the product surface.

## Mental Model

Read a governance domain through five questions:

1. Whose problem is this?
2. Whose votes count here?
3. Who can constitutionally block it?
4. What payload is valid here?
5. How far can successful execution reach?

## Related

- [Governance Overview](../overview/governance-overview.en.md)
- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [TMCTOL Standard](tmctol-standard.en.md)
- [Staking Pools](staking-pools.en.md)
- [Read-Model Split](read-model-split.en.md)
- [Physics-First vs Politics-First](../comparisons/physics-vs-politics.en.md)
- [Core Terms](../glossary/core-terms.en.md)
