---
type: implementation
title: Технологический стек
description: "Основные технологии фреймворка DEOS: Polkadot SDK, Rust, SvelteKit, JavaScript с BigInt и локальные средства автоматизации."
locale: ru
canonical_page_id: tech-stack
translation_of: tech-stack.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - resource: ../../docs/core.architecture.en.md
  - resource: ../../template/Cargo.toml
  - resource: ../../template/runtime/src/lib.rs
  - resource: ../../web-client/package.json
  - resource: ../../web-client/src/lib/read-model.ts
  - resource: ../../simulator/README.md
  - resource: ../../scripts/README.md
status: stable
audience: developer
tags:
  - implementation
  - tech-stack
  - architecture
related:
  - Структура репозитория
  - Принципы среды исполнения
---

# Технологический стек

## Кратко

DEOS построен на современных технологиях для блокчейна и веб-приложений. Ядро использует Polkadot SDK, ранее известный как Substrate, а интерфейс — SvelteKit для производительных реактивных приложений.

## Уровень блокчейна

### Polkadot SDK

DEOS реализован как среда исполнения парачейна на базе Polkadot SDK. Проект следует современной линии `Polkadot SDK 2606`, а не устаревшим подходам Substrate.

- **Язык:** Rust.
- **Система макросов:** `frame::v2` для строго типизированного определения паллетов.
- **Измерение производительности:** `frame_benchmarking::v2`.
- **Исполнение:** среда WebAssembly (Wasm).

### Архитектура Omni Node

DEOS использует архитектуру развёртывания Omni Node. Вместо собственного узла с большим объёмом шаблонного кода среда исполнения запускается стандартным двоичным файлом Omni Node из экосистемы Polkadot.

### XCM

Внешние активы интегрируются через XCM v5. Внутренний реестр активов сопоставляет их местоположения с устойчивыми локальными значениями `AssetId`.

## Уровень моделирования

Экономический полигон `/simulator` написан на обычном JavaScript и широко использует `BigInt`. Это позволяет подтвердить корректность математики до её реализации на Rust.

## Эталонный клиент

Эталонный клиент DEOS — лёгкий реактивный интерфейс, который прежде всего использует данные блокчейна и явно различает производные данные сеанса и материализованные данные.

- **Фреймворк:** SvelteKit.
- **Язык:** TypeScript.
- **Управление состоянием:** реактивные хранилища для ограниченных данных из блокчейна.

## Автоматизация и инструменты

- **Скрипты:** обычный Bash (`.sh`) для рабочих процедур.

## Как пользоваться этой страницей

Обращайтесь к этой карте после того, как определили изменяемую предметную область. Она показывает технологическую границу предстоящей работы и помогает выбрать необходимую глубину проверки и правильную часть репозитория.

## Связанные страницы

- [Структура репозитория](repository-structure.ru.md)
- [Принципы среды исполнения](../overview/runtime-patterns.ru.md)
- [Трёхуровневая проверка](../development/three-layer-validation.ru.md)
- [Эталонный клиент](../overview/reference-client.ru.md)
- [Слой скриптов](../usage/scripts-layer.ru.md)
