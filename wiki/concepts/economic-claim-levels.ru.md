---
page_type: concept
title: Уровни экономических утверждений
summary: Лестница для классификации DEOS/TMCTOL economic claims между formulas, simulations, runtime enforcement, governance dependency и market assumptions.
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
  - Threat Model
  - Трехуровневая валидация
last_compiled: 2026-05-17
confidence: 0.86
---

# Уровни экономических утверждений

## Кратко

Утверждения DEOS/TMCTOL не должны смешивать математику, simulation, implementation, governance и поведение рынка в одной фразе. Эта лестница показывает, на каком уровне подтверждается claim.

## Уровни

| Уровень | Значение             | Пример                            |
| ------- | -------------------- | --------------------------------- |
| Level 0 | Formula-defined      | TMC price следует curve equation  |
| Level 1 | Simulator-supported  | Threshold держится на vectors     |
| Level 2 | Runtime-enforced     | Tests фиксируют transition        |
| Level 3 | Governance-dependent | True внутри bounded policy        |
| Level 4 | Market assumption    | Зависит от users/liquidity/demand |

## Правило чтения

Более сильная формулировка не делает claim сильнее. Если statement зависит от поведения рынка, называйте его Level 4, даже если underlying formula относится к Level 0.

Пример: floor mechanics могут быть formula-defined и simulator-supported, но user demand, arbitrage timing или поведение liquidity providers все равно остаются Level 4 assumptions.

## Связанные страницы

- [Экономические пороги](economic-thresholds.ru.md)
- [Карта инвариантов](invariant-map.ru.md)
- [Threat Model](threat-model.ru.md)
- [Трехуровневая валидация](../development/three-layer-validation.ru.md)
