---
page_type: concept
title: Экономические пороги
summary: Wiki-level объяснение главных экономических threshold concepts TMCTOL, включая Gravity Well, Elasticity Inversion, compression, floor/ceiling spread и правило именования метрики.
locale: ru
canonical_page_id: economic-thresholds
translation_of: economic-thresholds.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../docs/tmctol.specification.en.md
  - ../../simulator/README.md
  - ../../AGENTS.md
status: active
audience: newcomer
tags:
  - concept
  - economics
  - tmctol
  - thresholds
related:
  - Стандарт TMCTOL
  - Формулы TMCTOL
  - Token Minting Curve
  - Контур маршрутизации и минтинга
  - Токеновые поверхности
last_compiled: 2026-05-17
confidence: 0.86
---

# Экономические пороги

## Кратко

В TMCTOL есть несколько экономических threshold concepts. Их легко смешать, поэтому wiki держит их отдельно, а не сводит к одному расплывчатому слову `compression`.

Короткое правило: всегда спрашивайте, по какой оси и какой метрике сделано утверждение.

## Gravity Well

`Gravity Well` — это возникающая зона стабилизации, где treasury-owned liquidity становится достаточно большой относительно market capitalization, чтобы гасить волатильность. Это не магическая гарантия цены. Это значит, что protocol-owned liquidity стала экономически значимой и меняет поведение системы.

Сила эффекта зависит от reserves, circulating supply, параметров curve и поведения market/liquidity.

## Elasticity Inversion

`Elasticity Inversion` — порог, после которого расширение supply перестает ухудшать effective floor. До этого порога дополнительный supply может размывать floor support. После него накопление reserves может компенсировать или превзойти это размытие.

Это expanding-supply threshold, а не то же самое, что burn-time compression claim.

## Compression claims

Любой compression claim должен назвать две вещи:

1. **Ось:** burn-time или expanding-supply.
2. **Метрику:** relative spread `C/F` или absolute gap `C - F`.

Где:

- `C` — curve-implied ceiling или mint-side price reference.
- `F` — floor или backing-side support reference.

Burning может снижать ceiling, пока floor support остается стабильным или усиливается, поэтому burn-time compression прямолинейна. Expanding-supply analysis устроен иначе: floor recovery после inversion не означает автоматически, что улучшается каждая compression metric.

## Почему это важно

Без дисциплины метрик четыре разные идеи схлопываются в одну фразу:

- Elasticity inversion;
- Relative compression parity;
- Absolute-gap compression;
- Arbitrage reversal или overtake.

Это не взаимозаменяемые вещи. Страница, simulator test или governance argument, где сказано `compression` без контекста метрики, неполны.

## Связанные страницы

- [Стандарт TMCTOL](tmctol-standard.ru.md)
- [Формулы TMCTOL](../math/tmctol-formulas.ru.md)
- [Token Minting Curve](../overview/token-minting-curve.ru.md)
- [Контур маршрутизации и минтинга](routing-and-minting-loop.ru.md)
- [Токеновые поверхности](token-surfaces.ru.md)
