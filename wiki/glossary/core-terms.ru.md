---
page_type: glossary
title: Базовые термины
summary: Компактный глоссарий по главным терминам DEOS и TMCTOL. Открывайте эту страницу первой, если сокращения, словарь runtime, governance-термины или различие между фреймворком и стандартом пока кажутся неочевидными.
locale: ru
canonical_page_id: core-terms
translation_of: core-terms.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../README.md
  - ../../docs/README.md
  - ../../docs/tmctol.specification.en.md
  - ../../docs/core.architecture.en.md
  - ../../docs/aaa.specification.en.md
  - ../../docs/governance.specification.en.md
  - ../../docs/governance.architecture.en.md
  - ../../docs/read-model.contract.en.md
status: active
audience: newcomer
tags:
  - glossary
  - terminology
related:
  - Обзор фреймворка DEOS
  - Стандарт TMCTOL
  - Токен-управляемая автоматизация
  - Обзор Governance
  - Домены Governance
  - Разделение read-model
  - FAQ для новичков
last_compiled: 2026-04-20
confidence: 0.95
---

# Базовые термины

## Кратко

Этот глоссарий держит самые важные термины проекта в одном месте. Он особенно полезен потому, что репозиторий строго различает идентичность фреймворка, идентичность стандарта, словарь runtime-исполнения, governance-словарь и терминологию read-model.

## Термины

### DEOS

`Deterministic Economic Operating System`. Имя фреймворка для этого репозитория и эталонного стека.

### TMCTOL

`Token Minting Curve + Treasury-Owned Liquidity`. Текущий флагманский экономический стандарт, работающий поверх DEOS.

### TMC

`Token Minting Curve`. Mint-only механизм линейной эмиссии, который задает текущий потолок цены для нового предложения.

### TOL

`Treasury-Owned Liquidity`. Политический слой, который направляет результат минтинга в ликвидность под контролем протокола и сегментирует ее по bucket-направлениям.

### AAA

`Account Abstraction Actors`. В DEOS так называется вся runtime-система: паллет, планировщик, правила жизненного цикла и среда исполнения.

### AA-Актор

Один конкретный ограниченный runtime-экземпляр внутри более широкой системы AAA.

### Axial Router

Паллет маршрутизации, который сравнивает ограниченные пути исполнения, выбирает лучший маршрут и направляет торговую комиссию в протокольный поток.

### Balance Ingress

Токен-управляемый триггер, при котором поступление активов в систему может запустить следующий детерминированный переход состояния.

### Governance Domain

Одна типизированная governance-cell внутри более широкой governance-системы. Она связывает управляемый subject, power surfaces, допустимые payload-family, cadence и execution authority.

### Primary Track

Линия, которая отвечает на судьбу самого proposal-а. В зависимости от домена и payload-family она может быть бинарной (`Aye / Nay`) или invoice-формы (`Amplify / Approve / Reduce / Nay`).

### Protection Track

Конституционный трек `Veto / Pass`, который может блокировать или процедурно ускорять protected governance-flow. Это отдельная линия, а не скрытый псевдоним обычного `Nay`.

### Proposal Payload Kind

Типизированное описание того, какое governance-действие proposal вообще пытается авторизовать: стратегическое Root-действие, тактическую treasury-трату, тактическое parameter-изменение, same-domain intent или тактический сигнал в стратегический слой.

### Execution Authority

Уровень authority, до которого успешный governance-payload реально может дойти при enactment. В DEOS тактические домены не наследуют стратегическую или Root-эквивалентную власть автоматически.

### GovXP

Ограниченный сигнал governance-участия, который governance-подсистема экспортирует для будущей reward- и reputation-логики. На текущей линии он остается counters-first, а не live vote-power multiplier.

### Canonical Projection

Ограниченное on-chain представление, предназначенное для прямого потребления клиентом как части живого протокольного контракта.

### Materialized View

Индексированное или внешне реконструированное представление для архива, поиска, дашбордов и аналитики, а не для ограниченной консенсусной правды.

### Native / `$NTVE`

Суверенный базовый токен текущей эталонной линии.

### `$VETO`

Токен защиты, используемый для стратегической конституционной поверхности в текущей линии.

### `$BLDR`

Флагманский токен тактического governance и координации билдеров в текущей линии.

### `stXXX`

Передаваемые staking-receipts, которые представляют долевое владение в пулах стейкинга.

## Связанные страницы

- [Обзор фреймворка DEOS](../overview/deos-framework.ru.md)
- [Стандарт TMCTOL](../concepts/tmctol-standard.ru.md)
- [Токен-управляемая автоматизация](../concepts/token-driven-automation.ru.md)
- [Обзор Governance](../overview/governance-overview.ru.md)
- [Домены Governance](../concepts/governance-domains.ru.md)
- [Разделение read-model](../concepts/read-model-split.ru.md)
- [FAQ для новичков](../faq/newcomer-faq.ru.md)

## Источники

- `README.md`
- `docs/README.md`
- `docs/tmctol.specification.en.md`
- `docs/core.architecture.en.md`
- `docs/aaa.specification.en.md`
- `docs/governance.specification.en.md`
- `docs/governance.architecture.en.md`
- `docs/read-model.contract.en.md`
