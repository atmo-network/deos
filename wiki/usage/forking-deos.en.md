---
page_type: usage
title: Forking DEOS
summary: A practical map of what a downstream team changes, preserves, and validates when forking DEOS into a concrete ecosystem.
locale: en
canonical_page_id: forking-deos
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../README.md
  - ../../docs/README.md
  - ../../template/pallets/aaa/EMBEDDING.md
  - ../../docs/framework-instance.contract.en.md
  - ../../template/README.md
  - ../../web-client/README.md
  - ../../docs/tmctol.specification.en.md
status: active
audience: developer
tags:
  - usage
  - forkability
  - framework
  - downstream
related:
  - DEOS Framework Overview
  - Repository Structure
  - Tech Stack
  - Token Surfaces
  - Three-Layer Validation
last_compiled: 2026-07-21
confidence: 0.85
---

# Forking DEOS

## Summary

DEOS is meant to be forked by teams launching concrete ecosystems. A fork should keep the reusable framework contracts clear while replacing the ecosystem-specific product, token, governance, and operator choices.

The rule of thumb: change identity and policy; preserve bounded mechanics and validation discipline. DEOS provides mechanisms, not a mandatory moral or business configuration for every instance.

## What Usually Changes

A downstream fork normally defines:

- Chain identity, branding, endpoints, bootnodes, and operator runbooks;
- Token names, ticker presentation, launch allocation, founder economics, and product narrative;
- Concrete governance domains, protection distribution, labor culture, invoice norms, and bootstrap handoff plan;
- Ecosystem product surfaces, dApps, portfolio/indexer needs, demand strategy, and materialized providers;
- Deployment parameters, collator/operator assumptions, and monitoring setup;
- Client copy, default endpoints, wallet presets, and user-facing flows.

These are product and ecosystem decisions. They should not silently leak back into DEOS as hardcoded framework assumptions.

## Mechanism vs Policy

DEOS-owned surfaces are reusable mechanisms and safety contracts: primitives, invariants, bounded execution, AAA task language, governance/protection mechanisms, read-model provenance, configuration seams, validation gates, and reference patterns.

Instance-owned surfaces are concrete policy: brand, dApps, launch economics, founder allocation or no-founder-allocation choices, contributor culture, invoice etiquette, bucket names/percentages, marketing, and demand strategy.

Builder invoices, bucketed capital flows, protocol-owned liquidity, governance protection, and System AAA topologies are mechanisms. A fork may keep, rename, reparameterize, disable, or replace policy choices as long as it preserves the guarantees it claims.

## What Should Stay Stable

A fork should preserve the core framework discipline unless it has strong evidence to change it:

- Deterministic protocol-managed economic reactions;
- Bounded runtime read surfaces versus materialized/indexed views;
- Explicit AAA actor roles and execution-plan boundaries;
- TMCTOL math validation before runtime changes;
- Governance domain/protection separation;
- Staking share-vault and receipt accounting invariants;
- Zero-warning runtime/client hygiene and trust validation for wiki content.

If a fork changes these mechanics, it is no longer only rebranding DEOS. It is changing the framework contract and should validate at the economic, runtime, and integration layers.

## Minimum Fork Profile

A DEOS fork should not launch by only changing names. Before a downstream team treats the fork as launch-ready, it needs explicit choices for the economic, governance, runtime, client, and operator profile that turns the framework into a concrete ecosystem.

| Area | Minimum fork decision |
| --- | --- |
| Native asset | Name, ticker, decimals, allocation, role in staking/governance |
| Foreign collateral set | Which assets can be registered, routed, or used as collateral |
| TMC curve params | Initial price, slope, supply assumptions, launch immutability policy |
| TOL distribution | Bucket split, paired treasuries, reserve/lane semantics |
| Bucket policies | Names, percentages, actor wakeups, thresholds, retries, treasury lanes, and spend priorities |
| Router fee | Fee bounds, burn/sink routing, governance mutability |
| Governance domain pairs | Primary/protection tokens, payload kinds, cadence, execution authority |
| Staking receipt policy | Receipt namespaces, native receipt, LP custody, reward paths |
| Materialized provider policy | Which user flows need indexers or archive/search providers |
| Collator/randomness posture | Trusted phase, upgrade path, relay/protocol randomness dependency |
| Client/product surface | Default endpoints, wallet presets, copy, dApps, risk wording |
| Validation baseline | Simulator, runtime tests, client validation, wiki trust, operator smoke checks |

If a row is undecided, the fork is still a prototype. If a row is decided but not validated, the fork is not launch-ready.

## Fork Checklist

1. Rename public identity without renaming TMCTOL-specific standard concepts blindly.
2. Decide which assets and governance surfaces are ecosystem-specific.
3. Set launch parameters and treat launch physics as immutable unless a stronger constitutional contract says otherwise.
4. Review System AAA actor roles and remove assumptions that only fit the reference ecosystem.
5. Treat builder invoices and bucket policies as opt-in mechanisms: decide what your instance enables, renames, funds, or disables.
6. For AAA reuse, implement host-runtime adapters and keep business policy in adapters or genesis actor configuration, not in `pallet-aaa` core.
7. Classify every client datum as direct on-chain projection or materialized/indexed view.
8. Update scripts, metadata export, endpoints, and operator documentation.
9. Run the smallest meaningful validation first, then escalate when math/runtime/client boundaries cross.

## What Can Flow Back Upstream

Good upstream contributions are framework-hardening changes: tests, client read-model honesty, safer scripts, clearer docs/wiki, better adapter boundaries, benchmark fixes, and bug fixes in reusable pallets.

Downstream-specific business logic, dApp behavior, token narrative, and ecosystem policy should usually stay in the fork.

## Related

- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [Repository Structure](../implementation/repository-structure.en.md)
- [Tech Stack](../implementation/tech-stack.en.md)
- [Parachain Context](../concepts/parachain-context.en.md)
- [Token Surfaces](../concepts/token-surfaces.en.md)
- [Three-Layer Validation](../development/three-layer-validation.en.md)
