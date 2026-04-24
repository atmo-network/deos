---
page_type: overview
title: Обзор Governance
summary: Широкая карта DEOS Governance, которая объясняет, зачем нужен этот слой, как он разделяет стратегию и тактику, какие большие элементы в нем важны и по каким страницам идти дальше.
locale: ru
canonical_page_id: governance-overview
translation_of: governance-overview.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../docs/governance.specification.en.md
  - ../../docs/governance.architecture.en.md
  - ../../docs/manifesto.en.md
status: active
audience: newcomer
tags:
  - overview
  - governance
  - domains
related:
  - Домены Governance
  - Physics-first против politics-first
  - Пулы стейкинга
  - Базовые термины
last_compiled: 2026-04-20
confidence: 0.94
---

# Обзор Governance

## Кратко

DEOS Governance — это ограниченный governance-слой фреймворка. Его задача не в том, чтобы подменить экономическое ядро бесконечной политикой, а в том, чтобы управлять стратегическими, тактическими и социальными поверхностями, которые остаются после механизации протокола.

Эта страница — широкая карта подсистемы. Она объясняет, зачем governance вообще нужен, как устроена текущая эталонная линия и по каким страницам идти дальше, если нужен более точный разбор.

## Зачем Governance Вообще Нужен

Мировоззрение DEOS — это не «все должно решаться token voting». Скорее логика такая:

- Протокольную физику нужно механизировать там, где это возможно
- Стратегическую защиту нужно держать явной
- Тактические и treasury-решения все равно требуют ограниченной человеческой координации
- Governance-surface должен оставаться queryable и конституционно понятным

Поэтому DEOS Governance больше похож на конституционный слой над детерминированным ядром, чем на обычный voting-portal.

## Три Больших Разделения

Форма governance здесь в основном определяется тремя крупными разделениями:

- `Стратегия` не равна `тактике`
- `Одобрение` не равно `защите`
- `Живая on-chain governance-истина` не равна `архивной истории`

Если держать в голове эти три различия, большая часть модели читается намного легче.

## Как Выглядит Текущая Линия

На высоком уровне текущая эталонная линия содержит:

- Явные governance-домены вместо одного безликого пула референдумов
- Dual-track модель: primary lane плюс protection lane
- Типизированные payload-kind вместо непрозрачных «proposal blobs»
- Публичный ordinary-ритм с `3-дневным` lead-in, `7-дневными` voting windows и `3-дневным` enactment delay
- Ограниченные on-chain query surfaces для живого governance UX

Не обязательно сразу запоминать все детали. Главная мысль в том, что DEOS Governance старается держать поверхности власти явными и ограниченными.

## Стратегический и Тактический Governance

Текущая эталонная линия проводит жесткую границу между стратегическими и тактическими решениями.

На практике это означает:

- Стратегический governance защищает протокольные и сетевые subject-ы
- Тактический governance занимается более узкими доменно-локальными тратами и координацией
- Тактические домены не наследуют стратегическую authority автоматически

Это одна из главных причин существования DEOS Governance в текущем виде: он пытается не превращать все важные решения в один плоский voting market.

## Почему Здесь Два Трека

DEOS Governance изначально двухконтурный:

- `Primary track` отвечает на судьбу proposal-а
- `Protection track` отвечает на то, должен ли proposal быть конституционно заблокирован или процедурно пропущен дальше

Это свойство всей подсистемы, а не деталь одного частного proposal family. Именно здесь находится одно из главных отличий DEOS Governance от простых моделей «Aye / Nay для всего».

Если нужен более точный разбор того, как protection-layer привязывается к конкретным governance-cell, читайте [Домены Governance](../concepts/governance-domains.ru.md).

## О Чем Governance Вообще Может Говорить

Текущий governance-словарь намеренно остается небольшим. Proposal-ы описываются через явные payload-kind, например:

- Стратегическое Root-эквивалентное действие
- Тактическая treasury-трата
- Тактическое parameter-изменение
- Advisory-волеизъявление внутри домена
- Тактический сигнал в стратегический слой

Точные значения payload-kind важны, но на уровне overview мысль проще: governance-действия здесь типизированы специально. DEOS не хочет, чтобы смысл proposal-а прятался в социальную договоренность или непрозрачные bytes.

## Как Ощущается Публичный Жизненный Цикл

Для новичка важны такие факты о lifecycle:

- Protection открывается сразу после submission
- Ordinary primary не стартует мгновенно
- Успешное approval может еще ждать в enactment delay
- Execution failure — это настоящий state, а не скрытая деталь после слова «approved»
- Недавние finalized outcomes видны on-chain только ограниченное время

То есть DEOS Governance спроектирован так, чтобы показывать честное живое состояние, а не просто сырую кучу исторических событий.

## Governance и Read Model

Runtime экспортирует bounded governance views для живого продуктового использования. Туда входят proposal state, timing, интерпретация tally, execution authority, payload availability и recent finalized outcomes.

Это сделано специально. DEOS хочет, чтобы канонический live governance UX можно было читать on-chain, а длинные archive/search/timeline surfaces оставались задачей indexed или materialized слоев.

## Governance и Staking

Governance также передает ограниченный сигнал о качестве участия в staking rewards.

Смысл на уровне overview не в том, что governance и staking — одна подсистема. Смысл в том, что DEOS считает качество governance экономически важным, но при этом отказывается хранить бесконечную социальную историю в runtime storage.

## Как Читать Governance-Кластер Wiki

Лучший порядок такой:

1. `Обзор Governance` — зачем нужна вся подсистема
2. [Домены Governance](../concepts/governance-domains.ru.md) — как типизируется одна governance-cell
3. `Базовые термины` — для повторяющегося словаря
4. `Physics-first против politics-first` — для философской рамки

Overview — это карта. Concept pages — это более близкое рассмотрение отдельных строительных блоков.

## Связанные страницы

- [Домены Governance](../concepts/governance-domains.ru.md)
- [Physics-first против politics-first](../comparisons/physics-vs-politics.ru.md)
- [Пулы стейкинга](../concepts/staking-pools.ru.md)
- [Базовые термины](../glossary/core-terms.ru.md)

## Источники

- `docs/governance.specification.en.md`
- `docs/governance.architecture.en.md`
- `docs/manifesto.en.md`
