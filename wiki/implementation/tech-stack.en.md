---
page_type: implementation
title: Tech Stack
summary: Overview of the underlying technologies that power the DEOS framework, including Polkadot SDK and SvelteKit.
locale: en
canonical_page_id: tech-stack
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/polkadot-sdk-2603.insights.en.md
  - ../../docs/core.architecture.en.md
status: active
audience: developer
tags:
  - implementation
  - tech-stack
  - architecture
related:
  - Repository Structure
  - Runtime Patterns
last_compiled: 2026-04-15
confidence: 0.95
---

# Tech Stack

## Summary

The DEOS framework is built on top of modern blockchain and web technologies. The core runtime leverages the Polkadot SDK (formerly Substrate), while the frontend uses SvelteKit for high-performance reactive interfaces.

## Blockchain Layer (L1)

### Polkadot SDK

DEOS is built as a Parachain runtime using the Polkadot SDK. It follows the modern `Polkadot SDK 2603` baseline, rather than outdated Substrate patterns.

- **Language**: Rust
- **Macro System**: `frame::v2` for strongly typed pallet definitions
- **Benchmarking**: `frame_benchmarking::v2`
- **Execution**: WebAssembly (Wasm) runtime

### Omni Node Architecture

DEOS uses the Omni Node deployment architecture. Instead of maintaining a custom boilerplate node repository, the runtime is executed via a standardized Omni Node binary provided by the Polkadot ecosystem.

### XCM (Cross-Consensus Messaging)

Foreign assets are integrated via XCM v5, mapped through the internal Asset Registry to create stable local `AssetId` values.

## Simulation Layer

The economic proving ground (`/simulator`) is written in standard **JavaScript** with heavy use of `BigInt` to ensure math correctness before Rust implementation.

## Reference Client Layer

The DEOS Reference Client is built to be lightweight, reactive, and strictly on-chain-first.

- **Framework**: SvelteKit
- **Language**: TypeScript
- **State Management**: Reactive stores for bounded on-chain data

## Automation and Tooling

- **Scripts**: Standard Bash (`.sh`) for operational workflows
- **AI Coordination**: Local markdown-based prompts and Bash-based execution skills (`.agents/skills/`)

## How to Use This Page

Use this page as the implementation stack map after you know which domain you are changing. It tells you which technology boundary you are entering before you choose validation depth or edit a repository surface.

## Related

- [Repository Structure](repository-structure.en.md)
- [Runtime Patterns](../overview/runtime-patterns.en.md)
- [Three-Layer Validation](../development/three-layer-validation.en.md)
- [Reference Client](../overview/reference-client.en.md)
- [Scripts Layer](../usage/scripts-layer.en.md)
- [Agent Coordination](../usage/agent-coordination.en.md)
