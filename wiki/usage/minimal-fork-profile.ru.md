---
page_type: usage
title: Минимальный профиль форка
summary: Минимальный набор решений, которые downstream-команда должна принять перед превращением DEOS в конкретную экосистему.
locale: ru
canonical_page_id: minimal-fork-profile
translation_of: minimal-fork-profile.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ./forking-deos.ru.md
  - ../../docs/tmctol.specification.en.md
status: active
audience: developer
tags:
  - usage
  - forkability
  - launch
related:
  - Форк DEOS
  - Чем DEOS не является
  - Карта инвариантов
  - Parachain context
last_compiled: 2026-05-17
confidence: 0.84
---

# Минимальный профиль форка

## Кратко

DEOS fork не должен запускаться простой заменой названий. Downstream-команда должна выбрать экономический, governance, runtime, клиентский и операторский профиль, который превращает фреймворк в реальную экосистему.

## Обязательные решения

| Область                      | Минимальное решение форка                                                      |
| ---------------------------- | ------------------------------------------------------------------------------ |
| Native asset                 | Название, ticker, decimals, allocation, роль в staking/governance              |
| Foreign collateral set       | Какие assets можно register, route или использовать как collateral             |
| TMC curve params             | Initial price, slope, supply assumptions, launch immutability policy           |
| TOL distribution             | Bucket split, paired treasuries, reserve/lane semantics                        |
| Bucket policies              | Какой bucket будит какого actor, threshold, retry и treasury lane              |
| Router fee                   | Границы fee, burn/sink routing, governance mutability                          |
| Governance domain pairs      | Primary/protection tokens, payload kinds, cadence, execution authority         |
| Staking receipt policy       | Receipt namespaces, native receipt, LP custody, reward paths                   |
| Materialized provider policy | Какие user flows требуют indexers или archive/search providers                 |
| Collator/randomness posture  | Trusted phase, upgrade path, relay/protocol randomness dependency              |
| Client/product surface       | Default endpoints, wallet presets, copy, dApps, risk wording                   |
| Validation baseline          | Simulator, runtime tests, client validation, wiki trust, operator smoke checks |

## Правило готовности

Если строка не решена, fork все еще prototype. Если строка решена, но не проверена, fork не готов к launch.

## Связанные страницы

- [Форк DEOS](forking-deos.ru.md)
- [Чем DEOS не является](../concepts/what-deos-is-not.ru.md)
- [Карта инвариантов](../concepts/invariant-map.ru.md)
- [Parachain context](../concepts/parachain-context.ru.md)
