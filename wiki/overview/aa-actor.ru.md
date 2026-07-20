---
page_type: overview
title: AA-Актор
summary: AAA в DEOS — это система Account Abstraction Actors, а AA-Актор — один конкретный ограниченный экземпляр внутри нее. Акторы выражают повторяющиеся протокольные процессы как типизированные и планируемые execution plans вместо bespoke pallet logic.
locale: ru
canonical_page_id: aa-actor
translation_of: aa-actor.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../docs/aaa.specification.en.md
  - ../../docs/aaa.architecture.en.md
  - ../../docs/core.architecture.en.md
status: active
audience: newcomer
tags:
  - overview
  - aaa
  - automation
related:
  - Система AAA
  - Токен-управляемая автоматизация
  - Контур маршрутизации и минтинга
  - Обзор Governance
  - Базовые термины
last_compiled: 2026-07-20
confidence: 0.9
---

# AA-Актор

## Кратко

`AAA` — система Account Abstraction Actors. `AA-Актор` — один ограниченный экземпляр исполнения внутри этой системы.

Для системного уровня используйте [Систему AAA](aaa-system.ru.md). Эта страница объясняет абстракцию одного актора.

## Контракт актора

Удобная ментальная модель:

```text
один sovereign account + одна trigger surface + один ограниченный plan
```

У актора есть свой account, schedule или trigger, execution plan, правила жизненного цикла и поведение при сбоях. Вместо того чтобы разносить повторяющуюся экономическую логику по специальным pallet-ам, DEOS может выразить ограниченный рабочий процесс как типизированные шаги актора под явными runtime limits.

Стабильный контракт подчеркивает:

- Детерминированное поведение для одного состояния и block context;
- Ограниченный объем работы;
- Stateless execution plans вместо mutable workflow memory;
- Предсказуемые исходы при сбоях;
- Уничтожение на месте без автоматического refund fan-out.

Акторы — runtime-инфраструктура, а не свободный scripting.

## Классы и применение

Спецификация различает два широких класса:

- `User AAA`: пользовательская модель комиссий и owner-slot rules;
- `System AAA`: governance-created actors для автоматизации протокола.

В текущей эталонной линии акторы поддерживают liquidity provisioning, burning/buyback flows, treasury split routing, bucket hold/unwind behavior и пользовательские ограниченные task pipelines. Большая часть protocol-owned исполнения реализована как System actors.

## Triggers и формы plan

Акторы могут запускаться по schedule, manual trigger или balance-ingress address event. Balance ingress — ключевая token-driven форма: актив, пришедший на аккаунт актора, может быть одновременно wake-up message.

Типовые формы plan:

- Timer-driven burning: swap собранных fees в Native, затем burn;
- Balance-triggered liquidity: реакция на foreign collateral arrival, swap части актива, затем add liquidity;
- Graph node: получить LP token от другого актора, unwind его и split outputs в treasury accounts.

Во всех случаях актор остается внутри полного AAA-контракта: deterministic scheduling, cooldowns, fee admission, lifecycle rules и bounded execution.

## Зачем акторы нужны

Акторы превращают экономическую координацию в явное runtime-поведение. Они связывают minting, routing, buckets, treasury actions и governance-owned operations, не заставляя каждый повторяющийся flow становиться custom pallet code.

Они также делают возможными actor graphs: выход баланса одного актора может стать trigger message для другого. Более крупное поведение протокола собирается из малых ограниченных частей и остается видимым как typed automation.

## Связанные страницы

- [Система AAA](aaa-system.ru.md)
- [Токен-управляемая автоматизация](../concepts/token-driven-automation.ru.md)
- [Контур маршрутизации и минтинга](../concepts/routing-and-minting-loop.ru.md)
- [Обзор Governance](governance-overview.ru.md)
- [Базовые термины](../glossary/core-terms.ru.md)
