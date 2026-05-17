---
page_type: overview
title: DEOS Framework Overview
summary: DEOS is a forkable economic runtime framework for sovereign ecosystems. It supplies deterministic economic services such as minting, routing, automation, staking, governance, and read-model discipline, while TMCTOL is the current standard running on top of it.
locale: en
canonical_page_id: deos-framework
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../README.md
  - ../../docs/README.md
  - ../../docs/core.architecture.en.md
status: active
audience: newcomer
tags:
  - overview
  - architecture
  - deos
related:
  - First Steps
  - AAA System
  - AA-Actor
  - Axial Router
  - Token Minting Curve
  - Governance Overview
  - Asset Identity
  - Randomness Strategy
  - Runtime Patterns
  - TMCTOL Standard
  - Token-Driven Automation
  - Routing and Minting Loop
  - Reference Client
  - Core Terms
last_compiled: 2026-04-16
confidence: 0.95
---

# DEOS Framework Overview

## Summary

DEOS stands for `Deterministic Economic Operating System`. It is a forkable runtime framework for programmable economies, not the finished downstream product.

The one-line model: DEOS connects token issuance, protocol-owned liquidity, routing, staking, governance, and automated actors into a deterministic institutional machine. In this repository, the current flagship configuration of that framework is the TMCTOL standard.

## What DEOS Means

DEOS is best understood as an economic execution substrate:

- `Deterministic` means protocol-managed reactions should be explicit and repeatable for the same on-chain state
- `Economic` means the framework focuses on capital, liquidity, staking, governance-conditioned flows, and treasury mechanics
- `Operating System` means the repository ships coordinated runtime services rather than one isolated token primitive

That is why the top-level project identity stays `DEOS`, while `TMCTOL` remains the current standard running on top of it.

## Framework vs Standard

The repository separates two layers:

- `DEOS` = the framework, reference stack, runtime substrate, docs, client, and tooling
- `TMCTOL` = the current tokenomic standard with a minting curve, treasury-owned liquidity, fee burning, and bucketed policy

That distinction matters because DEOS is meant to be forked by downstream ecosystems that may keep the framework while changing the product thesis or economic configuration.

## Main Stack Layers

The current reference stack is organized around four practical surfaces:

- `/docs` for normative contracts and architecture notes
- `/template` for the runtime kernel and pallets
- `/web-client` for the browser-facing reference client
- `/scripts` for local operator and validation automation

The `/simulator` remains the authoritative executable math surface when tokenomics or invariants change, but it is no longer the default entry point for unrelated maintenance.

## Economic Coordination Model

DEOS uses token-driven coordination instead of request-response administration. The framework tries to express economic behavior as bounded transitions driven by balances, schedules, and typed payloads.

In the current line, that model appears through:

- TMC for deterministic mint-side issuance
- The Axial Router for route selection and fee burning
- AAA for deterministic execution infrastructure
- Staking and governance for bounded social control surfaces
- An explicit read-model split between canonical on-chain projections and materialized views

## Related

- [First Steps](../getting-started/first-steps.en.md)
- [Forking DEOS](../usage/forking-deos.en.md)
- [AAA System](aaa-system.en.md)
- [AA-Actor](aa-actor.en.md)
- [Axial Router](axial-router.en.md)
- [Token Minting Curve](token-minting-curve.en.md)
- [Governance Overview](governance-overview.en.md)
- [Asset Identity](asset-identity.en.md)
- [Randomness Strategy](randomness-strategy.en.md)
- [Runtime Patterns](runtime-patterns.en.md)
- [TMCTOL Standard](../concepts/tmctol-standard.en.md)
- [Token-Driven Automation](../concepts/token-driven-automation.en.md)
- [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
- [Reference Client](reference-client.en.md)
- [Core Terms](../glossary/core-terms.en.md)
