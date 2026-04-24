---
page_type: overview
title: Система AAA
summary: AAA в DEOS — это система Account Abstraction Actors с pallet, scheduler, lifecycle-правилами, fee-моделью и детерминированной средой исполнения для отдельных акторов. Текущий контракт сохраняет AAA переносимым, потому что staking automation использует generic `Stake { asset, amount }` / `Unstake { asset, shares }`, а DEOS-native liquid staking, LP nomination и custody policy живут в runtime adapters и pallet-ах staking/governance.
locale: ru
canonical_page_id: aaa-system
translation_of: aaa-system.en.md
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
  - runtime
  - automation
related:
  - AA-Актор
  - Токен-управляемая автоматизация
  - Контур маршрутизации и минтинга
  - Обзор Governance
  - Базовые термины
last_compiled: 2026-04-25
confidence: 0.95
---

# Система AAA

## Кратко

`AAA` означает `Account Abstraction Actors`. В DEOS это имя всей runtime-системы: `pallet-aaa`, её планировщика, правил жизненного цикла и комиссий, а также типизированной среды исполнения для ограниченных протокольных потоков.

`AA-Актор` — это один конкретный экземпляр внутри этой системы. Эта страница объясняет системный уровень; отдельная страница [AA-Актор](aa-actor.ru.md) описывает уровень одного экземпляра.

## Что дает система

AAA дает runtime один переиспользуемый способ исполнять ограниченные планы действий, не превращая каждый повторяющийся экономический процесс в отдельный pallet.

Полная картина здесь состоит сразу из нескольких частей: детерминированное планирование, ограниченное исполнение, явные lifecycle- и fee-правила и реакция на ограниченные события. В модели DEOS баланс актора может работать как сообщение-триггер: поступление определенного актива на определенный акторный аккаунт может само по себе определять, какой план должен «проснуться» и какое экономическое действие должно произойти дальше.

На практическом уровне система дает:

- Детерминированное планирование исполнения
- Реакцию на события, прежде всего через пополнение баланса
- Типизированные задачи: переводы, swaps, действия с ликвидностью, burn, mint и staking
- Явные правила для паузы, сбоев, автозакрытия и ручного закрытия
- Разделение между пользовательскими акторами и governance-owned System actors
- Adapter boundaries, чтобы AAA координировал runtime-механику, не встраивая внутрь себя DEX, staking или asset-accounting logic

## Граница переносимости

Текущий staking-контракт AAA намеренно generic:

```text
Task::Stake { asset, amount }
Task::Unstake { asset, shares }
```

AAA не кодирует DEOS-specific `StakeNative`, выбор коллатора, имя `stNTVE` или custody для `NTVE/stNTVE` LP. Runtime adapters решают, что generic stake означает для конкретной цепи. В DEOS runtime adapter направляет native staking в `pallet-staking::stake_native`, а nomination security остается отдельной staking/governance поверхностью locked LP.

Так AAA остается полезным за пределами одной tokenomic-конфигурации.

## Система и актор — не одно и то же

Это различие принципиально:

- `AAA` = система, pallet, scheduler и все акторы вместе
- `AA-Актор` = один ограниченный экземпляр внутри этой системы

Именно поэтому в DEOS можно говорить об AAA как об инфраструктуре и одновременно — о множестве акторов с разными ролями.

## Роль AAA в DEOS

В текущей эталонной линии AAA является исполнительным слоем для runtime-логики протокола. Через него DEOS выражает сжигание, обеспечение ликвидности, маршрутизацию казначейских потоков, поведение bucket-аккаунтов и другие ограниченные экономические реакции.

Часть этих реакций запускается по таймеру. Другая часть — по балансу: актив, пришедший на аккаунт актора, может сам выступить сообщением-триггером для следующего ограниченного шага. Это одна из самых важных идей всей системы.

При этом AAA не заменяет TMC, Axial Router, staking или governance. Доменные правила и математика живут в этих подсистемах, а AAA дает им детерминированный способ работать вместе.

## Текущая системная топология

В поставляемом runtime DEOS создаются System actors в genesis, плюс один зарезервированный детерминированный адрес fee sink. Текущий набор System actors включает burning, Zap/liquidity, TOL bucket, treasury, BLDR lane и Native Staking LP Farmer roles.

Native Staking LP Farmer стартует как guarded skeleton. Его можно активировать только после готовности native staking receipt, staking pool, actor и непустого `NTVE/stNTVE` AMM.

## Зачем системе нужен именно такой слой

Без AAA runtime пришлось бы снова и снова добавлять специальную pallet-логику под каждый повторяющийся экономический workflow. AAA делает такие процессы явными, ограниченными и управляемыми через execution plans, scheduler-семантику, lifecycle-правила и trigger-семантику.

Один из самых интересных эффектов проявляется при композиции графов акторов. Выход одного актора может стать сообщением-триггером для другого, а цепочки таких реакций складываются в более крупное поведение протокола из небольших ограниченных частей. Но и эта композиция по-прежнему живет внутри того же детерминированного scheduler-контракта и bounded execution.

В рамках уже существующего языка задач и адаптеров это переносит большой класс изменений протокола из области runtime rewrite в область перенастройки on-chain графа акторов. Runtime upgrade по-прежнему нужен для новых примитивов, adapter-поверхностей или safety-инвариантов, но многие изменения workflow и topology могут оставаться на уровне конфигурации.

Это сохраняет ядро компактнее, а поведение протокола — более прозрачным: оно остается видимым как типизированная автоматизация, а не растворяется в скрытом glue code.

## Связанные страницы

- [AA-Актор](aa-actor.ru.md)
- [Токен-управляемая автоматизация](../concepts/token-driven-automation.ru.md)
- [Контур маршрутизации и минтинга](../concepts/routing-and-minting-loop.ru.md)
- [Обзор Governance](governance-overview.ru.md)
- [Базовые термины](../glossary/core-terms.ru.md)

## Источники

- `docs/aaa.specification.en.md`
- `docs/aaa.architecture.en.md`
- `docs/core.architecture.en.md`
