---
page_type: overview
title: Randomness Strategy
summary: DEOS currently uses a deliberately simplified randomness posture. The local VRF line was retired, trusted-collator previous-block-hash fallback is accepted for the current launch phase, and a future relay-beacon replacement is only acceptable if it becomes a real parachain-consumable per-block protocol beacon.
locale: en
canonical_page_id: randomness-strategy
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/randomness.strategy.en.md
  - ../../docs/manifesto.en.md
status: active
audience: newcomer
tags:
  - overview
  - randomness
  - launch-line
related:
  - Physics-First vs Politics-First
  - Governance Overview
  - AA-Actor
  - Newcomer FAQ
last_compiled: 2026-07-20
confidence: 0.95
---

# Randomness Strategy

## Summary

Randomness is not a first-class product story in the current DEOS launch line. The docs treat it as a secondary infrastructure concern and deliberately simplify it.

The old local `pallet-vrf` line was retired. The current contract accepts deterministic previous-block-hash sampling only in the trusted-collator launch phase, while the preferred long-term direction is a real relay-provided beacon.

## Current Position

The current runtime line assumes:

- No Local Randomness Pallet
- No Local Entropy-Provider Economy
- No Permissionless-Collator Activation Yet
- Trusted Invulnerable Collators On The Launch Line
- Previous-Block-Hash Fallback For Local Probabilistic Consumers

This is intentionally framed as an honest simplification rather than a hidden claim of strong permissionless fairness.

## Why the VRF Line Was Removed

The docs explain that the local VRF path carried too much protocol-owned complexity for the current product needs. Same-block fairness is no longer required, and the project no longer wants to maintain a second local entropy economy just to preserve optional cryptographic ambition.

## Preferred Future Direction

The preferred future is not “rebuild a better local randomness market.” It is “adopt a real relay beacon if the relay ecosystem eventually exposes a parachain-consumable per-block protocol beacon with a stable production contract.”

Until that exists, the project explicitly refuses to pretend that currently visible epoch-scale relay randomness items solve the product problem.

## Why This Matters for Governance and AA-Actor

Randomness simplification narrows the launch contract:

- AAA Probability Gates Are Convenience Mechanisms, Not Strong Fairness Claims
- Governance Does Not Need To Carry A Second Entropy Economy
- Permissionless Collator Expansion Stays Gated Behind A Stronger Future Randomness Contract

## Related

- [Physics-First vs Politics-First](../comparisons/physics-vs-politics.en.md)
- [Governance Overview](governance-overview.en.md)
- [AA-Actor](aa-actor.en.md)
- [Newcomer FAQ](../faq/newcomer-faq.en.md)
