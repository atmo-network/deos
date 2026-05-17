---
page_type: concept
title: Invariant Map
summary: A compact map of core DEOS/TMCTOL invariants, ownership, validation surfaces, governance mutability, and failure modes.
locale: en
canonical_page_id: invariant-map
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../docs/tmctol.specification.en.md
  - ../../docs/core.architecture.en.md
  - ../../docs/axial-router.architecture.en.md
  - ../../docs/aaa.specification.en.md
status: active
audience: developer
tags:
  - concept
  - invariants
  - validation
  - governance
related:
  - Threat Model
  - Economic Claim Levels
  - Three-Layer Validation
  - TMCTOL Standard
last_compiled: 2026-05-17
confidence: 0.84
---

# Invariant Map

## Summary

This page maps important DEOS/TMCTOL invariants to their owner, validation surface, governance mutability, and main failure mode. It is intentionally denser than a narrative overview.

| Invariant                   | Owner       | Validation            | Governance           | Failure                |
| --------------------------- | ----------- | --------------------- | -------------------- | ---------------------- |
| TMC integral pricing        | TMC         | simulator + tests     | no post-launch       | wrong mint price       |
| Unidirectional minting      | TMC         | pallet + runtime      | no                   | reserve extraction     |
| Router fee burn/split       | Router      | runtime + bench       | bounded              | fee bypass             |
| AAA bounded work            | AAA         | bench + tests         | no bypass            | overweight/stuck graph |
| TOL bucket split            | TOL + AAA   | sim + runtime         | maybe bounded        | bucket misuse          |
| Asset identity bijection    | Registry    | runtime tests         | register/update only | identity drift         |
| Staking share accounting    | Staking     | pallet + runtime      | no override          | receipt dilution       |
| Governance domain authority | Governance  | tests + review        | explicit policy      | authority creep        |
| Read-model honesty          | Client/docs | DAG + wiki + validate | no                   | false chain truth      |
| Wiki trust boundary         | Wiki/client | trust validator       | no                   | unsafe rendering       |

## Use

When a change touches one of these rows, validate the owner surface first and then escalate to the next layer if the change crosses math, runtime, client, or governance boundaries.

## Related

- [Threat Model](threat-model.en.md)
- [Economic Claim Levels](economic-claim-levels.en.md)
- [Three-Layer Validation](../development/three-layer-validation.en.md)
- [TMCTOL Standard](tmctol-standard.en.md)
