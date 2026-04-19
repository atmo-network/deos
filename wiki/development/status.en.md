---
page_type: status
title: Development Status
summary: Current implementation status, roadmap context, and active backlog items for the DEOS framework.
locale: en
canonical_page_id: status
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../BACKLOG.md
status: active
audience: newcomer
tags:
  - development
  - status
  - roadmap
related:
  - Three-Layer Validation
  - Contributing Guidelines
last_compiled: 2026-04-15
confidence: 0.95
---

# Development Status

## Summary

The DEOS framework is in active development, with a shipped reference baseline that already supports multi-asset staking, Account Abstraction Actors (AAA), and the core TMCTOL economic loop. The current focus is on hardening the governance surfaces, improving the reference web client, and deferring non-critical features until upstream dependencies are ready.

## Current Shipped Baseline

The local network and reference client currently ship with:

- **Core TMCTOL Loop**: Unidirectional minting, treasury-owned liquidity routing, and fee burning.
- **AAA Substrate**: Deterministic actors for liquidity provisioning, burning, and treasury logic.
- **Staking**: Multi-asset share-vault staking with `stXXX` receipts and native `$NTVE` binding.
- **Governance Foundation**: Bounded memory, proposal lifecycle, invoice-voting capabilities (`Amplify / Approve / Reduce / Nay`), and the primary/protection dual-track hierarchy.
- **Web Client**: On-chain-first wallet, swap, and basic governance viewing widgets built on SvelteKit.

## Active Focus: Governance v1

The immediate roadmap (tracked in `BACKLOG.md`) centers on rolling out the full **Governance v1** contract:

- Retargeting ordinary and fast-track public referendum cadences.
- Binding execution authority to explicit payload kinds (`L1RootAction`, `L2TreasurySpend`, `L2ParameterChange`, `Intent`, `L2SignalToL1`).
- Polishing the web client's governance product UX for proposal semantics and execution state.

## Externally Gated Work (Deferred)

Some features are intentionally deferred, waiting for the upstream Polkadot ecosystem:

- **Randomness / Relay Beacon**: The local `pallet-vrf` was retired. Permissionless collators and advanced cryptographic randomness are on hold until a real parachain-consumable per-block protocol beacon (like Safrole/Sassafras) is shipped by the Polkadot SDK. Currently, a trusted invulnerable collator set plus previous-block-hash fallback is used.

## Finding the Backlog

For real-time tracking of what's implemented and what's next, developers should consult the root [`BACKLOG.md`](../../BACKLOG.md) and [`CHANGELOG.md`](../../CHANGELOG.md) files in the repository.
