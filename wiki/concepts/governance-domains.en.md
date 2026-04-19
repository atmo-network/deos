---
page_type: concept
title: Governance Domains
summary: A governance domain is one typed governance cell inside the larger governance system. It binds together the governed subject, primary and protection power surfaces, valid payload families, cadence, and execution authority.
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
last_compiled: 2026-04-20
confidence: 0.93
---

# Governance Domains

## Summary

A governance domain is one concrete governance cell inside the wider DEOS Governance system. It is the unit that tells the runtime and the user what is being governed, whose power counts there, which protection surface can intervene, which proposal families make sense, and what kind of execution authority the resulting decision may reach.

So while [Governance Overview](../overview/governance-overview.en.md) explains the whole subsystem, this page explains one of its key building blocks.

## What A Domain Binds Together

A governance domain is not just “a namespace for proposals.” In DEOS it binds together:

- The governed subject
- The primary voting surface
- The protection voting surface
- The valid payload kinds
- The cadence rules that apply there
- The execution authority that successful proposals may actually reach

That is what makes a domain useful: it turns governance power from a vague social idea into a typed, inspectable contract.

## The Four Axes Around A Domain

The governance model stays compact by combining four explicit axes:

- `GovernanceDomain`
- `CadenceMode`
- `ProposalPayloadKind`
- `ProtectionTrack`

A domain is where those axes become concrete. It is the place where “strategy versus tactics,” “binary versus invoice voting,” and “advisory versus executable action” stop being abstract ideas and become actual runtime policy.

## The Current Canonical Domain Pairs

The current reference line keeps two domain pairs especially visible:

- `Native + $VETO` for protocol and network strategy
- `$BLDR + Native` for the flagship tactical domain

This means:

- Strategic proposals are protected by `$VETO`
- The flagship tactical domain is protected by native staking weight

Those are not just symbolic pairings. They define who can vote, who can protect, and what kind of authority a successful proposal may legitimately exercise.

## What Can Differ Between Domains

Two domains may differ on all of these:

- Which asset or stake surface supplies primary voting weight
- Which surface supplies protection voting weight
- Which payload kinds are meaningful there
- Whether the primary track is `Binary` or `Invoice`
- Whether the combination is publicly submittable or admin-only
- Whether an opening fee exists for that public path
- Whether urgent handling is allowed
- Which execution authority the payload can reach if approved

This is the core reason domains exist. DEOS does not want one governance token and one voting rule to pretend they honestly govern every layer of the system.

## Primary Track Family Is Domain-Shaped

Domains help determine the family of the primary track.

### Binary family

In a binary family, the primary lane is the familiar:

- `Aye`
- `Nay`

### Invoice family

In the canonical tactical `$BLDR` treasury domain, the primary lane is invoice-shaped:

- `Amplify`
- `Approve`
- `Reduce`
- `Nay`

That family exists because tactical spending is not always a yes-or-no question. Sometimes governance needs to choose the payout scalar, not just approve or reject a transfer.

## Protection Is Also Domain-Shaped

The protection lane is not one universal veto rule applied the same way everywhere. A domain determines:

- Which protection surface is eligible
- Which raw-veto thresholds matter
- Whether a protection-track `Pass` can procedurally accelerate urgent handling
- How final protection gating should be interpreted at resolution time

This is why a domain is a constitutional cell, not merely a proposal folder.

## Public Cadence Lives On Top Of Domains

On the shipped ordinary public line, domains currently inherit the same broad public rhythm:

- `3 day` lead-in
- `7 day` ordinary protection window
- `7 day` ordinary primary window
- `3 day` enactment delay

But domains still matter inside that shared rhythm, because the meaning of votes, the protection source, and the allowed payload families can still differ.

## Current Public Submission Scope By Domain Family

The current runtime keeps signed public submission intentionally bounded.

Today the public path covers:

- `Intent` across domains
- Tactical `$BLDR` `L2SignalToL1`
- Tactical `$BLDR` `L2TreasurySpend`

That means domain policy does not only decide who votes. It also decides which proposal families are actually opened to signed public ingress on the current line.

## Execution Authority Is Not The Same Everywhere

A domain does not magically grant Root power. It constrains where a successful payload may execute.

On the current line that means:

- `L1RootAction` is strategic and Root-equivalent
- `L2TreasurySpend` is domain-local treasury execution
- `L2ParameterChange` must stay inside genuinely delegated domain-owned surfaces
- `Intent` and `L2SignalToL1` remain advisory by contract

This is one of the most important practical uses of domains: they keep tactical and strategic authority from silently collapsing into each other.

## Why Some Tempting Surfaces Stay Out Of Domain Control

The docs are explicit that some system surfaces are still not honest tactical-domain parameters. On the current line, examples include:

- TMC launch physics
- Staking admin onboarding or recovery paths
- AAA global controls
- Asset-registry registration or migration

If a tactical domain wants to affect one of those system-owned areas, it must use an explicit handoff such as `L2SignalToL1` rather than pretending the domain already owns that authority.

## Domains And The Live Read Model

Domains also shape the governance read model. Domain-aware runtime views expose bounded live truth such as:

- Proposal status
- Proposal timing
- Tally interpretation
- Execution authority
- Submission authority
- Opening-fee truth
- Payload availability
- Recent finalized detail

That makes domains visible not only in the constitution, but also in the live product surface.

## Simple Mental Model

If you want the shortest useful mental model, read a governance domain like this:

- “Whose problem is this?”
- “Whose votes count here?”
- “Who can constitutionally block it?”
- “What kind of payload is valid here?”
- “How far can successful execution actually reach?”

That is the conceptual work a domain does inside DEOS Governance.

## Related

- [Governance Overview](../overview/governance-overview.en.md)
- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [TMCTOL Standard](tmctol-standard.en.md)
- [Staking Pools](staking-pools.en.md)
- [Read-Model Split](read-model-split.en.md)
- [Physics-First vs Politics-First](../comparisons/physics-vs-politics.en.md)
- [Core Terms](../glossary/core-terms.en.md)

## Sources

- `docs/governance.specification.en.md`
- `docs/governance.architecture.en.md`
- `docs/staking.specification.en.md`
