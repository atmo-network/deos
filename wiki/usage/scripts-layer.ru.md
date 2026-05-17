---
page_type: usage
title: Слой скриптов
summary: Локальный слой автоматизации для разработчиков и операторов DEOS — bootstrap, сборка, metadata export, authorized-upgrade checks, native staking readiness и plan-only подготовка call data для запуска `NTVE/stNTVE` pool.
locale: ru
canonical_page_id: scripts-layer
translation_of: scripts-layer.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../scripts/README.md
status: active
audience: developer
tags:
  - usage
  - automation
  - tooling
related:
  - Структура репозитория
  - Обзор фреймворка DEOS
last_compiled: 2026-04-25
confidence: 0.96
---

# Слой скриптов

## Кратко

Директория `/scripts` — это практический слой автоматизации для разработчиков и операторов DEOS. Здесь лежат атомарные Bash-скрипты, более крупные оркестраторы и административные утилиты, которые помогают запускать локальную сеть, собирать runtime, проверять состояние, готовить call data и выполнять служебные операции.

## Классы скриптов

Архитектура специально делит автоматизацию на понятные классы.

### Атомарные скрипты

Пронумерованные скрипты делают одну конкретную операцию и не оркестрируют друг друга. Примеры:

- `01-download-binaries.sh` — скачать бинарники Polkadot SDK
- `03-build-runtime.sh` — собрать Wasm-артефакт runtime
- `05-spawn-zombienet.sh` — поднять локальную сеть
- `07-seed-web-client-state.sh` — подготовить локальное состояние wallet/swap/native-staking для live-тестирования web-client

### Оркестраторы

Именованные workflow-скрипты собирают атомарные шаги в более крупные процессы:

- `Bootstrap-local-network.sh` — собрать runtime, подготовить спецификацию и запустить локальную сеть с клиентом
- `Validate-local.sh` — прогнать локальный набор CI/release/E2E проверок
- `Aaa-release-gate.sh` — запустить тяжелые stress-тесты для AAA scheduler
- `Benchmarks.sh` — запустить компиляцию runtime benchmarks и генерацию weights

## Административные утилиты

Административные скрипты помогают операторам проверять локальную или live-chain готовность, не скрывая границы полномочий.

Важные примеры:

- `Export-papi-metadata.sh` — экспортировать Rust runtime metadata и пересобрать PAPI descriptors для web-client
- `Bootstrap-native-staking-local.sh check` — прочитать готовность native staking bootstrap без отправки транзакций
- `Bootstrap-native-staking-local.sh prepare-calls` — выпустить следующий plan-only Root/governance staking-admin или signed operator call data для registration/initialization native staking, создания canonical `NTVE/stNTVE` pool или начального liquidity seeding
- `Authorized-upgrade-local.sh check` — проверить, совпадает ли локальный Wasm hash с pending authorized runtime upgrade on-chain
- `Authorized-upgrade-local.sh apply` — отправить already-authorized runtime code bytes только при явном запросе
- `Teardown-local-network.sh` — аккуратно остановить фоновые процессы и удалить временное состояние сети

## Native staking bootstrap helpers

Native staking bootstrap path разделен на два безопасных для оператора инструмента:

1. `bootstrap-native-staking-local.sh prepare-calls` читает live state и готовит следующие call data для production/operator path.
2. `bootstrap-native-staking-local.sh check` проверяет готовность canonical `NTVE/stNTVE` pool, native staking exchange rate и неактивного native staking LP provisioning actor.

Оба helper-а по умолчанию работают в режиме plan/read-only. Preparation helper никогда не подписывает и не отправляет транзакции; он только выводит call data и ожидаемую authority для каждого шага.

## Общие соглашения

Именованные и административные скрипты следуют одному и тому же каркасу:

1. `usage`
2. `parse_args`
3. `check_prerequisites` или `plan`
4. `main`

Они опираются на `_common.sh`, чтобы одинаково вести логи, отмечать шаги и управлять фоновыми процессами. Все такие скрипты должны поддерживать `--help`.

## Связанные страницы

- [Структура репозитория](../implementation/repository-structure.ru.md)
- [Обзор фреймворка DEOS](../overview/deos-framework.ru.md)
- [Технологический стек](../implementation/tech-stack.ru.md)
- [Трехуровневая валидация](../development/three-layer-validation.ru.md)
- [Troubleshooting validation](validation-troubleshooting.ru.md)
- [Статус разработки](../development/status.ru.md)
