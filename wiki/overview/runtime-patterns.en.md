---
page_type: overview
title: Runtime Patterns
summary: DEOS follows modern Polkadot SDK runtime patterns rather than older Substrate-era habits. The docs emphasize template-faithful runtime wiring, async-backing defaults, Omni Node deployment, and configuration discipline as part of the framework contract.
locale: en
canonical_page_id: runtime-patterns
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/core.architecture.en.md
  - ../../template/README.md
  - ../../template/Cargo.toml
  - ../../template/runtime/src/lib.rs
status: active
audience: newcomer
tags:
  - overview
  - runtime
  - polkadot-sdk
related:
  - DEOS Framework Overview
  - Asset Identity
  - Randomness Strategy
  - Core Terms
last_compiled: 2026-07-20
confidence: 0.9
---

# Runtime Patterns

## Summary

DEOS is not just a set of economic ideas. It is also a modern Polkadot SDK runtime that follows current platform patterns instead of stale Substrate assumptions.

The repository docs explicitly treat several SDK-era patterns as hard requirements for a healthy parachain baseline.

## Current Runtime Evidence

The current workspace and runtime assembly make the platform posture concrete:

- The workspace tracks the Polkadot SDK `2606` crate line;
- Pallets use FRAME v2 typed configuration and runtime-owned adapters;
- Omni Node replaces a repository-owned custom node crate;
- XCM, asset identity, runtime APIs, and weights remain explicit runtime integration surfaces;
- V3 scheduling stays disabled until its separate readiness gate is satisfied.

For DEOS, these are not abstract style notes. They define the current platform boundary and validation obligations.

## Why This Belongs in the Wiki

A newcomer reading only the economic docs could miss an important point: the framework is intentionally built on top of current parachain architecture patterns, not just on custom pallet ideas.

That affects how contributors should reason about runtime assembly, validation, and upgrade safety.

## Relationship to the Economic Stack

These runtime patterns are infrastructure, but they support the whole economic layer:

- Collator/runtime wiring must be correct for the chain to produce blocks
- Asset integration patterns affect foreign-asset support
- Consensus and async-backing posture affect how the runtime operates under the current launch line
- Omni Node keeps deployment closer to the current upstream standard

## Related

- [DEOS Framework Overview](deos-framework.en.md)
- [Asset Identity](asset-identity.en.md)
- [Randomness Strategy](randomness-strategy.en.md)
- [Core Terms](../glossary/core-terms.en.md)
