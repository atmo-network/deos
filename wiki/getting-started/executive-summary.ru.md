---
page_type: getting-started
title: Executive Summary
summary: Одностраничное внешнее резюме о том, что такое DEOS, почему это важно, почему Polkadot/Substrate подходит, что уже поставлено, что не поставлено и как начинается принятие.
locale: ru
canonical_page_id: executive-summary
translation_of: executive-summary.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../README.md
  - ../../AGENTS.md
  - ../../BACKLOG.md
  - ../../CHANGELOG.md
status: active
audience: partner
tags:
  - onboarding
  - positioning
  - executive
  - adoption
related:
  - DEOS за 60 секунд
  - Partner Pitch
  - DEOS vs DAO Treasury
  - Минимальный профиль форка
last_compiled: 2026-05-17
confidence: 0.84
---

# Executive Summary

## Что это

DEOS — форкаемый runtime framework для программируемых токеновых экономик. Основная идея проста: заменить discretionary treasury operations детерминированными экономическими контурами внутри протокола.

TMCTOL — первый экономический стандарт на DEOS. Он соединяет mint-only token curve, treasury-owned liquidity, fee burn, bucketed policy, staking, routing, bounded governance и automated actors.

## Почему это важно

Многие токеновые экономики полагаются на будущий комитет, который должен правильно управлять ликвидностью, treasury funds, выпуском, стимулами и upgrades. DEOS сужает эту поверхность доверия, перенося повторяющееся экономическое поведение в явные runtime-механизмы.

Результат — не обещание цены. Это более ясный контракт: эта часть управляется протоколом, эта часть управляется governance, эта часть индексируется или материализуется, а эта часть остается продуктовым и рыночным риском.

## Почему Polkadot/Substrate

DEOS нужна runtime-first среда, где economic rules, assets, automation, governance и XCM-facing asset identity можно выразить как первичную protocol logic. Substrate и Polkadot дают такую поверхность runtime construction, не превращая DEOS в обычное general-purpose smart-contract app.

## Что уже поставлено

- Rust runtime workspace с DEOS pallets, primitives и runtime configuration.
- Reference mechanics и specifications для TMCTOL.
- AAA actor automation model.
- SvelteKit reference client с domain slices и wiki rendering.
- Operator scripts, validation gates и generated wiki metadata.
- Release tag `0.5.0` для текущей линии framework.

## Что не поставлено

- Готовый consumer ecosystem product.
- Permissionless collator onboarding как текущий выбор по умолчанию.
- Full portfolio UX за пределами доступных сейчас chain/read-model surfaces.
- Гарантия рыночного спроса, price appreciation или risk-free treasury behavior.

## Путь принятия

Партнерская команда начинает с внешних entry pages, выбирает minimal fork profile, проверяет, подходит ли TMCTOL ее экосистеме, и определяет downstream product-specific dApps и user-facing philosophy.

## Следующие страницы

- [DEOS за 60 секунд](deos-in-60-seconds.ru.md)
- [Partner Pitch](partner-pitch.ru.md)
- [DEOS vs DAO Treasury](../comparisons/deos-vs-dao-treasury.ru.md)
- [Минимальный профиль форка](../usage/minimal-fork-profile.ru.md)
