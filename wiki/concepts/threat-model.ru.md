---
page_type: concept
title: Threat Model
summary: Центральная карта основных рисков DEOS, форм атак, защитных мер и owner surfaces.
locale: ru
canonical_page_id: threat-model
translation_of: threat-model.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../docs/core.architecture.en.md
  - ../../docs/tmctol.specification.en.md
  - ../../docs/governance.specification.en.md
  - ../../docs/web-client.architecture.en.md
status: active
audience: developer
tags:
  - concept
  - threat-model
  - security
  - governance
related:
  - Карта инвариантов
  - Чем DEOS не является
  - Economic Claim Levels
  - Домены Governance
last_compiled: 2026-05-17
confidence: 0.82
---

# Threat Model

## Кратко

DEOS не является системой без рисков. Он устроен так, чтобы важные риски были названы, ограничены и привязаны к правильной owner surface, а не спрятаны в продуктовых формулировках.

## Центральные угрозы

| Угроза | Форма | Защита | Owner |
| --- | --- | --- | --- |
| Governance выводит TOL | увод резервов | domain payloads + protection | governance/spec |
| Обход роутера | обход fee/burn | шлюз роутера | router |
| Неправильный bucket | потеря происхождения | сегментированные каналы | TMCTOL + AAA |
| Путаница с indexer | архив как правда | значки происхождения | client/docs |
| Доверенная фаза коллатора | доверие как permissionless | launch-line ограничение | runtime/ops |
| Зависание графа акторов | сбой/простой/оракул | retry + cooldown | AAA |
| Параметрический гриффинг | параметры ломают допущения | bounded settings | runtime/gov |
| Атака на оценку LP | LP завышен/учтен дважды | консервативное хранение | staking/gov |
| Ложное происхождение в UI | UI скрывает класс данных | read-model contract | web client |

## Правило чтения

Угроза не «решена» только потому, что страница её упомянула. Она контролируется только тогда, когда у поверхности owner есть bounded mechanism и путь проверки.

## Связанные страницы

- [Карта инвариантов](invariant-map.ru.md)
- [Чем DEOS не является](what-deos-is-not.ru.md)
- [Economic Claim Levels](economic-claim-levels.ru.md)
- [Домены Governance](governance-domains.ru.md)
