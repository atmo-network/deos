---
page_type: status
title: Development Status
summary: Current implementation status, roadmap context, and active backlog items for the DEOS framework. The current baseline has closed the native staking/liquidity/governance migration and is now biased toward governance v1 completion, web-client polish, and future-gated extensions only when policy or upstream dependencies justify them.
locale: en
canonical_page_id: status
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../BACKLOG.md
  - ../../CHANGELOG.md
status: active
audience: newcomer
tags:
  - development
  - status
  - roadmap
related:
  - Three-Layer Validation
  - Contributing Guidelines
last_compiled: 2026-04-25
confidence: 0.96
---

# Development Status

## Summary

DEOS is in active framework stabilization. The reference baseline already ships the TMCTOL loop, AAA, multi-asset staking, bounded governance, runtime benchmark discipline, local/operator scripts, and a SvelteKit reference client.

The current emphasis is crystallization: make the shipped contract easier to operate and understand, finish the narrow governance v1 rollout surface, and keep future extensions gated by real product pressure or upstream readiness.

## Current Shipped Baseline

The local network and reference client currently ship with:

- **Core TMCTOL Loop**: Unidirectional minting, treasury-owned liquidity routing, fee burning, and bounded economic invariants
- **AAA Substrate**: Deterministic actors for burning, liquidity provisioning, bucket/treasury logic, and native-staking LP donation, with portable generic staking tasks
- **Staking**: Multi-asset share-vault staking with `stXXX` receipts, native `$NTVE -> stNTVE` liquid staking, locked `NTVE/stNTVE` LP nomination, native governance custody, and native nomination reward settlement
- **Governance Foundation**: Domain-scoped primary/protection tracks, public cadence, typed payload kinds, invoice voting, runtime-upgrade authorization, bounded execution details, and governance reward-memory
- **Web Client**: On-chain-first wallet, swap, staking, governance, wiki, and execution-feedback surfaces built on SvelteKit
- **Operator Tooling**: Runtime metadata export, benchmark/weight generation, local bootstrap, authorized-upgrade helpers, and native staking bootstrap readiness/call-preparation helpers

## Recently Stabilized

The native staking/liquidity/governance migration baseline is closed on the current line. The shipped contract now treats `stNTVE` as a liquid receipt, moves collator security to locked `NTVE/stNTVE` LP, keeps native reward settlement on native-specific paths, and includes plan-only operator tooling for canonical pool bootstrap.

AAA staking was also narrowed back to a portable contract: generic `Stake { asset, amount }` and `Unstake { asset, shares }` tasks remain in AAA, while DEOS-native staking routing and nomination policy live in runtime adapters plus staking/governance pallets.

## Active Focus: Governance v1

The immediate roadmap centers on finishing the smallest honest **Governance v1** rollout surface:

- Keep execution authority bound to explicit domains, cadence modes, and payload kinds
- Add only genuinely delegated/domain-owned `L2ParameterChange` surfaces beyond the current Axial Router pair
- Improve execution-side observability only when new payload families or failure states ship
- Continue web-client governance UX only where proposal semantics or execution state need clearer product composition

## Future-Gated Work

Some work is intentionally not part of the current launch baseline:

- **LP Donation Acquisition**: The Native Staking LP Farmer already supports deterministic `$NTVE` acquisition into balanced `NTVE/stNTVE` donation. Swap/mixed-route acquisition remains future-gated until AAA policy needs route comparison, slippage bounds, and fallback behavior.
- **Randomness / Relay Beacon**: Permissionless collators and advanced randomness remain deferred until a real parachain-consumable per-block protocol beacon exists upstream. The current line uses trusted invulnerable collators plus deterministic previous-block-hash fallback where needed.
- **Materialized Archive UX**: The browser should not stretch bounded on-chain retention into archive/search features. Those belong to a future materialized provider contract.

## Finding the Backlog

For real-time tracking of what is implemented and what remains open, developers should consult the root [`BACKLOG.md`](../../BACKLOG.md) and [`CHANGELOG.md`](../../CHANGELOG.md) files.

## Related

- [Three-Layer Validation](three-layer-validation.en.md)
- [Contributing Guidelines](../community/contributing.en.md)

## Sources

- `BACKLOG.md`
- `CHANGELOG.md`
