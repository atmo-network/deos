---
page_type: concept
title: UI Kit и Domain DAG
summary: Веб-клиент DEOS держит повторяемые элементы интерфейса в UI Kit и использует Domain DAG, чтобы границы владения оставались явными между виджетами, layout, доменами, адаптерами и системной связкой.
locale: ru
canonical_page_id: ui-kit-and-domain-dag
translation_of: ui-kit-and-domain-dag.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../docs/web-client.architecture.en.md
  - ../../web-client/README.md
  - ../../web-client/src/lib/ui/README.md
status: active
audience: newcomer
tags:
  - web-client
  - ui-kit
  - domain-dag
  - frontend-architecture
related:
  - Эталонный клиент
  - Разделение read-model
  - Структура репозитория
  - Руководство контрибьютора
last_compiled: 2026-05-17
confidence: 0.92
---

# UI Kit и Domain DAG

## Кратко

Веб-клиент DEOS использует два взаимодополняющих архитектурных инструмента:

- `UI Kit` владеет повторяемыми визуальными примитивами.
- `Domain DAG` владеет локальной дисциплиной зависимостей и владения.

Вместе они предотвращают дублирование raw-controls, размытые `shared/` buckets, скрытый доступ к adapter internals и неясные границы между widgets, layout, domains, adapters и system wiring.

## Контракт UI Kit

UI Kit живет в `web-client/src/lib/ui/`. Он владеет presentation-only primitives и interaction wrappers: кнопками, карточками, notices, badges, detail rows, полями форм, select fields, textareas, popovers, side panels, read-model badges, форматированием отображения и склейкой классов.

Его задача — сделать безопасный путь путем по умолчанию:

- Reusable buttons по умолчанию имеют `type="button"`;
- Form primitives владеют связкой label/control и hydration-safe ids;
- Class merging централизован;
- Widgets передают доменные labels, values и callbacks в primitives, а не пересобирают primitive behavior локально.

UI Kit должен оставаться foundation-only. Он не должен импортировать market, governance, wallet, adapter или другие product slices.

## Контракт Domain DAG

Domain DAG — локальный gate владения клиента, настроенный в `web-client/domain-dag.json`.

Он проверяет, что:

- Imports остаются ацикличными;
- Source files сохраняют ownership headers;
- Generic shared buckets не возвращаются;
- Widgets не импортируют concrete adapter internals;
- UI Kit не импортирует product domains;
- Domain slices не скрывают владение обходом public entrypoints;
- Widget size и callback/function surfaces остаются видимыми warning signals.

Цель не в том, чтобы плодить папки. Цель в том, чтобы существующие папки говорили правду.

## Правило размещения

Перед добавлением helper используйте такой маршрут:

- Reusable visual primitive: `src/lib/ui/`;
- Browser/session infrastructure: `src/lib/system/`;
- Wallet account или signer concern: `src/lib/wallet/`;
- Governance labels, payload helpers или projections: `src/lib/governance/`;
- Transport implementation: `src/lib/adapters/`;
- Economic/product composition: `src/lib/widgets/`;
- Широкий cross-cutting contract: root foundation files вроде `read-model.ts` или `economics.ts`.

Если ничего не подходит, generic `shared/` folder все равно не лучший default. Сначала определите домен-владелец.

## Карта для новичка

Читайте дерево клиента так: UI Kit — переиспользуемая инфраструктура отображения, widgets — видимые product actions, domain slices владеют state и contracts, adapters говорят с внешними системами, а system собирает shell/session wiring.

## Связанные страницы

- [Эталонный клиент](../overview/reference-client.ru.md)
- [Разделение read-model](read-model-split.ru.md)
- [Структура репозитория](../implementation/repository-structure.ru.md)
- [Руководство контрибьютора](../community/contributing.ru.md)
