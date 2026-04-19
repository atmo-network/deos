---
page_type: overview
title: Reference Client
summary: The DEOS web client is an on-chain-first reference UI for live protocol flows. Its architecture separates economic widgets from layout infrastructure, centralizes execution feedback, and keeps data provenance visible instead of hiding where information comes from.
locale: en
canonical_page_id: reference-client
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/web-client.architecture.en.md
  - ../../web-client/README.md
  - ../../docs/read-model.contract.en.md
status: active
audience: newcomer
tags:
  - overview
  - web-client
  - product
related:
  - First Steps
  - Read-Model Split
  - Newcomer FAQ
  - Core Terms
last_compiled: 2026-04-16
confidence: 0.9
---

# Reference Client

## Summary

The repository-local web client is the browser-facing DEOS reference client. It is described as `on-chain-first`, which means its main live product flows should rely on bounded canonical runtime surfaces rather than quietly depending on off-chain reconstruction.

The client is still early, but its architecture already has a clear opinion about truth surfaces, widget boundaries, and layout vocabulary.

## What `On-Chain-First` Means Here

The client reads live protocol state through chain-backed adapters. Current product surfaces include balances, route previews, governance views, automation status, and session-bounded charting.

The docs are explicit that large archives and long-range analytics still belong to indexed or materialized providers. The UI should not present those surfaces as if they were identical to direct chain truth.

## Widgets vs Layout

A major client rule is to keep economic functions separate from layout infrastructure.

- `Widgets` are product surfaces such as swap, wallet, governance, charts, and automation
- `Layout` is the pane, tile, split, tab, and reserved-lane machinery that arranges those widgets

This vocabulary matters because the architecture does not want pane mechanics to masquerade as economic features.

## Reserved Edge Lanes

The header, footer, and sidebar are treated as reserved edge lanes around the central workspace rather than as ordinary tab panes.

That keeps shell controls, account selection, settings, and compact status surfaces outside the same interaction model used for the main economic widgets.

## Centralized Execution Feedback

The client also tries to avoid repeating transaction-status UI everywhere. `LogWidget` is the main execution-feedback surface, while action widgets stay focused on initiating actions.

That follows the same anti-duplication rule the docs apply to shared UI components and provenance badges.

## Read-Model Honesty in the UI

The client follows the repository-wide read-model split and adds a second browser-side realization axis. That makes it possible to say not only whether a surface is canonical-chain or materialized, but also whether the browser currently realizes it directly, from session cache, from session-derived state, or from a provider.

The point is honesty: a session-built view should not pretend to be the same thing as retained archive truth.

## Related

- [First Steps](../getting-started/first-steps.en.md)
- [Read-Model Split](../concepts/read-model-split.en.md)
- [Newcomer FAQ](../faq/newcomer-faq.en.md)
- [Core Terms](../glossary/core-terms.en.md)

## Sources

- `docs/web-client.architecture.en.md`
- `web-client/README.md`
- `docs/read-model.contract.en.md`
