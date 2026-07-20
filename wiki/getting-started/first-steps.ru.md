---
page_type: getting-started
title: Первые шаги
summary: Короткий самодостаточный маршрут для понимания DEOS, выбора домена и запуска правильной проверки.
locale: ru
canonical_page_id: first-steps
translation_of: first-steps.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../README.md
  - ../../docs/README.md
  - ../../docs/read-model.contract.en.md
status: active
audience: newcomer
tags:
  - getting-started
  - onboarding
  - workflow
related:
  - Начните здесь
  - DEOS за 60 секунд
  - Executive Summary
  - Карта доменов
  - Обзор фреймворка DEOS
  - Стандарт TMCTOL
  - Разделение read-model
  - Эталонный клиент
  - FAQ для новичков
last_compiled: 2026-07-20
confidence: 0.85
---

# Первые шаги

## Кратко

Если вы впервые смотрите на DEOS, начните с крючка, а уже потом с карты доменов. DEOS — это форкаемый runtime-фреймворк, где выпуск токена, protocol-owned liquidity, маршрутизация, staking, governance и автоматизированные actors становятся детерминированной институциональной машиной.

Проект — не просто runtime, не просто веб-клиент и не просто токеномическая формула. Это фреймворк, где экономическая физика, автономные акторы, governance, стейкинг, клиентские модели чтения и инструменты проверки усиливают друг друга.

Хороший первый проход: используйте [Начните здесь](start-here.ru.md), чтобы выбрать путь понимания за 10 минут, локальный запуск или безопасный fork/change-economy путь. Затем используйте эту страницу, когда нужна более широкая карта доменов.

## Первый маршрут

0. [Начните здесь](start-here.ru.md)
1. [DEOS за 60 секунд](deos-in-60-seconds.ru.md)
2. [Executive Summary](executive-summary.ru.md)
3. [Питч для партнера](partner-pitch.ru.md)
4. [Обзор фреймворка DEOS](../overview/deos-framework.ru.md)
5. [Карта доменов](../concepts/domain-map.ru.md)
6. [Базовые термины](../glossary/core-terms.ru.md)
7. [Стандарт TMCTOL](../concepts/tmctol-standard.ru.md)
8. [Токен-управляемая автоматизация](../concepts/token-driven-automation.ru.md)
9. [Разделение read-model](../concepts/read-model-split.ru.md)

Этот путь дает словарь проекта до того, как появляются имена pallet-ов, runtime-файлы или продуктовые UI-термины.

Если вы оцениваете DEOS как партнер, используйте маршрут оценки внутри [Partner Pitch](partner-pitch.ru.md), а не читайте весь граф подряд.

## Выберите правильный домен

### Экономическая физика

Используйте [Стандарт TMCTOL](../concepts/tmctol-standard.ru.md), [Формулы TMCTOL](../math/tmctol-formulas.ru.md) и [Token Minting Curve](../overview/token-minting-curve.ru.md), когда речь идет о формулах, полу, потолке, minting, burning или claims о compression.

### Runtime и акторы

Используйте [Систему AAA](../overview/aaa-system.ru.md), [AA-Актор](../overview/aa-actor.ru.md), [Паттерны runtime](../overview/runtime-patterns.ru.md) и [Идентичность активов](../overview/asset-identity.ru.md), когда затронуты поведение реализации, потоки планировщика, активы или интеграции.

### Governance и защита

Используйте [Обзор Governance](../overview/governance-overview.ru.md), [Домены Governance](../concepts/governance-domains.ru.md) и [Physics-first против politics-first](../comparisons/physics-vs-politics.ru.md), когда затронуты полномочия, payload, protection tracks или пути обновлений.

### Клиент и wiki

Используйте [Эталонный клиент](../overview/reference-client.ru.md), [UI Kit и Domain DAG](../concepts/ui-kit-and-domain-dag.ru.md) и [Generated Wiki](../concepts/generated-wiki.ru.md), когда работа касается браузерного UX, честности read-model, layout, UI-примитивов или рендеринга wiki.

### Локальные операции

Используйте [Слой скриптов](../usage/scripts-layer.ru.md), [Трехуровневую валидацию](../development/three-layer-validation.ru.md) и [Статус разработки](../development/status.ru.md), когда нужны настройка, проверки, release gates или текущие открытые границы.

## Подход к валидации

Начинайте с наименьшего осмысленного слоя проверки:

- Изменения математики -> проверки simulator;
- Изменения runtime -> targeted cargo validation;
- Изменения клиента -> проверка веб-клиента;
- Изменения wiki -> проверка trusted wiki и формы ссылок;
- Междоменные изменения -> локальная проверка завершенности репозитория.

## Связанные страницы

- [Начните здесь](start-here.ru.md)
- [DEOS за 60 секунд](deos-in-60-seconds.ru.md)
- [Executive Summary](executive-summary.ru.md)
- [Карта доменов](../concepts/domain-map.ru.md)
- [Обзор фреймворка DEOS](../overview/deos-framework.ru.md)
- [Стандарт TMCTOL](../concepts/tmctol-standard.ru.md)
- [Разделение read-model](../concepts/read-model-split.ru.md)
- [Эталонный клиент](../overview/reference-client.ru.md)
- [FAQ для новичков](../faq/newcomer-faq.ru.md)
