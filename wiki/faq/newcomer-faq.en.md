---
page_type: faq
title: Newcomer FAQ
summary: A compact FAQ for recurring newcomer questions about DEOS, TMCTOL, AAA, governance, staking, data surfaces, and the reference client. Use this page when you need quick orientation before diving into the full docs.
locale: en
canonical_page_id: newcomer-faq
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../README.md
  - ../../docs/README.md
  - ../../docs/manifesto.en.md
  - ../../docs/aaa.specification.en.md
  - ../../docs/governance.specification.en.md
  - ../../docs/staking.specification.en.md
  - ../../docs/read-model.contract.en.md
  - ../../docs/web-client.architecture.en.md
status: active
audience: newcomer
tags:
  - faq
  - onboarding
related:
  - DEOS Framework Overview
  - First Steps
  - AAA System
  - Physics-First vs Politics-First
  - Core Terms
last_compiled: 2026-04-16
confidence: 0.92
---

# Newcomer FAQ

## Summary

This page answers the questions that usually show up first when someone lands in the repository: what DEOS is, how TMCTOL fits into it, what governance still controls, how AAA works at a high level, and how honest the reference client is supposed to be.

For exact contracts, always return to `/docs`.

## Is DEOS the token or the standard?

No. `DEOS` is the framework and reference stack. `TMCTOL` is the current flagship tokenomic standard running on top of that framework.

## Why are there both `/docs` and `/wiki`?

`/docs` is the authoritative source. It carries the normative contracts, specifications, and architecture notes.

`/wiki` is the smaller navigation and onboarding layer. It is meant to help humans and agents find the right concept quickly, then return to `/docs` when full detail is needed.

## Why does TMCTOL avoid redemption?

The current standard uses a unidirectional minting curve. The docs describe that as a way to avoid reserve extraction through the curve path and to preserve clearer downside boundaries.

## Does governance disappear in DEOS?

No. Governance stays, but the project tries to narrow its role. Governance should steer direction, tactical domains, and bounded upgrade paths instead of acting as day-to-day control over the protocol's survival physics.

## What does `deterministic` mean here?

It means protocol-managed economic reactions should be explicit and repeatable for the same on-chain state, typed payloads, and token flows. It does not mean the market becomes perfectly predictable.

## What is the difference between AAA and an AA-Actor?

`AAA` is the whole Account Abstraction Actors system: the pallet, scheduler, lifecycle rules, and execution environment.

An `AA-Actor` is one concrete runtime instance inside that system.

## How does staking work at a high level?

Staking is modeled as a multi-asset share vault. Each asset has its own pool, and users own shares or receipt-backed ownership instead of receiving constant fan-out reward writes.

## Why does the project talk so much about on-chain vs materialized data?

Because DEOS explicitly separates bounded canonical runtime truth from archive, search, analytics, and other indexed views. The goal is to avoid pretending that off-chain infrastructure is the same thing as the canonical protocol contract.

## Is the web client the source of truth?

No. `/docs` and the runtime contract remain authoritative. The web client is a reference product surface that is supposed to expose chain truth honestly and label materialized or ambiguous data clearly.

## Related

- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [First Steps](../getting-started/first-steps.en.md)
- [AAA System](../overview/aaa-system.en.md)
- [Physics-First vs Politics-First](../comparisons/physics-vs-politics.en.md)
- [Core Terms](../glossary/core-terms.en.md)

## Sources

- `README.md`
- `docs/README.md`
- `docs/manifesto.en.md`
- `docs/aaa.specification.en.md`
- `docs/governance.specification.en.md`
- `docs/staking.specification.en.md`
- `docs/read-model.contract.en.md`
- `docs/web-client.architecture.en.md`
