---
page_type: overview
title: Эталонный клиент
summary: Веб-клиент DEOS — on-chain-first эталонный интерфейс для живых протокольных сценариев. Он разделяет product widgets и layout, централизует UI-примитивы, проверяет владение через Domain DAG и показывает происхождение данных.
locale: ru
canonical_page_id: reference-client
translation_of: reference-client.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../docs/web-client.architecture.en.md
  - ../../web-client/README.md
  - ../../web-client/src/lib/ui/README.md
  - ../../docs/read-model.contract.en.md
status: active
audience: newcomer
tags:
  - overview
  - web-client
  - product
  - ui-kit
  - domain-dag
related:
  - Первые шаги
  - Разделение read-model
  - FAQ для новичков
  - Базовые термины
last_compiled: 2026-07-24
confidence: 0.9
---

# Эталонный клиент

## Кратко

Локальный веб-клиент в репозитории — браузерный эталонный клиент DEOS. Он `on-chain-first`: главные живые продуктовые сценарии должны опираться на ограниченные канонические runtime-поверхности, а не тихо зависеть от off-chain реконструкции.

Модель владения явная: widgets выражают product actions, layout владеет pane/lane mechanics, UI Kit владеет reusable presentation primitives, system владеет browser/session wiring, а adapters остаются transport boundaries.

## Контракт продукта и layout

Текущие продуктовые поверхности включают балансы, предварительный расчет маршрутов, управление, состояние автоматизации и стейкинга, ограниченные сессией графики и чтение вики.

Поверхность автоматизации напрямую читает известных системных акторов из ограниченных `ActorHot`, `ActorProgram` и разреженного `ContinuationState`. Она показывает номер логического запуска, приостановленную попытку, нерешенный шаг и блок последней попытки. Это текущее каноническое состояние, а не архив попыток. Контракт создания планов разрешает `RetryLater` только изменяемым акторам.

Клиент отделяет экономические функции от инфраструктуры размещения:

- Widgets — видимые product surfaces вроде swap, wallet, governance, charts, staking, automation и wiki;
- Layout — pane, tile, split, tab, footer, header, sidebar и reserved-lane machinery;
- Reserved edge lanes — developer-configured shell zones, а не экономические панели, которые пользователь свободно переставляет.

Widgets адаптируются к ширине и высоте pane, а не предполагают только desktop stack. Переиспользуемый каркас владеет иерархией задач, минимальными фазами, доступностью, происхождением данных и safety-состояниями; downstream-форки владеют брендом, палитрой, типографикой, эффектами, терминологией и включенной продуктовой политикой.

## Адаптивность и работа без сети

Desktop workspace сохраняет ограниченное дерево плиток с изменением размера только соседних панелей. Ниже мобильного порога клиент проецирует те же панели в одномерный аккордеон с одной раскрытой задачей и отдельным сохраненным порядком; перемещение или раскрытие мобильной задачи не переписывает desktop-топологию. Область аккаунта и настроек становится модальной нижней панелью с удержанием фокуса, а не вытесняет workspace.

Отсутствие chain-данных никогда не превращается в выдуманный ноль. Widgets различают загрузку, живой снимок, сохраненные устаревшие данные, явный preview, ненастроенного провайдера и ошибку. Выбор локального аккаунта, поиск signer, работа с адресом и черновиками, чтение Wiki и receipts могут оставаться полезными без сети, тогда как действия на основе баланса, котировки и исполнение требуют живого канонического снимка.

## Владение и feedback

Клиент использует дисциплины UI Kit и Domain DAG, чтобы повторяемые controls и структурные границы жили в owner layers. Widgets должны выражать product intent, а не пересобирать primitive controls и не лезть в adapter internals.

Execution feedback централизован: `LogWidget` — главная transaction/progress surface, а action widgets остаются сфокусированы на запуске действий. Это тот же anti-duplication rule, что и для UI primitives и provenance badges.

## Границы данных и wiki

Клиент должен честно маркировать и protocol provenance, и browser realization. Session-built views не должны притворяться retained archive truth. Длинная аналитика и архивы принадлежат indexed или materialized providers, а не прямой chain truth.

Каноническая модель данных описана в [Разделении read-model](../concepts/read-model-split.ru.md).

Веб-клиент рендерит generated wiki как trusted repo-local markdown и использует скомпилированные metadata для навигации, алиасов, graph links, state и provenance. Граница доверия и правила развития wiki описаны в [Generated Wiki](../concepts/generated-wiki.ru.md).

## Связанные страницы

- [Первые шаги](../getting-started/first-steps.ru.md)
- [Разделение read-model](../concepts/read-model-split.ru.md)
- [Generated Wiki](../concepts/generated-wiki.ru.md)
- [FAQ для новичков](../faq/newcomer-faq.ru.md)
- [Базовые термины](../glossary/core-terms.ru.md)
