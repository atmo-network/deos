---
page_type: overview
title: Обзор Governance
summary: Широкая карта DEOS Governance как ограниченного конституционного слоя, который разделяет протокольную физику, стратегическую защиту, тактическую координацию и живую истину read-model.
locale: ru
canonical_page_id: governance-overview
translation_of: governance-overview.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../docs/governance.specification.en.md
  - ../../docs/governance.architecture.en.md
  - ../../docs/manifesto.en.md
status: active
audience: newcomer
tags:
  - overview
  - governance
  - domains
related:
  - Домены Governance
  - Physics-first против politics-first
  - Пулы стейкинга
  - Базовые термины
last_compiled: 2026-07-20
confidence: 0.9
---

# Обзор Governance

## Кратко

DEOS Governance — это ограниченный конституционный слой фреймворка. Он не подменяет протокольную физику политикой по умолчанию, а управляет стратегическими, тактическими и социальными поверхностями, которые остаются после механизации экономического ядра.

Всю подсистему проще читать через три разделения:

- Стратегия не равна тактике;
- Одобрение не равно защите;
- Живая on-chain governance-истина не равна архивной истории.

## Текущая форма

Текущая эталонная линия использует явные governance-домены вместо одного безликого пула референдумов. У каждого домена видны полномочия, семейства payload, cadence и защитная поверхность.

На уровне обзора форма такая:

- Стратегический governance защищает протокольные и сетевые предметы управления;
- Тактический governance занимается более узкими доменно-локальными тратами и координацией;
- Primary lane решает судьбу proposal;
- Protection lane решает, должен ли proposal быть заблокирован или пропущен дальше;
- Payload типизированы и не прячутся в непрозрачные bytes;
- Живой governance UX читает ограниченные runtime views, а архивный поиск и длинные timeline остаются задачей indexed или materialized слоев.

Поэтому DEOS Governance больше похож на конституционный слой над детерминированным ядром, чем на обычный voting portal.

## Публичный жизненный цикл

Для новичка proposal лучше читать как типизированный жизненный цикл, а не как один yes/no event:

1. submission открывает item и его protection window;
2. ordinary primary voting стартует после настроенного lead-in;
3. approval может еще ждать enactment delay;
4. execution может завершиться ошибкой, и эта ошибка видна как состояние;
5. недавние finalized outcomes остаются on-chain только для ограниченной живой наблюдаемости.

Цель — честное живое состояние, а не бесконечный социальный архив внутри runtime storage.

## Связь со стейкингом

Governance может передавать в стейкинг ограниченные сигналы качества участия, но governance и staking остаются разными подсистемами. Связь означает, что качество governance экономически важно; она не означает, что staking pallet владеет governance history или что governance хранит бесконечную reward memory.

## Как читать governance-кластер

Лучший порядок чтения:

1. `Обзор Governance` — зачем нужна вся подсистема;
2. [Домены Governance](../concepts/governance-domains.ru.md) — как типизируется одна governance-ячейка;
3. [Physics-first против politics-first](../comparisons/physics-vs-politics.ru.md) — почему протокольная физика остается защищенной;
4. [Пулы стейкинга](../concepts/staking-pools.ru.md) — где governance-conditioned reward signals встречаются со стейкингом;
5. [Базовые термины](../glossary/core-terms.ru.md) — повторяющийся словарь.

## Связанные страницы

- [Домены Governance](../concepts/governance-domains.ru.md)
- [Physics-first против politics-first](../comparisons/physics-vs-politics.ru.md)
- [Пулы стейкинга](../concepts/staking-pools.ru.md)
- [Базовые термины](../glossary/core-terms.ru.md)
