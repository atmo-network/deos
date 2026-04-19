---
page_type: concept
title: Домены Governance
summary: Governance-домен — это одна типизированная governance-cell внутри большой governance-системы. Он связывает управляемый subject, primary и protection power surfaces, допустимые payload-family, cadence и execution authority.
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
last_compiled: 2026-04-20
confidence: 0.93
---

# Домены Governance

## Кратко

Governance-домен — это одна конкретная governance-cell внутри более широкой DEOS Governance system. Это единица, которая говорит runtime и пользователю, что именно здесь управляется, чья власть считается в primary lane, какая protection-surface может вмешаться, какие proposal-family вообще здесь осмысленны и до какой execution authority результат может реально дотянуться.

Поэтому если [Обзор Governance](../overview/governance-overview.ru.md) объясняет всю подсистему целиком, то эта страница объясняет один из ее главных строительных блоков.

## Что Именно Связывает Домен

Governance-домен — это не просто «namespace для proposal-ов». В DEOS он связывает вместе:

- Управляемый subject
- Primary voting surface
- Protection voting surface
- Допустимые payload-kind
- Cadence-правила, которые тут применяются
- Execution authority, до которой успешный proposal действительно может дойти

Именно это делает домен полезным: он превращает governance-власть из расплывчатой социальной идеи в типизированный и проверяемый контракт.

## Четыре Оси Вокруг Домена

Governance-модель остается компактной за счет сочетания четырех явных осей:

- `GovernanceDomain`
- `CadenceMode`
- `ProposalPayloadKind`
- `ProtectionTrack`

Домен — это место, где эти оси становятся конкретными. Именно здесь идеи вроде «стратегия против тактики», «binary против invoice voting» и «advisory против executable action» превращаются из общей теории в реальную runtime-policy.

## Текущие Канонические Пары Доменов

В текущей эталонной линии особенно заметны две пары:

- `Native + $VETO` для протокольной и сетевой стратегии
- `$BLDR + Native` для флагманского тактического домена

Это означает, что:

- Стратегические proposal-ы защищаются `$VETO`
- Флагманский тактический домен защищается native staking weight

Это не просто символические пары. Они определяют, кто голосует, кто защищает и до какой authority успешный proposal имеет право дотянуться.

## Что Может Отличаться Между Доменами

Два домена могут различаться по всем этим пунктам:

- Какая asset- или stake-surface дает primary voting weight
- Какая surface дает protection voting weight
- Какие payload-kind там вообще осмысленны
- Является ли primary track `Binary` или `Invoice`
- Открыт ли этот path для signed public submission или он admin-only
- Есть ли opening fee для публичного пути
- Разрешен ли urgent-режим
- До какой execution authority payload может дойти при approval

В этом и состоит главная причина существования доменов. DEOS не хочет, чтобы один governance-token и одно voting rule притворялись честным способом управлять всеми слоями системы сразу.

## Primary Track Имеет Доменную Форму

Домен помогает определить семейство primary track.

### Binary family

В binary family primary lane выглядит привычно:

- `Aye`
- `Nay`

### Invoice family

В каноническом тактическом `$BLDR` treasury-домене primary lane имеет invoice-форму:

- `Amplify`
- `Approve`
- `Reduce`
- `Nay`

Это семейство существует потому, что tactical spending не всегда является вопросом yes-or-no. Иногда governance должен выбрать payout scalar, а не только разрешить или запретить transfer.

## Protection Тоже Имеет Доменную Форму

Protection lane — это не одно универсальное veto-правило, одинаковое везде. Домен определяет:

- Какая protection-surface вообще допустима
- Какие raw-veto thresholds важны
- Может ли protection-track `Pass` процедурно ускорить urgent handling
- Как final protection gate должен интерпретироваться в момент resolution

Именно поэтому домен — это конституционная cell, а не просто папка для proposal-ов.

## Публичный Cadence Живет Поверх Доменов

На отгруженной ordinary public line домены сейчас наследуют один и тот же общий ритм:

- `3 дня` lead-in
- `7 дней` ordinary protection window
- `7 дней` ordinary primary window
- `3 дня` enactment delay

Но даже внутри этого общего ритма домены остаются важны, потому что смысл голосов, protection-source и допустимые payload-family все равно различаются.

## Текущая Публичная Submission Scope По Семействам Доменов

Текущий runtime держит signed public submission намеренно ограниченной.

Сейчас публичный path покрывает:

- `Intent` во всех доменах
- Тактический `$BLDR` `L2SignalToL1`
- Тактический `$BLDR` `L2TreasurySpend`

То есть доменная policy определяет не только то, кто голосует. Она также определяет, какие proposal-family вообще открыты для signed public ingress на текущей линии.

## Execution Authority Не Везде Одинакова

Домен не выдает Root-власть магическим образом. Он ограничивает, куда одобренный payload может реально исполниться.

На текущей линии это означает:

- `L1RootAction` — стратегический и Root-эквивалентный
- `L2TreasurySpend` — domain-local treasury execution
- `L2ParameterChange` — только внутри действительно делегированных domain-owned surfaces
- `Intent` и `L2SignalToL1` — advisory по контракту

Это один из самых важных практических смыслов доменов: они не дают тактической и стратегической authority незаметно схлопнуться в одно и то же.

## Почему Некоторые Соблазнительные Surface Остаются Вне Доменного Контроля

Документация прямо говорит, что некоторые system-surface пока не являются честными tactical-domain parameters. На текущей линии это, например:

- TMC launch physics
- Staking admin onboarding или recovery paths
- AAA global controls
- Asset-registry registration или migration

Если тактический домен хочет повлиять на одну из этих system-owned зон, он должен использовать явный handoff вроде `L2SignalToL1`, а не делать вид, что домен уже владеет этой authority.

## Домены и Живой Read Model

Домены формируют и governance read model. Domain-aware runtime views показывают bounded live truth, такую как:

- Proposal status
- Proposal timing
- Интерпретация tally
- Execution authority
- Submission authority
- Opening-fee truth
- Payload availability
- Recent finalized detail

То есть домены видны не только в конституции, но и в живой продуктовой поверхности.

## Простая Ментальная Модель

Если нужен самый короткий полезный mental model, читайте governance-домен так:

- «Чья это проблема?»
- «Чьи голоса тут считаются?»
- «Кто может конституционно это остановить?»
- «Какой payload здесь вообще допустим?»
- «До какой authority это может реально дойти?»

Именно эту концептуальную работу домен выполняет внутри DEOS Governance.

## Связанные страницы

- [Обзор Governance](../overview/governance-overview.ru.md)
- [Обзор фреймворка DEOS](../overview/deos-framework.ru.md)
- [Стандарт TMCTOL](tmctol-standard.ru.md)
- [Пулы стейкинга](staking-pools.ru.md)
- [Разделение read-model](read-model-split.ru.md)
- [Physics-first против politics-first](../comparisons/physics-vs-politics.ru.md)
- [Базовые термины](../glossary/core-terms.ru.md)

## Источники

- `docs/governance.specification.en.md`
- `docs/governance.architecture.en.md`
- `docs/staking.specification.en.md`
