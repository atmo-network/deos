---
page_type: concept
title: Домены Governance
summary: Governance-домен — это одна типизированная governance-ячейка внутри большой governance-системы. Он связывает управляемый предмет, voting/protection surfaces, допустимые payload-family, cadence и полномочия исполнения.
locale: ru
canonical_page_id: governance-domains
translation_of: governance-domains.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../docs/governance.specification.en.md
  - ../../docs/governance.architecture.en.md
  - ../../docs/staking.specification.en.md
status: active
audience: newcomer
tags:
  - concept
  - governance
  - domains
related:
  - Обзор Governance
  - Обзор фреймворка DEOS
  - Стандарт TMCTOL
  - Пулы стейкинга
  - Разделение read-model
  - Physics-first против politics-first
  - Базовые термины
last_compiled: 2026-05-17
confidence: 0.93
---

# Домены Governance

## Кратко

Governance-домен — это одна типизированная governance-ячейка внутри DEOS Governance. Она говорит runtime и пользователю, что именно управляется, чьи голоса считаются, какая protection surface может вмешаться, какие proposal-family допустимы и до каких полномочий может дойти успешное исполнение.

[Обзор Governance](../overview/governance-overview.ru.md) объясняет всю подсистему. Эта страница объясняет единицу, которая не дает подсистеме схлопнуться в один плоский voting market.

## Контракт домена

Домен связывает шесть вещей:

- Управляемый предмет;
- Primary voting surface;
- Protection voting surface;
- Допустимые payload-kind;
- Cadence-правила;
- Максимальные полномочия исполнения.

Такой контракт превращает governance из расплывчатого социального процесса в типизированную policy. Он также делает конкретными четыре оси governance: `GovernanceDomain`, `CadenceMode`, `ProposalPayloadKind` и `ProtectionTrack`.

Поэтому два домена могут отличаться тем, кто голосует, кто защищает, какие payload допустимы, является ли primary track `Binary` или `Invoice`, открыт ли signed public submission, нужны ли fees или urgent handling и какой authority может достичь payload.

## Текущая эталонная форма

В текущей линии особенно заметны две пары:

- `Native + $VETO` для протокольной и сетевой стратегии;
- `$BLDR + Native` для флагманского тактического домена.

Стратегические proposal защищаются `$VETO`. Флагманский тактический домен защищается native staking weight. Эти пары не символические: они определяют voting power, protection power и легитимную глубину исполнения.

Обычный публичный cadence сейчас общий:

- `3 дня` lead-in;
- `7 дней` protection window;
- `7 дней` primary voting window;
- `3 дня` enactment delay.

Signed public path остается намеренно ограниченным: `Intent` во всех доменах, тактический `$BLDR` `L2SignalToL1` и тактический `$BLDR` `L2TreasurySpend`.

## Tracks, payload и authority

Форма primary track принадлежит домену. Одни домены используют binary family `Aye / Nay`. Канонический тактический `$BLDR` treasury-домен использует invoice-shaped family: `Amplify`, `Approve`, `Reduce`, `Nay`. Тактической трате иногда нужен payout scalar, а не только yes/no approval.

Protection тоже имеет доменную форму. Домен решает, какая protection surface допустима, какие veto thresholds важны, может ли `Pass` ускорять urgent handling и как интерпретируется финальный protection gate.

Execution authority ограничивается доменом. `L1RootAction` — стратегический и Root-эквивалентный. `L2TreasurySpend` — domain-local treasury execution. `L2ParameterChange` должен оставаться внутри реально делегированных domain-owned surfaces. `Intent` и `L2SignalToL1` остаются advisory по контракту.

Некоторые соблазнительные поверхности остаются вне тактического доменного владения: TMC launch physics, staking admin onboarding/recovery, AAA global controls и asset-registry registration/migration. Тактический домен должен использовать явную передачу вроде `L2SignalToL1`, а не делать вид, что уже владеет этими зонами.

## Живой read model

Домены формируют governance read model. Domain-aware runtime views показывают ограниченную живую истину: proposal status, timing, интерпретацию tally, полномочия исполнения, submission authority, реальный opening fee, доступность payload и недавние finalized details.

Так домены становятся видны не только в конституционной модели, но и в продуктовой поверхности.

## Ментальная модель

Читайте governance-домен через пять вопросов:

1. Чья это проблема?
2. Чьи голоса здесь считаются?
3. Кто может конституционно это остановить?
4. Какой payload здесь допустим?
5. До каких полномочий может дойти успешное исполнение?

## Связанные страницы

- [Обзор Governance](../overview/governance-overview.ru.md)
- [Обзор фреймворка DEOS](../overview/deos-framework.ru.md)
- [Стандарт TMCTOL](tmctol-standard.ru.md)
- [Пулы стейкинга](staking-pools.ru.md)
- [Разделение read-model](read-model-split.ru.md)
- [Physics-first против politics-first](../comparisons/physics-vs-politics.ru.md)
- [Базовые термины](../glossary/core-terms.ru.md)
