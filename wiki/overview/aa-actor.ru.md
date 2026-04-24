---
page_type: overview
title: AA-Актор
summary: AAA в DEOS — это система Account Abstraction Actors, а AA-Актор — один конкретный ограниченный экземпляр внутри нее. Акторы позволяют выражать повторяющиеся протокольные процессы как типизированные и планируемые execution plans, а не разносить одну и ту же логику по множеству специальных паллетов.
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
last_compiled: 2026-04-16
confidence: 0.93
---

# AA-Актор

## Кратко

`AAA` означает `Account Abstraction Actors`, то есть систему и паллет. `AA-Актор` — это один ограниченный экземпляр исполнения внутри этой системы.

Если вам нужен взгляд на весь системный уровень, начните со страницы [Система AAA](aaa-system.ru.md). Эта страница объясняет, что такое один актор и зачем DEOS использует именно такую абстракцию.

## Что такое актор

Актор — это настроенный runtime-экземпляр со своим sovereign account, расписанием, execution plan, правилами жизненного цикла и поведением при сбоях.

Удобная ментальная модель такая: `один sovereign account + одна trigger-поверхность + один ограниченный план`.

Вместо того чтобы размазывать повторяющуюся экономическую логику по множеству специальных паллетов, DEOS может выразить ограниченный workflow как актора с типизированными шагами и явными правилами исполнения.

## Для чего используются акторы

В текущей эталонной линии акторы используются для таких процессов, как:

- Обеспечение ликвидности
- Потоки сжигания и buyback
- Казначейская split-маршрутизация
- Удержание или unwinding bucket-позиций
- Пользовательские ограниченные цепочки задач

Большая часть протокольного исполнения в текущей линии реализована как System-акторы.

## Стабильный контракт

Спецификация делает центральными несколько гарантий:

- Детерминированное поведение для одного и того же состояния и контекста блока
- Ограниченный объем работы в рамках явных лимитов
- Stateless execution plans вместо изменяемой памяти между шагами
- Предсказуемые исходы при сбоях
- Уничтожение на месте без автоматического refund fan-out

Эти гарантии важны, потому что акторы должны быть безопасной runtime-инфраструктурой, а не просто удобным скриптингом.

## Типы акторов

Спецификация различает два широких класса:

- `User AAA`, который подчиняется пользовательской модели комиссий и правилам owner-slot
- `System AAA`, который создается через governance, остается изменяемым и используется для автоматизации протокола

Это разделение сохраняет движок переиспользуемым, но при этом позволяет runtime поставлять собственные семейства детерминированных акторов.

## Триггеры и расписание

Акторы могут запускаться по расписанию, вручную или по событиям поступления баланса на адрес. Это часть более широкой token-driven модели: переходы состояния должны реагировать на ограниченные условия исполнения, а не опираться только на специальные административные вызовы.

## Упрощенные примеры конфигурации

Примеры ниже написаны в виде концептуального pseudo-config. Это не точный синтаксис extrinsic-вызовов, а наглядный способ посмотреть на одного актора изнутри.

При этом примеры специально изолируют только форму trigger-а и execution plan. Реальный актор все равно живет внутри полного AAA-контракта: детерминированного scheduler, cooldown-ограничений, fee admission, lifecycle-правил и bounded execution.

### 1. Таймерный актор сжигания

```yaml
actor: Burn collected fees
kind: System
trigger:
  type: Timer
  every_blocks: 10
execution_plan:
  - task: SwapExactIn
    asset_in: ForeignFeeAsset
    asset_out: Native
    amount: AllBalance
  - task: Burn
    asset: Native
    amount: AllBalance
```

Такой актор просыпается каждые `10` блоков, конвертирует накопленный fee-актив в нативный актив и затем сжигает результат.

### 2. Актор, который реагирует на пополнение баланса

```yaml
actor: React to foreign collateral arrival
kind: System
trigger:
  type: OnAddressEvent
  asset_filter: [ForeignAsset]
execution_plan:
  - task: SwapExactIn
    asset_in: ForeignAsset
    asset_out: Native
    amount: PercentageOfCurrent(50%)
  - task: AddLiquidity
    asset_a: Native
    asset_b: ForeignAsset
```

Здесь актор вообще ничего не делает, пока на его аккаунт не придет `ForeignAsset`. Поступивший актив — это не только ценность на балансе, но и сигнал пробуждения, который говорит актору: пора реагировать.

### 3. Один актор как узел графа

```yaml
actor: Treasury lane B
kind: System
trigger:
  type: OnAddressEvent
  asset_filter: [LPToken]
execution_plan:
  - task: RemoveLiquidity
    asset: LPToken
    amount: AllBalance
  - task: SplitTransfer
    outputs:
      - Native -> TreasuryB
      - ForeignAsset -> TreasuryB
```

Этот актор может долго ничего не делать, пока другой актор не пришлет ему `LPToken`. В терминах графа это означает, что выход одного актора становится trigger-message для другого. Именно так из небольших ограниченных акторов складывается более крупное поведение протокола.

## Почему акторы важны для DEOS

Через акторов DEOS превращает экономическую координацию в явное runtime-поведение. Они связывают эмиссию, маршрутизацию, bucket-механику, казначейские действия и downstream-операции под управлением governance, не заставляя каждый повторяющийся процесс становиться отдельным pallet-костылем.

## Связанные страницы

- [Система AAA](aaa-system.ru.md)
- [Токен-управляемая автоматизация](../concepts/token-driven-automation.ru.md)
- [Контур маршрутизации и минтинга](../concepts/routing-and-minting-loop.ru.md)
- [Обзор Governance](governance-overview.ru.md)
- [Базовые термины](../glossary/core-terms.ru.md)

## Источники

- `docs/aaa.specification.en.md`
- `docs/aaa.architecture.en.md`
- `docs/core.architecture.en.md`
