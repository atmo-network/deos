---
page_type: overview
title: Типизированные наблюдения
summary: Типизированные наблюдения предоставляют ограниченную текущую scalar truth, тогда как producers владеют samples, AAA — реакциями, DEOS Router — маршрутизацией, а indexed providers — историей.
locale: ru
canonical_page_id: typed-observations
translation_of: typed-observations.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../template/pallets/oracle/docs/specification.en.md
  - ../../template/pallets/oracle/docs/architecture.en.md
  - ../../docs/oracle.integration.en.md
status: active
audience: newcomer
tags:
  - overview
  - oracle
  - observations
related:
  - DEOS Router
  - Система AAA
  - Автоматизация через токены
  - Разделение read-model
last_compiled: 2026-07-28
confidence: 0.95
---

# Типизированные наблюдения

## Кратко

Типизированные наблюдения задают доменный контракт ограниченной текущей scalar truth. DEOS Oracle — её текущий ограниченный владелец. При регистрации governance фиксирует producer, смысл, scale, правило aggregation, zero policy, контракт freshness и provenance канала.

Подсистема не владеет историей исходных samples, решениями маршрутизации, исполнением акторов или неограниченной аналитикой. Эти обязанности остаются у producers, DEOS Router, AAA и indexed providers.

## Контракт текущей истины

Каждый допущенный канал предоставляет ограниченное текущее состояние:

- неизменяемую семантику канала;
- одно текущее scalar value после инициализации;
- блок последней принятой публикации;
- revision, меняющийся только при изменении результата;
- явные freshness и availability;
- ограниченные индексы producers и feeds.

Равный aggregate output обновляет `updated_at`, но не увеличивает revision и не вызывает change hook. Изменение семантики требует нового feed identity, а не мутации существующего канала.

## Направленные наблюдения пулов

Эталонный runtime DEOS регистрирует прямое и обратное наблюдения пула как разные каналы. Канонический допуск пула транзакционно создает оба направления, а DEOS Router публикует исполняемое направление перед прямым исполнением.

Одно направление нельзя выводить из обратного. Канал записывает резервы до исполнения с provenance Router, но не обещает универсальную справедливую цену, иммунитет к манипуляциям или полную рыночную историю.

## Граница реактивного AAA

Изменение revision вызывает независимый от подписчиков O(1) ingress hook AAA. Hook отмечает только последнюю dirty revision. Отложенный fanout затем обходит точные занятые страницы подписчиков и сходится к существующим pending latch, queue, wakeup и scheduler AAA.

Observation trigger запрашивает повторную проверку последнего состояния. Conditions владеют порогами и проверяют freshness при попытке исполнения актора. DEOS Oracle не исполняет подписчиков синхронно и не обещает отдельный запуск для каждой промежуточной revision.

## Граница read-model

Текущая конфигурация, status, value, revision и update block канала — каноническая ограниченная истина цепи. Исторические samples, длинные timeline, поиск и аналитика принадлежат indexed или materialized providers.

Эталонный клиент читает финализированное текущее состояние и показывает provenance. Он не должен восстанавливать историю из session cache или выдавать provider data за прямую runtime truth.

## Связанные страницы

- [DEOS Router](router.ru.md)
- [Система AAA](aaa-system.ru.md)
- [Автоматизация через токены](../concepts/token-driven-automation.ru.md)
- [Разделение read-model](../concepts/read-model-split.ru.md)
