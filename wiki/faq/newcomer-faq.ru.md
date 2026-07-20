---
page_type: faq
title: FAQ для новичков
summary: Короткий самодостаточный FAQ по главным вопросам новичков о DEOS, TMCTOL, AAA, governance, стейкинге, поверхностях данных, доменах wiki и эталонном клиенте.
locale: ru
canonical_page_id: newcomer-faq
translation_of: newcomer-faq.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../README.md
  - ../../docs/README.md
  - ../../docs/manifesto.en.md
  - ../../docs/aaa.specification.en.md
  - ../../docs/governance.specification.en.md
  - ../../docs/staking.specification.en.md
  - ../../docs/read-model.contract.en.md
  - ../../docs/web-client.architecture.en.md
  - ../../web-client/README.md
status: active
audience: newcomer
tags:
  - faq
  - onboarding
related:
  - Карта доменов
  - Обзор фреймворка DEOS
  - Первые шаги
  - Маршруты чтения
  - Система AAA
  - Physics-first против politics-first
  - UI Kit и Domain DAG
  - Generated Wiki
  - Базовые термины
last_compiled: 2026-07-20
confidence: 0.85
---

# FAQ для новичков

## Кратко

На этой странице собраны ответы на вопросы, которые обычно возникают первыми: что такое DEOS, как в него вписывается TMCTOL, что контролирует governance, как на высоком уровне устроены AAA и staking, как организована wiki и насколько честным должен быть эталонный клиент.

Используйте [Карту доменов](../concepts/domain-map.ru.md), когда нужна общая форма системы, и [Маршруты чтения](../getting-started/reading-paths.ru.md), когда у вас есть конкретная задача.

## Идентичность и старт

**DEOS — это токен или стандарт?** Нет. `DEOS` — это фреймворк и эталонный стек. `TMCTOL` — текущий флагманский токеномический стандарт, работающий поверх него.

**Почему wiki организована по доменам?** Потому что DEOS проще понимать как взаимодействующие домены, а не как список pallet-ов: экономическая физика, автономные акторы, маршрутизация, governance, staking, модели чтения, клиентский UX, инструменты и future gates.

**С чего начать?** Если нужен самый короткий путь, прочитайте [Обзор фреймворка DEOS](../overview/deos-framework.ru.md), [Базовые термины](../glossary/core-terms.ru.md) и [Стандарт TMCTOL](../concepts/tmctol-standard.ru.md). Если вы собираетесь что-то менять, используйте [Маршруты чтения](../getting-started/reading-paths.ru.md).

## Экономика, governance и акторы

**Почему TMCTOL избегает redemption?** Потому что текущий стандарт трактует minting как однонаправленную физику протокола, а не как выход из резервов. Смотрите [Стандарт TMCTOL](../concepts/tmctol-standard.ru.md) и [Token Minting Curve](../overview/token-minting-curve.ru.md).

**Исчезает ли governance?** Нет. Governance остается, но его роль сужена: он задает направление, тактические домены и ограниченные пути обновлений, а не вручную управляет survival physics. Смотрите [Обзор Governance](../overview/governance-overview.ru.md) и [Домены Governance](../concepts/governance-domains.ru.md).

**Что значит deterministic?** Реакции под управлением протокола явны и повторяемы для одного и того же состояния chain. Это не значит, что рынки становятся предсказуемыми.

**Что такое AAA и AA-Актор?** `AAA` — вся система Account Abstraction Actors: scheduler, правила жизненного цикла, execution plans, actor accounts и task execution. `AA-Актор` — один конкретный runtime-экземпляр внутри этой системы. Смотрите [Систему AAA](../overview/aaa-system.ru.md) и [AA-Актор](../overview/aa-actor.ru.md).

**Как устроен staking?** Staking — это домен multi-asset share-vault. [Пулы стейкинга](../concepts/staking-pools.ru.md) объясняют native `stNTVE`, LP nomination и snapshots наград.

## Данные, клиент и границы wiki

**Почему on-chain vs materialized data так важны?** Честность продукта зависит от того, видно ли, где каноническая chain-истина, а где индексированные или производные данные. Смотрите [Разделение read-model](../concepts/read-model-split.ru.md).

**Web client — источник истины?** Нет. Web client — эталонная продуктовая поверхность, которая должна честно маркировать происхождение данных. Смотрите [Эталонный клиент](../overview/reference-client.ru.md).

**Где живут версии релизов и status notes?** История релизов живет в changelog, открытая работа — в backlog, а текущее состояние для новичков — в [Статусе разработки](../development/status.ru.md). Architecture и wiki pages должны объяснять implementation truth и границы.

**Что такое UI Kit и Domain DAG?** Это клиентские дисциплины против дублирования и размытых границ владения. Смотрите [UI Kit и Domain DAG](../concepts/ui-kit-and-domain-dag.ru.md).

**Почему web client может рендерить wiki markdown напрямую?** Wiki markdown — доверенный repo-local content под проверкой репозитория, а не произвольный пользовательский ввод. Смотрите [Generated Wiki](../concepts/generated-wiki.ru.md).

## Связанные страницы

- [Карта доменов](../concepts/domain-map.ru.md)
- [Обзор фреймворка DEOS](../overview/deos-framework.ru.md)
- [Первые шаги](../getting-started/first-steps.ru.md)
- [Маршруты чтения](../getting-started/reading-paths.ru.md)
- [Система AAA](../overview/aaa-system.ru.md)
- [Physics-first против politics-first](../comparisons/physics-vs-politics.ru.md)
- [UI Kit и Domain DAG](../concepts/ui-kit-and-domain-dag.ru.md)
- [Generated Wiki](../concepts/generated-wiki.ru.md)
- [Базовые термины](../glossary/core-terms.ru.md)
