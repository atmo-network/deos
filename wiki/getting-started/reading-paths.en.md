---
page_type: getting-started
title: Reading Paths
summary: Self-contained role-based routes through the wiki for newcomers, economics work, runtime changes, governance work, client work, status checks, and tooling.
locale: en
canonical_page_id: reading-paths
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/README.md
  - ../../README.md
  - ../../BACKLOG.md
  - ../../CHANGELOG.md
  - ../../web-client/README.md
  - ../../template/README.md
  - ../../scripts/README.md
status: active
audience: newcomer
tags:
  - onboarding
  - reading-path
  - documentation
related:
  - Domain Map
  - First Steps
  - Generated Wiki
  - Development Status
  - DEOS Framework Overview
  - Core Terms
last_compiled: 2026-05-17
confidence: 0.91
---

# Reading Paths

## Summary

DEOS has several linked domains: framework identity, economic physics, autonomous actors, routing, governance, staking, read models, client, tooling, and future gates. You do not need to read everything in one pass.

Use the path that matches your task. Each path stays inside the wiki and ends with the concepts you need to understand before touching code, runtime parameters, or release notes.

## If you are completely new

1. [DEOS in 60 Seconds](deos-in-60-seconds.en.md)
2. [Who DEOS Is For](who-deos-is-for.en.md)
3. [Partner Pitch](partner-pitch.en.md)
4. [Executive Summary](executive-summary.en.md)
5. [Partner Evaluation Route](../usage/partner-evaluation-route.en.md)
6. [DEOS Framework Overview](../overview/deos-framework.en.md)
7. [Domain Map](../concepts/domain-map.en.md)
8. [Architecture Diagrams](../concepts/architecture-diagrams.en.md)
9. [Core Terms](../glossary/core-terms.en.md)
10. [End-to-End Flows](../concepts/end-to-end-flows.en.md)
11. [TMCTOL Standard](../concepts/tmctol-standard.en.md)
12. [Token-Driven Automation](../concepts/token-driven-automation.en.md)
13. [Newcomer FAQ](../faq/newcomer-faq.en.md)

This path gives you project vocabulary before pallet names, runtime details, or implementation-specific terms appear.

## If you are evaluating DEOS externally

1. [DEOS in 60 Seconds](deos-in-60-seconds.en.md)
2. [Who DEOS Is For](who-deos-is-for.en.md)
3. [Partner Pitch](partner-pitch.en.md)
4. [Executive Summary](executive-summary.en.md)
5. [Partner Evaluation Route](../usage/partner-evaluation-route.en.md)
6. [DEOS vs DAO Treasury](../comparisons/deos-vs-dao-treasury.en.md)
7. [What DEOS Is Not](../concepts/what-deos-is-not.en.md)
8. [TMCTOL Standard](../concepts/tmctol-standard.en.md)
9. [Economic Claim Levels](../concepts/economic-claim-levels.en.md)
10. [Threat Model](../concepts/threat-model.en.md)
11. [Minimal Fork Profile](../usage/minimal-fork-profile.en.md)
12. [Reference Client](../overview/reference-client.en.md)

This path is for partners, ecosystem readers, and technical evaluators who need the meme, boundaries, risk model, and fork obligations before reading implementation topology.

## If you are changing economics

1. [Domain Map](../concepts/domain-map.en.md)
2. [TMCTOL Standard](../concepts/tmctol-standard.en.md)
3. [TMCTOL Formulas](../math/tmctol-formulas.en.md)
4. [Economic Thresholds](../concepts/economic-thresholds.en.md)
5. [Economic Claim Levels](../concepts/economic-claim-levels.en.md)
6. [Invariant Map](../concepts/invariant-map.en.md)
7. [Threat Model](../concepts/threat-model.en.md)
8. [TOL Bucket Scenarios](../concepts/tol-bucket-scenarios.en.md)
9. [End-to-End Flows](../concepts/end-to-end-flows.en.md)
10. [Token Minting Curve](../overview/token-minting-curve.en.md)
11. [Axial Router](../overview/axial-router.en.md)
12. [Three-Layer Validation](../development/three-layer-validation.en.md)

