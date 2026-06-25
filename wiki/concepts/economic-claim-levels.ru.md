---
page_type: concept
title: Уровни экономических утверждений
summary: Лестница и audit posture для классификации экономических утверждений DEOS/TMCTOL между формулами, симуляциями, runtime enforcement, governance dependency и рыночными assumptions.
locale: ru
canonical_page_id: economic-claim-levels
translation_of: economic-claim-levels.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../docs/tmctol.specification.en.md
  - ../../simulator/README.md
  - ../../.agents/skills/alignment/economic-claims.json
status: active
audience: newcomer
tags:
  - concept
  - economics
  - validation
  - claims
related:
  - Экономические пороги
  - Карта инвариантов
  - Карта инвариантов и угроз
  - Трехуровневая валидация
last_compiled: 2026-06-25
confidence: 0.91
---

# Уровни экономических утверждений

## Кратко

Утверждения DEOS/TMCTOL не должны смешивать математику, симуляции, реализацию, governance и поведение рынка в одной фразе. Эта лестница показывает, на каком уровне подтверждается claim, а alignment-аудит держит небольшой реестр ключевых экономических утверждений с явными видами доказательств.

## Уровни

| Уровень | Значение             | Пример                            |
| ------- | -------------------- | --------------------------------- |
| Level 0 | Formula-defined      | TMC price следует curve equation  |
| Level 1 | Simulator-supported  | Threshold держится на vectors     |
| Level 2 | Runtime-enforced     | Tests фиксируют transition        |
| Level 3 | Governance-dependent | True внутри bounded policy        |
| Level 4 | Market assumption    | Зависит от users/liquidity/demand |

## Правило чтения

Более сильная формулировка не делает claim сильнее. Если утверждение зависит от поведения рынка, относите его к Level 4, даже если underlying formula относится к Level 0.

Пример: floor mechanics могут быть formula-defined и simulator-supported, но user demand, arbitrage timing или поведение liquidity providers все равно остаются Level 4 assumptions.

## Аудит-реестр

Репозиторный alignment-слой также ведет выбранные экономические утверждения в `economic-claims.json`. Каждая запись должна указывать на evidence: формулы, тесты, runtime symbols, архитектурные claims или guard scripts. Реестр не является маркетинговым чеклистом. Его задача — привязывать сильную прозу к проверяемой опоре и делать отсутствующие proof surfaces видимыми.

## Связанные страницы

- [Экономические пороги](economic-thresholds.ru.md)
- [Карта инвариантов](invariant-map.ru.md)
- [Карта инвариантов и угроз](invariant-map.ru.md)
- [Трехуровневая валидация](../development/three-layer-validation.ru.md)
