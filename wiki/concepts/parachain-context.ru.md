---
page_type: concept
title: Parachain context
summary: Как DEOS связан с Polkadot, parachains, XCM, collators, Omni Node и upstream relay-chain dependencies.
locale: ru
canonical_page_id: parachain-context
translation_of: parachain-context.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../docs/core.architecture.en.md
  - ../../docs/asset-registry.architecture.en.md
  - ../../docs/randomness.strategy.en.md
  - ../../docs/polkadot-sdk-2603.insights.en.md
  - ../../template/README.md
status: active
audience: newcomer
tags:
  - concept
  - polkadot
  - parachain
  - xcm
  - collators
related:
  - Обзор фреймворка DEOS
  - Runtime patterns
  - Идентичность активов
  - Технологический стек
  - Стратегия randomness
last_compiled: 2026-06-13
confidence: 0.86
---

# Parachain context

## Кратко

DEOS — parachain-oriented framework. Он не пытается быть отдельным blockchain stack с custom node внутри этого репозитория. Reference runtime рассчитан на Omni Node deployment, соглашения Polkadot SDK, local assets, XCM asset identity и controlled collator phase.

Короткая модель: DEOS владеет economic logic, а Polkadot дает более широкий parachain execution environment.

## Главные слои

```text
Relay / ecosystem layer
  provides shared security, parachain context, upstream SDK constraints

Omni Node
  runs the parachain without an in-repo custom node crate

DEOS runtime
  owns pallets, assets, routing, staking, governance, and AAA actors

Reference client / indexers
  read bounded chain state directly and materialized history externally
```

## XCM и Asset Identity

DEOS рассматривает foreign assets как local registered assets после governance-controlled registration. Stable identity — не “как сейчас сериализуется XCM location”. Registry хранит bidirectional `Location <-> AssetId` mappings, чтобы будущие location updates не ломали balances или local economic logic.

Так cross-chain asset identity остается совместимой с local bitmask-based asset model, который используют DEOS runtime primitives.

## Collators и randomness

Текущая launch line использует trusted-collator simplification phase. Native binding targets остаются ограничены trusted collators, пока не появится production-ready relay/protocol beacon для parachain-consumable per-block randomness.

Это значит, что local pseudo-random fallbacks могут поддерживать bounded reference behavior, но не заменяют будущий protocol-grade randomness source. Существующая epoch-scale relay randomness не удовлетворяет этому контракту; предпочтительная замена — будущий per-block protocol beacon, который parachains смогут потреблять через weight-accounted ingress.

## Costs и operations

Downstream ecosystem все равно несет operator concerns:

- Инфраструктура collators и monitoring;
- Настройка endpoints и bootnodes;
- Процедура runtime upgrade;
- XCM asset registration и поддержка locations;
- Archive/indexer infrastructure для unbounded history;
- Client endpoint defaults и provider reliability.

Это deployment responsibilities, а не hidden product assumptions внутри runtime.

## Связанные страницы

- [Обзор фреймворка DEOS](../overview/deos-framework.ru.md)
- [Runtime patterns](../overview/runtime-patterns.ru.md)
- [Идентичность активов](../overview/asset-identity.ru.md)
- [Технологический стек](../implementation/tech-stack.ru.md)
- [Стратегия randomness](../overview/randomness-strategy.ru.md)
