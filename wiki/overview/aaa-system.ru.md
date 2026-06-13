---
page_type: overview
title: Система AAA
summary: AAA в DEOS — система Account Abstraction Actors с pallet, scheduler, правилами жизненного цикла, моделью комиссий и детерминированной средой исполнения для отдельных акторов, при сохранении доменной логики в adapters и pallet-ах.
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
  - ../../docs/aaa.embedding.en.md
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
last_compiled: 2026-06-13
confidence: 0.95
---

# Система AAA

## Кратко

`AAA` означает `Account Abstraction Actors`. В DEOS это имя всей runtime-системы: `pallet-aaa`, scheduler, правила жизненного цикла, модель комиссий, аккаунты акторов и типизированная среда исполнения для ограниченных протокольных потоков.

[AA-Актор](aa-actor.ru.md) — один конкретный экземпляр внутри этой системы. Эта страница объясняет системный контракт.

## Системный контракт

AAA дает runtime один переиспользуемый способ исполнять ограниченные планы действий, не превращая каждый повторяющийся рабочий процесс в отдельный pallet.

На уровне системы AAA дает:

- Детерминированное планирование;
- Запуск от событий и балансов;
- Типизированные задачи: transfer, swap, liquidity, burn, mint, stake и unstake;
- Правила жизненного цикла для pause, failure, auto-close и manual close;
- Разделение между пользовательскими акторами и System actors под governance;
- Adapter boundaries, чтобы AAA координировал runtime-механику, но не владел DEX, staking или asset logic.

Баланс актора может работать как сообщение-триггер: приход актива на аккаунт актора может разбудить следующий ограниченный execution plan.

## Граница встраивания

Внешний runtime может переиспользовать `pallet-aaa`, не наследуя каталог System actors из DEOS/TMCTOL. Host runtime предоставляет ограниченные adapters для assets, DEX, staking, liquidity donation, fee conversion, ingress, entropy и task weights. AAA владеет scheduling, lifecycle, amount resolution, fee reservation и task orchestration.

Гарантия atomicity действует на уровне task, а не всего execution plan. Если adapter падает после частичной мутации, failed task откатывает свои локальные эффекты и success event; более ранние успешные steps остаются committed. `ContinueNextStep` или `AbortCycle` затем решает, продолжится cycle или остановится.

## Граница переносимости

Текущий staking-контракт намеренно общий:

```text
Task::Stake { asset, amount }
Task::Unstake { asset, shares }
```

AAA не кодирует DEOS-specific `StakeNative`, выбор коллатора, имя `stNTVE` или custody для `NTVE/stNTVE` LP. Runtime adapters решают, что общий stake означает для конкретной chain. В DEOS adapter направляет native staking в `pallet-staking::stake_native`, а nomination security остается отдельной locked-LP staking/governance поверхностью.

Так AAA остается полезным за пределами одной токеномической конфигурации.

## Роль в DEOS

В текущей эталонной линии AAA — исполнительный слой для runtime-side поведения протокола: burning, liquidity provisioning, treasury splitting, bucket handling, BLDR lane flows и native staking LP provisioning.

Поставляемый runtime создает System actors при genesis, плюс один детерминированный fee-sink address. Native staking LP provisioning стартует неактивным и может включиться только после готовности native staking receipt, staking pool, actor и непустого `NTVE/stNTVE` AMM.

AAA не заменяет TMC, Axial Router, staking или governance. Эти подсистемы владеют математикой и доменными правилами. AAA дает им детерминированный способ работать вместе.

## Зачем это нужно

Без AAA повторяющиеся экономические рабочие процессы снова и снова превращались бы в bespoke pallet logic. AAA делает такие рабочие процессы явными, ограниченными, управляемыми и компонуемыми как типизированные графы акторов.

Выход баланса одного актора может стать trigger message для другого. Более крупное поведение протокола может складываться из малых ограниченных частей, оставаясь внутри deterministic scheduling и execution limits.

В рамках существующего языка задач и adapters многие изменения workflow/topology могут перейти из runtime rewrites в on-chain actor-graph configuration. Runtime upgrades остаются нужны для новых primitives, adapter surfaces или safety invariants.

## Связанные страницы

- [AA-Актор](aa-actor.ru.md)
- [Токен-управляемая автоматизация](../concepts/token-driven-automation.ru.md)
- [Контур маршрутизации и минтинга](../concepts/routing-and-minting-loop.ru.md)
- [Обзор Governance](governance-overview.ru.md)
- [Базовые термины](../glossary/core-terms.ru.md)
