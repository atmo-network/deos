---
type: overview
title: Randomness Strategy
description: DEOS keeps randomness outside the current Actors contract. The local VRF line was retired, Actors timers now use deterministic cadence only, and any future probabilistic trigger requires a real financially secure entropy contract.
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

The old local `pallet-vrf` line was retired. Actors now exposes deterministic timer cadence only and performs no probability sampling or hash fallback. The preferred long-term randomness direction remains a real relay-provided beacon for consumers that can justify it.

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

## Preferred Future Direction

The preferred future is not “rebuild a better local randomness market.” It is “adopt a real relay beacon if the relay ecosystem eventually exposes a parachain-consumable per-block protocol beacon with a stable production contract.”

Until that exists, the project explicitly refuses to pretend that currently visible epoch-scale relay randomness items solve the product problem.

## Why This Matters for Governance and AA-Actor

Randomness simplification narrows the launch contract:

- Actors Has No Probability Gate In The Current Contract
- Governance Does Not Need To Carry A Second Entropy Economy
- Permissionless Collator Expansion Stays Gated Behind A Stronger Future Randomness Contract

## Related

- [Physics-First vs Politics-First](../comparisons/physics-vs-politics.en.md)
- [Governance](governance.en.md)
- [AA-Actor](actor.en.md)
- [Newcomer FAQ](../faq/newcomer-faq.en.md)