Economics work must preserve the difference between formulas, runtime behavior, and integration effects. The wiki route should tell you which domain you are changing before you run the deeper validation stack.

## If you are changing runtime behavior

1. [Runtime Patterns](../overview/runtime-patterns.en.md)
2. [AAA System](../overview/aaa-system.en.md)
3. [End-to-End Flows](../concepts/end-to-end-flows.en.md)
4. [Asset Identity](../overview/asset-identity.en.md)
5. [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
6. [Read-Model Split](../concepts/read-model-split.en.md)
7. [Three-Layer Validation](../development/three-layer-validation.en.md)

Runtime work should identify the affected domain first, then validate whether the change is math-only, pallet-local, runtime-integrated, or client-visible.

## If you are changing governance

1. [Governance Overview](../overview/governance-overview.en.md)
2. [Governance Domains](../concepts/governance-domains.en.md)
3. [Physics-First vs Politics-First](../comparisons/physics-vs-politics.en.md)
4. [Staking Pools](../concepts/staking-pools.en.md)
5. [Read-Model Split](../concepts/read-model-split.en.md)
6. [Core Terms](../glossary/core-terms.en.md)

Governance work must keep constitutional protection, primary tracks, typed payload families, and execution authority separate.

## If you are changing the web client

1. [Reference Client](../overview/reference-client.en.md)
2. [UI Kit and Domain DAG](../concepts/ui-kit-and-domain-dag.en.md)
3. [Read-Model Split](../concepts/read-model-split.en.md)
4. [Generated Wiki](../concepts/generated-wiki.en.md)
5. [Scripts Layer](../usage/scripts-layer.en.md)
6. [Development Status](../development/status.en.md)

Client work should preserve UI Kit reuse, Domain DAG ownership, read-model provenance, and the trusted wiki rendering boundary.

## If you are checking current status or release history

1. [Development Status](../development/status.en.md)
2. [Domain Map](../concepts/domain-map.en.md)
3. [Newcomer FAQ](../faq/newcomer-faq.en.md)
4. [Generated Wiki](../concepts/generated-wiki.en.md)
5. [Core Terms](../glossary/core-terms.en.md)

Status work should separate shipped baseline, open backlog, completed delivery, and future-gated work. The wiki explains that boundary without becoming release notes itself.

## If you are operating scripts or local tooling

1. [Scripts Layer](../usage/scripts-layer.en.md)
2. [Three-Layer Validation](../development/three-layer-validation.en.md)
3. [Runtime Patterns](../overview/runtime-patterns.en.md)
4. [Development Status](../development/status.en.md)
5. [Repository Structure](../implementation/repository-structure.en.md)
6. [Tech Stack](../implementation/tech-stack.en.md)
7. [Parachain Context](../concepts/parachain-context.en.md)
8. [Forking DEOS](../usage/forking-deos.en.md)
9. [Minimal Fork Profile](../usage/minimal-fork-profile.en.md)
10. [What DEOS Is Not](../concepts/what-deos-is-not.en.md)

Tooling and fork work should stay bounded, explicit, and honest about prerequisites, preserved framework contracts, and behavior.

## Related

- [DEOS in 60 Seconds](deos-in-60-seconds.en.md)
- [Who DEOS Is For](who-deos-is-for.en.md)
- [Partner Pitch](partner-pitch.en.md)
- [Executive Summary](executive-summary.en.md)
- [Partner Evaluation Route](../usage/partner-evaluation-route.en.md)
- [DEOS vs DAO Treasury](../comparisons/deos-vs-dao-treasury.en.md)
- [Domain Map](../concepts/domain-map.en.md)
- [First Steps](first-steps.en.md)
- [Generated Wiki](../concepts/generated-wiki.en.md)
- [Development Status](../development/status.en.md)
- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [Core Terms](../glossary/core-terms.en.md)
