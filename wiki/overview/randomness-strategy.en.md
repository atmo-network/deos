---
type: overview
title: Randomness Strategy
description: DEOS has no local VRF pallet or probabilistic Actors triggers. Actors timers use deterministic cadence, and the runtime uses a trusted collator set.
locale: en
canonical_page_id: randomness-strategy
translation_status: source
available_locales:
  - en
  - ru
sources:
  - resource: ../../docs/core.architecture.en.md
  - resource: ../../docs/manifesto.en.md
status: stable
audience: newcomer
tags:
  - overview
  - randomness
  - launch-line
related:
  - Physics-First vs Politics-First
  - Governance
  - AA-Actor
  - Newcomer FAQ
---

# Randomness Strategy

## Summary

Randomness is not a first-class product story in the current DEOS launch line. The docs treat it as a secondary infrastructure concern and deliberately simplify it.

The old local `pallet-vrf` line was retired. Actors now exposes deterministic timer cadence only and performs no probability sampling or hash fallback.

## Current Position

The current runtime line assumes:

- No Local Randomness Pallet
- No Local Entropy-Provider Economy
- No Permissionless-Collator Activation Yet
- Trusted Invulnerable Collators On The Launch Line
- Deterministic Actors Timers With No Entropy Dependency

This is intentionally framed as an honest simplification rather than a hidden claim of strong permissionless fairness.

## Why the VRF Line Was Removed

The docs explain that the local VRF path carried too much protocol-owned complexity for the current product needs. Same-block fairness is no longer required, and the project no longer wants to maintain a second local entropy economy just to preserve optional cryptographic ambition.

## Current Randomness Boundary

The runtime does not use a relay-provided per-block randomness beacon. Epoch-scale relay randomness does not satisfy its per-block entropy contract.

The trusted collator set is an explicit limitation, not evidence of permissionless fairness.

## Why This Matters for Governance and AA-Actor

Randomness simplification narrows the launch contract:

- Actors Has No Probability Gate In The Current Contract
- Governance Does Not Need To Carry A Second Entropy Economy
- Permissionless Collators Are Not Part Of The Current Runtime Baseline

## Related

- [Physics-First vs Politics-First](../comparisons/physics-vs-politics.en.md)
- [Governance](governance.en.md)
- [AA-Actor](actor.en.md)
- [Newcomer FAQ](../faq/newcomer-faq.en.md)
