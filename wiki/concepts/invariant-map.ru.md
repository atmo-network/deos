---
page_type: concept
title: Карта инвариантов
summary: Компактная карта ключевых инвариантов DEOS/TMCTOL, их владельцев, проверок, governance-mutability и failure modes.
locale: ru
canonical_page_id: invariant-map
translation_of: invariant-map.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../docs/tmctol.specification.en.md
  - ../../docs/core.architecture.en.md
  - ../../docs/axial-router.architecture.en.md
  - ../../docs/aaa.specification.en.md
status: active
audience: developer
tags:
  - concept
  - invariants
  - validation
  - governance
related:
  - Threat Model
  - Economic Claim Levels
  - Трехуровневая валидация
  - Стандарт TMCTOL
last_compiled: 2026-05-17
confidence: 0.84
---

# Карта инвариантов

## Кратко

Эта страница связывает ключевые инварианты DEOS/TMCTOL с владельцем, поверхностью проверки, допустимостью governance-изменений и главным failure mode. Это не обзорная статья, а плотная карта контроля.

| Инвариант                   | Владелец    | Проверка              | Governance           | Failure                |
| --------------------------- | ----------- | --------------------- | -------------------- | ---------------------- |
| TMC integral pricing        | TMC         | simulator + tests     | нет post-launch      | wrong mint price       |
| Unidirectional minting      | TMC         | pallet + runtime      | нет                  | reserve extraction     |
| Router fee burn/split       | Router      | runtime + bench       | ограничено           | fee bypass             |
| AAA bounded work            | AAA         | bench + tests         | no bypass            | overweight/stuck graph |
| TOL bucket split            | TOL + AAA   | sim + runtime         | возможно bounded     | bucket misuse          |
| Asset identity bijection    | Registry    | runtime tests         | register/update only | identity drift         |
| Staking share accounting    | Staking     | pallet + runtime      | no override          | receipt dilution       |
| Governance domain authority | Governance  | tests + review        | explicit policy      | authority creep        |
| Read-model honesty          | Client/docs | DAG + wiki + validate | нет                  | false chain truth      |
| Wiki trust boundary         | Wiki/client | trust validator       | нет                  | unsafe rendering       |

## Как использовать

Если изменение затрагивает одну из строк, сначала проверяйте owner surface. Затем поднимайтесь выше, если изменение пересекает math, runtime, client или governance boundaries.

## Связанные страницы

- [Threat Model](threat-model.ru.md)
- [Economic Claim Levels](economic-claim-levels.ru.md)
- [Трехуровневая валидация](../development/three-layer-validation.ru.md)
- [Стандарт TMCTOL](tmctol-standard.ru.md)
