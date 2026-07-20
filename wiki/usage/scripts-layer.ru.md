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
last_compiled: 2026-07-20
confidence: 0.9
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

### Оркестраторы

Именованные workflow-скрипты собирают атомарные шаги в более крупные процессы:

- `bootstrap-local-network.sh` — собрать runtime, подготовить спецификацию и запустить локальную сеть с клиентом
- `validate-local.sh` — выполнить выбранный план локального аудита, сборки и сквозных проверок
- `aaa-release-gate.sh` — запустить тяжелые нагрузочные тесты планировщика AAA
- `benchmarks.sh` — скомпилировать benchmarks runtime и сформировать weights

## Административные утилиты

Административные скрипты помогают операторам проверять готовность локальной или действующей сети, не скрывая границы полномочий.

Важные примеры:

- `seed-web-client-state.sh` — подготовить состояние кошелька, свапа и нативного стейкинга для проверки web-client в действующей сети
- `export-papi-metadata.sh` — экспортировать метаданные Rust runtime и пересобрать дескрипторы PAPI для web-client
- `bootstrap-native-staking-local.sh check` — проверить готовность начальной настройки нативного стейкинга без отправки транзакций
- `bootstrap-native-staking-local.sh prepare-calls` — подготовить данные следующего вызова Root, governance staking-admin или подписанного оператора для регистрации и настройки нативного стейкинга, создания канонического пула `NTVE/stNTVE` или начального внесения ликвидности
- `authorized-upgrade-local.sh check` — проверить, совпадает ли hash локального Wasm с ожидающим авторизованным обновлением runtime в сети
- `authorized-upgrade-local.sh apply` — отправить уже авторизованный код runtime только при явном запросе
- `teardown-local-network.sh` — аккуратно остановить фоновые процессы и удалить временное состояние сети

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
- [Статус разработки](../development/status.ru.md)
