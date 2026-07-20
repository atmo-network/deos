---
page_type: concept
title: Токеновые поверхности
summary: Краткая карта главных token surfaces DEOS/TMCTOL, включая Native, VETO, BLDR, stNTVE, LP tokens и роли токенов в экономике, governance, staking и read-model boundaries.
locale: ru
canonical_page_id: token-surfaces
translation_of: token-surfaces.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../docs/tmctol.specification.en.md
  - ../../docs/governance.specification.en.md
  - ../../docs/staking.specification.en.md
  - ../../template/primitives/src/ecosystem.rs
status: active
audience: newcomer
tags:
  - concept
  - tokens
  - economics
  - governance
related:
  - Стандарт TMCTOL
  - Пулы стейкинга
  - Домены Governance
  - Экономика $BLDR
  - Token Minting Curve
  - Разделение read-model
last_compiled: 2026-07-20
confidence: 0.9
---

# Токеновые поверхности

## Кратко

DEOS одновременно использует несколько token surfaces. Они не взаимозаменяемы. Одни токены выражают экономику протокола, другие отвечают за protection или tactical governance, третьи являются receipts для позиций в staking или liquidity systems.

Эта страница — компактная карта. Она не заменяет точные формулы, governance rules или runtime constants, которыми владеют другие страницы.

## Главные токены

### Native / `$NTVE`

`$NTVE` — sovereign base token текущей эталонной линии. Он якорит native staking, может входить в пару с `stNTVE` для collator-security LP custody и участвует в protocol/network governance hierarchy.

### `$VETO`

`$VETO` — protection token, а не второй ordinary governance token. Его задача — constitutional safety: он может блокировать или защищать стратегические protocol/network changes, но не должен становиться позитивной daily-control surface.

### `$BLDR`

`$BLDR` — флагманский токен тактического управления для координации созидателей. В текущей линии он связан с управлением через счета-заявки, финансированием труда, ликвидностью под контролем протокола и специализированными контурами координации BLDR. Полный паттерн описывает [Экономика $BLDR](builder-economy.ru.md). Его ценность не сводится к эмиссии: она зависит от того, создаст ли производная экосистема реальный спрос на эту поверхность координации.

## Receipt и position tokens

### `stNTVE`

`stNTVE` — native liquid staking receipt. Он представляет share-vault ownership, а не прямую collator nomination сам по себе. Collator security использует явную locked `NTVE/stNTVE` LP custody.

### LP tokens

LP tokens представляют позиции в AMM pools. Некоторые LP tokens могут стать входом для protocol automation: actor может получить LP, выполнить unwind, разделить outputs или использовать позицию в treasury/staking flows согласно execution plan.

### `stXXX`

`stXXX` — общее семейство staking receipt assets. Native receipts и foreign receipts используют разные namespaces, чтобы derivation receipts не конфликтовали друг с другом.

## Границы monetary policy

Текущая wiki специально разделяет три вопроса:

- **Emission math**: [Token Minting Curve](../overview/token-minting-curve.ru.md) и [Формулы TMCTOL](../math/tmctol-formulas.ru.md).
- **Governance power**: [Домены Governance](governance-domains.ru.md).
- **Координация созидателей**: [Экономика $BLDR](builder-economy.ru.md).
- **Receipt value**: [Пулы стейкинга](staking-pools.ru.md) и учет liquidity positions.

Не выводите полную monetary policy каждого токена только из governance-role. Например, `$BLDR` может быть важен для tactical governance, но wiki не должна притворяться, что downstream demand, launch allocation или ecosystem product-market fit уже решены внутри framework.

## Правило read-model

Token balances и ограниченные receipt/projection data могут быть прямой runtime truth. Long-range holder analytics, historical valuation, portfolio discovery по множеству assets и нарративы спроса — materialized или downstream-product concerns.

Используйте [Разделение read-model](read-model-split.ru.md), чтобы решить, к какой поверхности относится token datum.

## Связанные страницы

- [Стандарт TMCTOL](tmctol-standard.ru.md)
- [Пулы стейкинга](staking-pools.ru.md)
- [Домены Governance](governance-domains.ru.md)
- [Экономика $BLDR](builder-economy.ru.md)
- [Token Minting Curve](../overview/token-minting-curve.ru.md)
- [Разделение read-model](read-model-split.ru.md)
