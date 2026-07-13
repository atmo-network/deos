---
page_type: implementation
title: Технологический стек
summary: Ключевые технологии фреймворка DEOS — Polkadot SDK, Rust, SvelteKit, JavaScript/BigInt и локальный слой автоматизации.
locale: ru
canonical_page_id: tech-stack
translation_of: tech-stack.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../docs/core.architecture.en.md
status: active
audience: developer
tags:
  - implementation
  - tech-stack
  - architecture
related:
  - Структура репозитория
  - Паттерны runtime
last_compiled: 2026-04-16
confidence: 0.95
---

# Технологический стек

## Кратко

DEOS построен на современном сочетании блокчейн- и веб-технологий. Runtime-слой опирается на Polkadot SDK и Rust, экономическая симуляция — на JavaScript и BigInt, а браузерный клиент — на SvelteKit и TypeScript.

## Блокчейн-слой

### Polkadot SDK

DEOS реализован как runtime парачейна на базе Polkadot SDK и следует актуальной линии `2606`.

Основные опоры здесь такие:

- **Rust** как основной язык реализации
- **`frame::v2`** для строго типизированных паллетов
- **`frame_benchmarking::v2`** для измерения весов
- **Wasm** как целевая среда исполнения runtime

### Omni Node

DEOS использует архитектуру Omni Node. Это означает, что проект не держит отдельный кастомный `node/` с большим количеством шаблонного кода, а опирается на современный стандарт развертывания из экосистемы Polkadot.

### XCM

Внешние активы приходят через XCM и затем привязываются к локальным `AssetId` через реестр активов.

## Слой симуляции

Экономический полигон в `/simulator` написан на JavaScript с активным использованием `BigInt`. Это нужно для того, чтобы сначала подтвердить математическую модель, а уже потом переносить ее в Rust-код runtime.

## Эталонный клиент

Веб-клиент DEOS построен на:

- **SvelteKit**
- **TypeScript**
- Реактивных store-механизмах для bounded on-chain данных и клиентских производных представлений

## Автоматизация и инструменты

- **Bash-скрипты** для локального запуска, проверок и операционной рутины
- **Markdown + skills** в `/.agents/` для координации агентной работы

## Как пользоваться этой страницей

Используйте эту страницу как карту стека реализации после того, как уже понятно, какой домен меняется. Она показывает, в какую технологическую границу вы входите, прежде чем выбирать глубину проверки или редактировать конкретную поверхность репозитория.

## Связанные страницы

- [Структура репозитория](repository-structure.ru.md)
- [Паттерны runtime](../overview/runtime-patterns.ru.md)
- [Трехуровневая валидация](../development/three-layer-validation.ru.md)
- [Эталонный клиент](../overview/reference-client.ru.md)
- [Слой скриптов](../usage/scripts-layer.ru.md)
- [Координация агентов](../usage/agent-coordination.ru.md)
