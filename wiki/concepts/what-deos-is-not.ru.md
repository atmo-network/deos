---
page_type: concept
title: Чем DEOS не является
summary: Определение DEOS через отрицательные границы, которое убирает типичные ошибки про governance, liquidity, гарантии, smart contracts, indexers и randomness.
locale: ru
canonical_page_id: what-deos-is-not
translation_of: what-deos-is-not.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../docs/core.architecture.en.md
  - ../../docs/tmctol.specification.en.md
status: active
audience: newcomer
tags:
  - concept
  - positioning
  - boundaries
related:
  - Обзор фреймворка DEOS
  - Карта инвариантов и угроз
  - Economic Claim Levels
  - Parachain context
last_compiled: 2026-05-17
confidence: 0.88
---

# Чем DEOS не является

## Кратко

DEOS проще понимать, когда явно видны его отрицательные границы. Это deterministic economic framework, а не обещание, что каждая соседняя проблема уже решена внутри runtime.

## Не эти вещи

- **Не generic DAO.** Governance в DEOS привязана к доменам и ограничена типизированными payloads, protection tracks и полномочиями исполнения.
- **Не redemption-backed stable asset.** У TMCTOL есть поддержка floor и механика ликвидности, но нет обещания неограниченного погашения.
- **Не unbounded smart-contract platform.** Runtime поставляет ограниченные экономические сервисы, а не произвольные вычисления, развёртываемые пользователями.
- **Не oracle-free guarantee machine.** Некоторые утверждения зависят от резервов, состояния рынка, состояния маршрутизации или будущих provider/read-model surfaces.
- **Не indexerless analytics platform.** Ограниченные live projections могут быть on-chain; неограниченная история и поиск принадлежат materialized providers.
- **Не randomness/fairness product at launch.** Launch line использует trusted-collator упрощение, пока не появится подходящий relay/protocol randomness path.
- **Не finished ecosystem product.** Репозиторий — forkable framework; downstream forks приносят продуктовый нарратив, пользователей, dApps и политику экосистемы.

## Почему это важно

Отрицательные границы защищают от overclaiming. Они также показывают fork-командам, какие обязанности переходят к ним, а не “молча закрываются” фреймворком.

## Связанные страницы

- [Обзор фреймворка DEOS](../overview/deos-framework.ru.md)
- [Карта инвариантов и угроз](invariant-map.ru.md)
- [Economic Claim Levels](economic-claim-levels.ru.md)
- [Parachain context](parachain-context.ru.md)
