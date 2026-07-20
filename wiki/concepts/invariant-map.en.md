---
page_type: concept
title: Invariant and Threat Map
summary: A compact map of core DEOS/TMCTOL invariants, threat shapes, owner surfaces, validation routes, governance mutability, and failure modes.
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
  - ../../docs/governance.specification.en.md
  - ../../docs/web-client.architecture.en.md
status: active
audience: developer
tags:
  - concept
  - invariants
  - validation
  - governance
  - threat-model
  - security
related:
  - Economic Claim Levels
  - Three-Layer Validation
  - TMCTOL Standard
  - What DEOS Is Not
  - Governance Domains
last_compiled: 2026-07-20
confidence: 0.85
---

# Invariant and Threat Map

## Summary

This page maps important DEOS/TMCTOL invariants and threats to their owner surface, validation route, governance mutability, and main failure mode. It is intentionally denser than a narrative overview.

| Invariant                   | Owner       | Validation            | Governance           | Failure                |
|---|---|---|---|---|
| TMC integral pricing        | TMC         | simulator + tests     | no post-launch       | wrong mint price       |
| Unidirectional minting      | TMC         | pallet + runtime      | no                   | reserve extraction     |
| Router fee capture + burn flow | Router + AAA | runtime + bench    | bounded config       | noncanonical path or actor liveness |
| AAA bounded work            | AAA         | bench + tests         | typed authority      | overweight/stuck graph |
| TOL bucket topology         | TOL + AAA   | sim + runtime         | explicit plans       | provenance or activation drift |
| Asset identity bijection    | Registry    | runtime tests         | register/update only | identity drift         |
| Staking share accounting    | Staking     | pallet + runtime      | no override          | receipt dilution       |
| Governance domain authority | Governance  | tests + review        | explicit policy      | authority creep        |
| Read-model honesty          | Client/docs | DAG + wiki + validate | no                   | false chain truth      |
| Wiki trust boundary         | Wiki/client | trust validator       | no                   | unsafe rendering       |

## Central Threats

| Threat | Shape | Mitigation | Owner |
|---|---|---|---|
| Reserve commitment weakened | authorized policy redirects or unwinds reserves | typed domain authority, protection, and explicit instance policy | governance/spec |
| Noncanonical swap path | lower-level conversion avoids router fee flow | canonical client gateway, explicit scope, and runtime audits; not a universal call barrier | router/runtime |
| Bucket misuse | provenance or activation state collapses | segmented accounts, explicit plans, and readiness gates | TMCTOL + AAA |
| Indexer truth confusion | archive shown as truth | provenance badges | client/docs |
| Collator trust phase | trust mistaken as permissionless | launch-line constraint | runtime/ops |
| Actor graph stuck | cooldown/outage/oracle gap | retry + cooldown | AAA |
| Parameter griefing | params leave assumptions | bounded settings | runtime/gov |
| LP valuation attack | LP overvalued/double-counted | conservative custody | staking/gov |
| Frontend provenance lie | UI hides data class | read-model contract | web client |

## Use

When a change touches one of these rows, validate the owner surface first and then escalate to the next layer if the change crosses math, runtime, client, or governance boundaries.

A threat is not “solved” just because a page mentions it. It is controlled only when the owner surface has a bounded mechanism and validation route.

## Related

- [Economic Claim Levels](economic-claim-levels.en.md)
- [Three-Layer Validation](../development/three-layer-validation.en.md)
- [TMCTOL Standard](tmctol-standard.en.md)
- [What DEOS Is Not](what-deos-is-not.en.md)
- [Governance Domains](governance-domains.en.md)
