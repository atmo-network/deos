---
page_type: concept
title: Архитектурные схемы
summary: Компактные текстовые схемы главных связей DEOS, включая карту доменов, routing loop, граф AAA actors, read-model split и границу governance/staking.
locale: ru
canonical_page_id: architecture-diagrams
translation_of: architecture-diagrams.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../docs/core.architecture.en.md
  - ../../docs/aaa.architecture.en.md
  - ../../docs/axial-router.architecture.en.md
  - ../../docs/governance.architecture.en.md
  - ../../docs/staking.architecture.en.md
status: active
audience: newcomer
tags:
  - concept
  - diagrams
  - architecture
  - onboarding
related:
  - Карта доменов
  - Сквозные сценарии
  - Контур маршрутизации и минтинга
  - Система AAA
  - Разделение read-model
last_compiled: 2026-05-17
confidence: 0.88
---

# Архитектурные схемы

## Кратко

Эта страница дает компактные визуальные карты для читателей, которым сначала нужна общая форма, а уже потом детали. Схемы намеренно текстовые: так они одинаково читаются в репозитории, web client и agent contexts.

Используйте [Карту доменов](domain-map.ru.md) для понимания владельцев доменов и [Сквозные сценарии](end-to-end-flows.ru.md) для пошаговых walkthroughs.

## Главный доменный контур

```text
User intent
  -> Reference Client
  -> Read-model classification
  -> Runtime surface
  -> Axial Router / TMC / Staking / Governance
  -> Events, balances, bounded projections
  -> Reference Client feedback
```

Клиент не является источником истины. Он читает bounded chain truth напрямую, когда это возможно, и явно помечает session/materialized data, если это не прямое protocol state.

## Routing и minting

```text
Swap request
  -> Axial Router
      -> Compare XYK market path
      -> Compare TMC protocol path
      -> Choose best bounded route
  -> Execute swap or mint
  -> Collect fee
  -> Burn / treasury / actor flow
```

Router координирует market liquidity и protocol liquidity. TMC владеет deterministic mint-side pricing. Long-range analytics остаются вне consensus state.

## Граф AAA actors

```text
Asset arrives on actor account
  -> Balance ingress trigger
  -> AAA scheduler checks lifecycle / cooldown / limits
  -> Actor executes typed plan
  -> Output asset lands elsewhere
  -> Next actor may wake
```

AAA — переиспользуемая система исполнения. AA-Актор — один ограниченный экземпляр внутри нее. Более крупное поведение протокола собирается из малых actor steps.

## Граница governance и staking

```text
Governance domain
  -> Primary track + protection track
  -> Typed payload
  -> Bounded execution authority
  -> Optional participation-quality signal
  -> Staking reward coefficient

Staking pool
  -> Share-vault accounting
  -> Receipt supply
  -> Reward settlement
```

Governance и staking взаимодействуют, но не схлопываются в одну подсистему. Governance может давать bounded reward signals; staking владеет pool math и settlement.

## Read-model split

```text
Public datum
  -> Bounded canonical on-chain projection
  -> Or indexed / materialized view

Browser realization
  -> Direct read
  -> Session cache
  -> Session-derived view
  -> Provider-backed materialized data
```

Первое разделение — protocol contract. Второе объясняет, как браузер сейчас получает конкретное значение.

## Связанные страницы

- [Карта доменов](domain-map.ru.md)
- [Сквозные сценарии](end-to-end-flows.ru.md)
- [Контур маршрутизации и минтинга](routing-and-minting-loop.ru.md)
- [Система AAA](../overview/aaa-system.ru.md)
- [Разделение read-model](read-model-split.ru.md)
