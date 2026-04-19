---
page_type: getting-started
title: First Steps
summary: Start with the docs hub, understand the framework-versus-standard split, then choose the right working surface for your task. Use the simulator for tokenomics, the runtime workspace for implementation, the web client for product flows, and scripts for bounded local operations.
locale: en
canonical_page_id: first-steps
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../README.md
  - ../../docs/README.md
  - ../../docs/read-model.contract.en.md
status: active
audience: newcomer
tags:
  - getting-started
  - onboarding
  - workflow
related:
  - DEOS Framework Overview
  - TMCTOL Standard
  - Read-Model Split
  - Reference Client
  - Newcomer FAQ
last_compiled: 2026-04-16
confidence: 0.93
---

# First Steps

## Summary

If you are new to the repository, start with the conceptual layer before touching code. The docs explain what the system is supposed to do. The runtime, client, and scripts are implementations of that contract.

A simple first pass is: understand DEOS, understand TMCTOL, then move into the specific subsystem you need.

## Recommended Reading Order

1. Read [`docs/README.md`](../../docs/README.md)
2. Read the [DEOS Framework Overview](../overview/deos-framework.en.md)
3. Read the [TMCTOL Standard](../concepts/tmctol-standard.en.md)
4. Read the [Token-Driven Automation](../concepts/token-driven-automation.en.md)
5. Read the [Read-Model Split](../concepts/read-model-split.en.md)
6. Then open the exact subsystem spec or architecture note you need

## Pick the Right Surface

### When to use `/docs`

Use `/docs` first when you need definitions, rationale, architecture maps, and the intended contract of a subsystem.

### When to use `/simulator`

Use `/simulator` when tokenomics, formulas, thresholds, or invariants are changing or being rechecked.

### When to use `/template`

Use `/template` when you are implementing or validating runtime behavior, pallets, integrations, tests, or benchmarks.

### When to use `/web-client`

Use `/web-client` when you are working on the browser-facing reference product and how it presents chain truth to users.

### When to use `/scripts`

Use `/scripts` for bounded local setup, probes, operator checks, and repository automation.

## Core Mental Model

Three ideas explain most of the repository:

- DEOS is the framework
- TMCTOL is the current economic standard running on top of that framework
- User-facing data must be classified as either canonical on-chain truth or a materialized view

## Validation Mindset

Use the smallest meaningful validation layer first:

- Math changes -> simulator
- Runtime changes -> targeted cargo validation
- Client changes -> targeted web-client checks
- Docs and wiki changes -> terminology, link, and navigation sanity

## Related

- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [TMCTOL Standard](../concepts/tmctol-standard.en.md)
- [Read-Model Split](../concepts/read-model-split.en.md)
- [Reference Client](../overview/reference-client.en.md)
- [Newcomer FAQ](../faq/newcomer-faq.en.md)

## Sources

- `README.md`
- `docs/README.md`
- `docs/read-model.contract.en.md`
