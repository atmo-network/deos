---
page_type: getting-started
title: Executive Summary
summary: Одностраничное внешнее резюме о том, что такое DEOS, почему это важно, чем он отличается от discretionary DAO treasury management, почему Polkadot/Substrate подходит, что уже поставлено, что не поставлено и как начинается принятие.
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
  - ../../docs/manifesto.en.md
  - ../../docs/governance.specification.en.md
  - ../../docs/tmctol.specification.en.md
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
  - Форк DEOS
  - Physics vs Politics
  - Чем DEOS не является
  - Уровни экономических утверждений
last_compiled: 2026-06-13
confidence: 0.86
---

# Executive Summary

## Что это

DEOS — форкаемый runtime framework для программируемых токеновых экономик. Основная идея проста: заменить discretionary treasury operations детерминированными экономическими контурами внутри протокола.

TMCTOL — первый экономический стандарт на DEOS. Он соединяет mint-only token curve, treasury-owned liquidity, fee burn, bucketed policy, staking, routing, bounded governance и automated actors.

## Почему это важно

Многие токеновые экономики полагаются на будущий комитет, который должен правильно управлять ликвидностью, treasury funds, выпуском, стимулами и upgrades. DEOS сужает эту поверхность доверия, перенося повторяющееся экономическое поведение в явные runtime-механизмы.

Результат — не обещание цены. Это более ясный контракт: эта часть управляется протоколом, эта часть управляется governance, эта часть индексируется или материализуется, а эта часть остается продуктовым и рыночным риском.

## DAO treasury vs deterministic circuits

Обычная DAO treasury часто сначала является политической поверхностью управления: voters, delegates, multisigs, committees или off-chain operators решают, когда тратить средства, делать buyback, поддерживать ликвидность, платить contributors или менять стимулы.

DEOS рассматривает базовый treasury loop сначала как экономическую инфраструктуру. Governance остается нужна для launch parameters, domain ownership, protected upgrades, границ treasury policy и emergency choices, но она не должна каждую неделю вручную воспроизводить базовый экономический цикл.

Практический контраст простой:

- DAO treasury default: “DAO будет ответственно управлять средствами”.
- DEOS default: “этот mechanism исполняется при таких явных условиях”.

DEOS не против governance. Он против mystery-governance.

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

Партнерская команда начинает с внешних entry pages, смотрит fork profile внутри [Форка DEOS](../usage/forking-deos.ru.md), проверяет, подходит ли TMCTOL ее экосистеме, и определяет downstream product-specific dApps и user-facing philosophy.

## Следующие страницы

- [DEOS за 60 секунд](deos-in-60-seconds.ru.md)
- [Partner Pitch](partner-pitch.ru.md)
- [Форк DEOS](../usage/forking-deos.ru.md)
- [Physics vs Politics](../comparisons/physics-vs-politics.ru.md)
- [Чем DEOS не является](../concepts/what-deos-is-not.ru.md)
- [Уровни экономических утверждений](../concepts/economic-claim-levels.ru.md)
