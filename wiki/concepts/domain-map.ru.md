---
page_type: concept
title: Карта доменов
summary: Самодостаточная карта основных доменов знаний DEOS и связей между ними внутри wiki.
locale: ru
canonical_page_id: domain-map
translation_of: domain-map.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../docs/README.md
  - ../../docs/core.architecture.en.md
  - ../../docs/tmctol.specification.en.md
  - ../../docs/web-client.architecture.en.md
status: active
audience: newcomer
tags:
  - domain-map
  - onboarding
  - wiki
related:
  - Обзор фреймворка DEOS
  - Стандарт TMCTOL
  - Система AAA
  - Обзор Governance
  - Экономика $BLDR
  - Эталонный клиент
last_compiled: 2026-07-20
confidence: 0.9
---

# Карта доменов

## Кратко

DEOS проще понимать как набор связанных доменов, а не как список pallet-ов, файлов или UI-виджетов. Pallet-ы и модули — это форма реализации. Wiki-домены — смысловые контуры: каждый объясняет отдельную силу в системе и ведет к другим силам, от которых зависит.

Используйте эту страницу как карту графа знаний внутри wiki.

## Главные домены

### Идентичность фреймворка

Этот домен объясняет, что такое DEOS: форкаемая deterministic economic operating system для протокольных экономик. Он отделяет DEOS как фреймворк от TMCTOL как текущего токеномического стандарта.

Начните с [Обзора фреймворка DEOS](../overview/deos-framework.ru.md), затем используйте [Базовые термины](../glossary/core-terms.ru.md), когда словарь становится плотным.

### Экономическая физика

Этот домен объясняет управляемую токен-экономику: кривые minting, treasury-owned liquidity, поведение пола/потолка, живучесть burn-механики и claims о compression.

Читайте [Стандарт TMCTOL](tmctol-standard.ru.md), [Токеновые поверхности](token-surfaces.ru.md), [Сценарии TOL buckets](tol-bucket-scenarios.ru.md), [Экономические пороги](economic-thresholds.ru.md), [Формулы TMCTOL](../math/tmctol-formulas.ru.md), [Token Minting Curve](../overview/token-minting-curve.ru.md) и [Контур маршрутизации и минтинга](routing-and-minting-loop.ru.md).

### Автономные акторы

Домен акторов объясняет, как protocol-owned accounts выполняют ограниченные задачи. System AAA actors сжигают, маршрутизируют, добавляют ликвидность, разделяют потоки, держат buckets и исполняют treasury policies без зависимости от специальных manager pallets.

Читайте [Систему AAA](../overview/aaa-system.ru.md), [AA-Актор](../overview/aa-actor.ru.md) и [Токен-управляемую автоматизацию](token-driven-automation.ru.md).

### Маршрутизация и идентичность активов

Домен маршрутизации связывает намерение пользователя, пути minting, AMM-ликвидность, комиссии и зарегистрированные активы. Здесь фреймворк решает, использовать рыночную или протокольную ликвидность, и как foreign assets становятся локальными runtime-сущностями.

Читайте [Axial Router](../overview/axial-router.ru.md), [Идентичность активов](../overview/asset-identity.ru.md) и [Разделение read-model](read-model-split.ru.md).

### Governance и защита

Домен governance объясняет, кто и что может менять. DEOS governance привязан к доменам: у каждой управляемой области есть primary power surface, protection surface, типизированные payload, ограниченные полномочия исполнения и явные ограничения.

Читайте [Обзор Governance](../overview/governance-overview.ru.md), [Домены Governance](governance-domains.ru.md) и [Physics-first против politics-first](../comparisons/physics-vs-politics.ru.md).

### Экономика созидателей и полезная работа

Домен созидателей находится на пересечении тактического управления, финансирования труда, ликвидности под контролем протокола и спроса на продукты производной экосистемы. Он объясняет, как публичные счета-заявки могут вознаграждать завершенную работу, не превращая статус основателя в привилегию фреймворка, а защита Native удерживает тактический домен внутри делегированных границ.

Сначала прочитайте [Экономику $BLDR](builder-economy.ru.md), затем обратитесь к [Токеновым поверхностям](token-surfaces.ru.md) и [Доменам Governance](governance-domains.ru.md) за подробностями о токене и полномочиях.

### Стейкинг и награды

Домен staking объясняет share-vault receipts, native liquid staking, LP nomination, reward memory и protocol donation into liquidity. Он связывает экономическую безопасность, пользовательские позиции и governance-conditioned rewards.

Читайте [Пулы стейкинга](staking-pools.ru.md) и [Трехуровневую валидацию](../development/three-layer-validation.ru.md).

### Клиент и read model

Клиентский домен объясняет, как браузерный продукт показывает систему, не притворяясь источником истины. Он отделяет прямое on-chain состояние, session-derived projections и будущих materialized/indexed providers.

Читайте [Эталонный клиент](../overview/reference-client.ru.md), [UI Kit и Domain DAG](ui-kit-and-domain-dag.ru.md) и [Разделение read-model](read-model-split.ru.md).

### Инструменты и валидация

Домен инструментов объясняет, как contributors и agents удерживают систему честной: simulator math, проверки runtime, проверка доверенной wiki, проверка Domain DAG и release gates.

Читайте [Трехуровневую валидацию](../development/three-layer-validation.ru.md), [Слой скриптов](../usage/scripts-layer.ru.md) и [Статус разработки](../development/status.ru.md).

### Future gates

Домен future gates объясняет, что намеренно не входит в текущую поставленную базовую линию: permissionless collators, relay-beacon randomness, full indexed portfolio discovery и более богатые materialized-архивы.

Читайте [Стратегию случайности](../overview/randomness-strategy.ru.md) и [Статус разработки](../development/status.ru.md).

## Как домены связаны

Полезный маршрут:

1. Идентичность фреймворка задает продуктовую границу.
2. Экономическая физика задает token laws.
3. Автономные акторы исполняют повторяющиеся economic flows.
4. Маршрутизация и идентичность активов связывают users, assets и protocol liquidity.
5. Governance и защита определяют, какие изменения допустимы.
6. Экономика созидателей превращает делегированный капитал в оцененную полезную работу.
7. Стейкинг и награды связывают пользователей с security и стимулами.
8. Клиент и read model показывают систему без выдумывания истины.
9. Инструменты и валидация удерживают граф синхронизированным.
10. Future gates не дают спекулятивной работе выглядеть поставленной реальностью.

## Связанные страницы

- [Архитектурные схемы](architecture-diagrams.ru.md)
- [Сквозные сценарии](end-to-end-flows.ru.md)
- [Обзор фреймворка DEOS](../overview/deos-framework.ru.md)
- [Стандарт TMCTOL](tmctol-standard.ru.md)
- [Система AAA](../overview/aaa-system.ru.md)
- [Обзор Governance](../overview/governance-overview.ru.md)
- [Экономика $BLDR](builder-economy.ru.md)
- [Эталонный клиент](../overview/reference-client.ru.md)
