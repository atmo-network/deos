---
page_type: concept
title: Карта инвариантов и угроз
summary: Компактная карта ключевых инвариантов DEOS/TMCTOL, угроз, owner surfaces, проверок, governance-mutability и failure modes.
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
  - ../../docs/governance.specification.en.md
  - ../../docs/web-client.architecture.en.md
status: active
audience: developer
tags:
  - concept
  - invariants
  - validation
  - governance
  - threat-model
  - security
related:
  - Economic Claim Levels
  - Трехуровневая валидация
  - Стандарт TMCTOL
  - Чем DEOS не является
  - Домены Governance
last_compiled: 2026-06-13
confidence: 0.86
---

# Карта инвариантов и угроз

## Кратко

Эта страница связывает ключевые инварианты и угрозы DEOS/TMCTOL с owner surface, путем проверки, допустимостью governance-изменений и главным failure mode. Это не обзорная статья, а плотная карта контроля.

| Инвариант                   | Владелец    | Проверка              | Governance           | Failure                |
|---|---|---|---|---|
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

## Центральные угрозы

| Угроза | Форма | Защита | Owner |
|---|---|---|---|
| Governance выводит TOL | увод резервов | domain payloads + protection | governance/spec |
| Обход роутера | обход fee/burn | шлюз роутера | router |
| Неправильный bucket | потеря происхождения | сегментированные каналы | TMCTOL + AAA |
| Путаница с indexer | архив как правда | значки происхождения | client/docs |
| Доверенная фаза коллатора | доверие как permissionless | launch-line ограничение | runtime/ops |
| Зависание графа акторов | сбой/простой/оракул | retry + cooldown | AAA |
| Параметрический гриффинг | параметры ломают допущения | bounded settings | runtime/gov |
| Атака на оценку LP | LP завышен/учтен дважды | консервативное хранение | staking/gov |
| Ложное происхождение в UI | UI скрывает класс данных | read-model contract | web client |

## Как использовать

Если изменение затрагивает одну из строк, сначала проверяйте owner surface. Затем поднимайтесь выше, если изменение пересекает math, runtime, client или governance boundaries.

Угроза не «решена» только потому, что страница её упомянула. Она контролируется только тогда, когда у поверхности owner есть bounded mechanism и путь проверки.

## Связанные страницы

- [Economic Claim Levels](economic-claim-levels.ru.md)
- [Трехуровневая валидация](../development/three-layer-validation.ru.md)
- [Стандарт TMCTOL](tmctol-standard.ru.md)
- [Чем DEOS не является](what-deos-is-not.ru.md)
- [Домены Governance](governance-domains.ru.md)
