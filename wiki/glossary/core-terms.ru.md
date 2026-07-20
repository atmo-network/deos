---
page_type: glossary
title: Базовые термины
summary: Компактный глоссарий по главным терминам DEOS и TMCTOL. Открывайте эту страницу первой, если сокращения, runtime-словарь, governance-термины, frontend-архитектура, роли wiki или статусные поверхности пока кажутся неочевидными.
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
  - ../../BACKLOG.md
  - ../../CHANGELOG.md
  - ../../docs/tmctol.specification.en.md
  - ../../docs/core.architecture.en.md
  - ../../docs/aaa.specification.en.md
  - ../../docs/governance.specification.en.md
  - ../../docs/governance.architecture.en.md
  - ../../docs/read-model.contract.en.md
  - ../../docs/web-client.architecture.en.md
  - ../../web-client/README.md
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
  - Экономика $BLDR
  - Разделение read-model
  - UI Kit и Domain DAG
  - Generated Wiki
  - Маршруты чтения
  - Статус разработки
  - FAQ для новичков
last_compiled: 2026-07-20
confidence: 0.9
---

# Базовые термины

## Кратко

Этот глоссарий — справочная поверхность, а не второй слой объяснений. Каждый термин остается коротким и ведет к связанным страницам за полным контекстом.

## Термины

### DEOS

`Deterministic Economic Operating System`. Имя фреймворка для этого репозитория и эталонного стека.

### TMCTOL

`Token Minting Curve + Treasury-Owned Liquidity`. Текущий флагманский экономический стандарт, работающий поверх DEOS.

### TMC

`Token Minting Curve`. Mint-only механизм линейной эмиссии, который задает текущий потолок цены для нового предложения.

### TOL

`Treasury-Owned Liquidity`. Ликвидность под контролем протокола, разделенная на домены buckets.

### AAA

`Account Abstraction Actors`. В DEOS так называется вся runtime-система: паллет, планировщик, правила жизненного цикла и среда исполнения.

### AA-Актор

Один конкретный ограниченный runtime-экземпляр внутри более широкой системы AAA.

### Axial Router

Ограниченный механизм выбора маршрута для протокольных сделок и потока комиссий.

### Balance Ingress

Токен-управляемый триггер, при котором поступление активов в систему может запустить следующий детерминированный переход состояния.

### Governance Domain

Одна типизированная governance-ячейка, связывающая subject, power surface, семейство payload, cadence и полномочия исполнения.

### Primary Track

Линия решения по proposal: binary или invoice-shaped в зависимости от домена и payload.

### Protection Track

Конституционный трек `Veto / Pass`, отдельный от обычного approval по proposal.

### Proposal Payload Kind

Типизированная категория действия, которое governance proposal пытается авторизовать.

### Execution Authority

Уровень полномочий, до которого успешный governance payload может дойти при enactment.

### GovXP

Ограниченный сигнал участия в governance для логики наград или репутации.

### Canonical Projection

Ограниченное on-chain представление, предназначенное для прямого потребления клиентом как части живого протокольного контракта.

### Materialized View

Индексированное или внешне реконструированное представление для архива, поиска, дашбордов и аналитики, а не для ограниченной консенсусной правды.

### Native / `$NTVE`

Суверенный базовый токен текущей эталонной линии.

### `$VETO`

Токен защиты, используемый для стратегической конституционной поверхности в текущей линии.

### `$BLDR`

Флагманский токен тактического управления и координации созидателей в текущей линии. См. [Экономику $BLDR](../concepts/builder-economy.ru.md).

### `stXXX`

Передаваемые staking-receipts, которые представляют долевое владение в пулах стейкинга.

### UI Kit

Локальный набор визуальных примитивов интерфейса.

### Domain DAG

Дисциплина владения в веб-клиенте и validator для направления импортов, headers и drift границ.

### Widget

Браузерная продуктовая поверхность: swap, wallet, staking, governance, chart, automation, log, account, settings, status или чтение wiki.

### Layout

Подсистема веб-клиента, которая размещает panes, tabs, reserved lanes, header, footer и sidebar, не становясь продуктовым доменом.

### Generated Wiki

Самодостаточный слой объяснений `/wiki` для введения, навигации, чтения метаданных и frontend-рендеринга.

### Reading Path

Маршрут wiki для конкретной роли или задачи: экономика, runtime, governance, клиент, статус или инструменты.

### Architecture Document

Не привязанное к релизу зеркало реализации для поставленной подсистемы.

### `BACKLOG.md`

Канонический файл открытой работы для закрываемых поставок, явных gates и ограниченных epics.

### `CHANGELOG.md`

Канонический файл завершенных поставок и истории релизов.

### Development Status

Wiki-карта текущего состояния для базовых доменов, активного фокуса, открытых границ и работы за gates.

## Связанные страницы

- [Обзор фреймворка DEOS](../overview/deos-framework.ru.md)
- [Стандарт TMCTOL](../concepts/tmctol-standard.ru.md)
- [Токен-управляемая автоматизация](../concepts/token-driven-automation.ru.md)
- [Обзор Governance](../overview/governance-overview.ru.md)
- [Домены Governance](../concepts/governance-domains.ru.md)
- [Экономика $BLDR](../concepts/builder-economy.ru.md)
- [Разделение read-model](../concepts/read-model-split.ru.md)
- [UI Kit и Domain DAG](../concepts/ui-kit-and-domain-dag.ru.md)
- [Generated Wiki](../concepts/generated-wiki.ru.md)
- [Маршруты чтения](../getting-started/reading-paths.ru.md)
- [Статус разработки](../development/status.ru.md)
- [FAQ для новичков](../faq/newcomer-faq.ru.md)
