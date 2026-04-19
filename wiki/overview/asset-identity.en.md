---
page_type: overview
title: Asset Identity
summary: The asset registry is DEOS's foreign-asset identity layer. It turns XCM locations into stable runtime asset identities, preserves that mapping across XCM-version changes, and keeps foreign assets inside a dedicated namespace so the rest of the runtime can treat them as first-class economic inputs.
locale: en
canonical_page_id: asset-identity
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/asset-registry.architecture.en.md
  - ../../docs/core.architecture.en.md
status: active
audience: newcomer
tags:
  - overview
  - assets
  - xcm
related:
  - Routing and Minting Loop
  - Staking Pools
  - Core Terms
  - Newcomer FAQ
last_compiled: 2026-04-15
confidence: 0.91
---

# Asset Identity

## Summary

The asset registry is the runtime's foreign-asset gateway. Its job is not to decide treasury or liquidity policy, but to give foreign assets stable identities inside the local runtime.

That identity layer is important because the router, staking, liquidity flows, and other economic subsystems need a consistent way to refer to outside assets.

## What the Registry Does

The registry maps XCM `Location` values to runtime `AssetId` values and persists that mapping in storage.

The docs describe this as a hybrid pattern:

- Use deterministic hashing at registration time
- Persist the resulting mapping in storage
- Allow later location-key migration without breaking balances

## Why Pure Hashing Is Not Enough

A pure hash-only model would break when XCM location encodings change across versions. The same conceptual foreign asset could end up with a new hash just because its serialized location changed.

The registry avoids that by persisting the mapping after registration.

## Namespace Discipline

Foreign assets live in a dedicated `0xF...` namespace under the broader DEOS bitmask asset taxonomy. That separation helps prevent collisions with local protocol assets, LP assets, and staking receipts.

## Why This Matters for the Rest of the Runtime

Once registered, foreign assets can participate in the rest of the economic stack as first-class assets. That includes routing, pool creation, burning flows, and other runtime wiring.

The asset registry is therefore an identity layer that enables economic composition without itself becoming a policy engine.

## Related

- [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
- [Staking Pools](../concepts/staking-pools.en.md)
- [Core Terms](../glossary/core-terms.en.md)
- [Newcomer FAQ](../faq/newcomer-faq.en.md)

## Sources

- `docs/asset-registry.architecture.en.md`
- `docs/core.architecture.en.md`
