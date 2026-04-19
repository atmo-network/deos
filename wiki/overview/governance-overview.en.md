---
page_type: overview
title: Governance Overview
summary: A newcomer-facing map of DEOS Governance as a whole that explains why it exists, how strategy and tactics stay separated, which moving parts matter most, and where to go next for deeper concepts.
locale: en
canonical_page_id: governance-overview
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/governance.specification.en.md
  - ../../docs/governance.architecture.en.md
  - ../../docs/manifesto.en.md
status: active
audience: newcomer
tags:
  - overview
  - governance
  - domains
related:
  - Governance Domains
  - Physics-First vs Politics-First
  - Staking Pools
  - Core Terms
last_compiled: 2026-04-20
confidence: 0.94
---

# Governance Overview

## Summary

DEOS Governance is the framework's bounded governance layer. Its job is not to replace the protocol's economic kernel with politics-by-default, but to govern the strategic, tactical, and social surfaces that remain after the framework mechanizes what it can.

This page is the wide-angle map of that subsystem. It explains what governance is for, how the current reference line is shaped, and which deeper pages to read next.

## Why Governance Exists Here At All

The DEOS worldview is not “everything should be decided by token voting.” It is closer to this:

- Protocol physics should be mechanized where possible
- Strategic protection should stay explicit
- Tactical and treasury choices still need bounded human coordination
- The governance surface should stay queryable and constitutionally legible

That is why DEOS Governance looks more like a constitutional layer above a deterministic kernel than a generic voting portal.

## The Big Structural Idea

The governance model keeps three large distinctions in view:

- `Strategy` is not the same as `tactics`
- `Approval` is not the same as `protection`
- `Live on-chain governance truth` is not the same as `archive history`

Those three separations explain most of the system's shape.

## What the Current Line Looks Like

At a high level, the current reference line has:

- Explicit governance domains instead of one undifferentiated referendum pool
- A dual-track model: primary decision lane plus protection lane
- Typed payload kinds instead of opaque “proposal blobs”
- A public ordinary cadence with `3 day` lead-in, `7 day` voting windows, and `3 day` enactment delay
- Bounded on-chain query surfaces for live governance UX

You do not need every detail at once to understand the intent. The important point is that DEOS Governance tries to keep power surfaces explicit and limited.

## Strategic Versus Tactical Governance

The current reference line keeps a strong split between strategic and tactical decisions.

In practice, that means:

- Strategic governance protects protocol and network-level subjects
- Tactical governance handles narrower domain-local spending and coordination
- Tactical domains do not automatically inherit strategic authority

This is one of the main reasons DEOS Governance exists in its current form: it tries to avoid collapsing every serious decision into one flat voting market.

## Why There Are Two Tracks

DEOS Governance is dual-track by design:

- The `primary track` answers the proposal itself
- The `protection track` answers whether the proposal should be constitutionally blocked or procedurally allowed through

This is a whole-system property, not just a detail of one proposal family. It is one of the strongest differences between DEOS Governance and simpler “Aye / Nay for everything” models.

If you want the deeper explanation of how that protection layer is bound to concrete governance cells, read [Governance Domains](../concepts/governance-domains.en.md).

## What Governance Can Talk About

The current governance vocabulary stays intentionally small. Proposals are described through explicit payload kinds such as:

- Strategic Root-equivalent action
- Tactical treasury spending
- Tactical parameter change
- Advisory same-domain intent
- Tactical signal toward the strategic layer

The exact meaning of those payload kinds matters, but the overview-level point is simpler: governance action is typed on purpose. DEOS does not want proposal meaning to hide inside social convention or opaque bytes.

## How the Public Lifecycle Feels

From a newcomer perspective, the important lifecycle facts are:

- Protection opens immediately when a proposal is submitted
- Ordinary primary voting does not start immediately
- Successful approval may still wait in enactment delay
- Execution failure is a real state, not something hidden behind “approved”
- Recent finalized outcomes stay visible on-chain only for a bounded time

That means DEOS Governance is designed to show honest live state, not just a raw pile of past events.

## Governance and the Read Model

The runtime exports bounded governance views for live product use. That includes proposal state, timing, tally interpretation, execution authority, payload availability, and recent finalized outcomes.

This is deliberate. DEOS wants canonical live governance UX to be queryable on-chain, while long-range archive/search/timeline surfaces remain the job of indexed or materialized layers.

## Governance and Staking

Governance also feeds a bounded participation-quality signal into staking rewards.

The overview-level point is not that governance and staking are the same subsystem. It is that DEOS treats governance quality as economically relevant, while still refusing to keep unbounded social history in runtime storage.

## How To Read The Governance Wiki Cluster

Use the pages in this order:

1. `Governance Overview` — what the subsystem is for
2. [Governance Domains](../concepts/governance-domains.en.md) — how one governance cell is typed
3. `Core Terms` — for recurring vocabulary
4. `Physics-First vs Politics-First` — for the philosophical frame

The overview is the map. The concept pages are the closer inspections of particular building blocks.

## Related

- [Governance Domains](../concepts/governance-domains.en.md)
- [Physics-First vs Politics-First](../comparisons/physics-vs-politics.en.md)
- [Staking Pools](../concepts/staking-pools.en.md)
- [Core Terms](../glossary/core-terms.en.md)

## Sources

- `docs/governance.specification.en.md`
- `docs/governance.architecture.en.md`
- `docs/manifesto.en.md`
