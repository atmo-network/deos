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

- `03-build-runtime.sh` — собрать Wasm-артефакт runtime
- `05-spawn-zombienet.sh` — поднять локальную сеть
- `06-network-smoke.sh` — проверить ограниченное продвижение финализации relay chain и parachain
- `07-network-e2e.sh` — подтвердить одну подписанную финализированную передачу по событиям и storage действующей сети
- `08-session-transition.sh` — наблюдать одну финализированную смену сессии через RPC обоих коллаторов
- `09-composed-economic-path.sh` — сверить финализированный путь DEOS Router, DEOS Oracle и Burn Actor по событиям и storage

### Оркестраторы

Именованные workflow-скрипты собирают атомарные шаги в более крупные процессы:

- `bootstrap-local-network.sh` — собрать runtime, подготовить спецификацию и запустить локальную сеть с клиентом
- `validate-local.sh` — выполнить выбранный план локального аудита, сборки и сквозных проверок
- `actors-assurance.sh` — выполнить тяжелые проверки нагрузки и пропускной способности планировщика Actors
- `network-assurance-local.sh` — объединить проверки топологии, финализации, переключения коллаторов, перезапуска и подписанной передачи; `SESSION_TRANSITION=1` добавляет многочасовую проверку смены сессии, а `COMPOSED_PATH=1` — финализированные свидетельства DEOS Router, DEOS Oracle и Burn Actor
- `benchmarks.sh` — скомпилировать benchmarks runtime и сформировать weights

## Административные утилиты

Административные скрипты помогают операторам проверять готовность локальной или действующей сети, не скрывая границы полномочий.

Важные примеры:

- `seed-web-client-state.sh` — подготовить состояние кошелька, свапа и нативного стейкинга для проверки web-client в действующей сети
- `export-papi-metadata.sh` — экспортировать метаданные Rust runtime и пересобрать дескрипторы PAPI для web-client
- `bootstrap-native-staking-local.sh check` — проверить готовность начальной настройки нативного стейкинга без отправки транзакций
- `bootstrap-native-staking-local.sh prepare-calls` — подготовить данные следующего вызова Root, governance staking-admin или подписанного оператора для регистрации и настройки нативного стейкинга, создания канонического пула `NTVE/stNTVE` или начального внесения ликвидности
- `authorized-upgrade-local.sh check` — зафиксировать финализированную версию runtime, сравнить код сети и локальный код, проверить право стратегической подачи, эмиссию `$VETO` и ожидающий авторизованный hash без отправки транзакций
- `authorized-upgrade-local.sh prepare-authorization` — подготовить связанные с кандидатом данные вызовов для стейкинга, preimage и стратегического предложения без подписи; вызов protection `Pass` недоступен до готовности жизненного цикла
- `authorized-upgrade-local.sh apply` — отправить уже авторизованный код runtime только при явном запросе
- `authorized-upgrade-local.sh snapshot|verify` — зафиксировать финализированное непустое исходное состояние и проверить точное сохранение DEOS Router, DEOS Oracle, Actors, версии runtime и кода кандидата после обновления
- `teardown-local-network.sh` — аккуратно остановить фоновые процессы и удалить временное состояние сети

## Native staking bootstrap helpers

Native staking bootstrap path разделен на два безопасных для оператора инструмента:

1. `bootstrap-native-staking-local.sh prepare-calls` читает live state и готовит следующие call data для production/operator path.
2. `bootstrap-native-staking-local.sh check` проверяет готовность canonical `NTVE/stNTVE` pool, native staking exchange rate и неактивного Native Staking Liquidity Actor.

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
