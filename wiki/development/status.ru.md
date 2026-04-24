---
page_type: status
title: Статус разработки
summary: Текущий статус реализации, контекст дорожной карты и главные активные направления работы во фреймворке DEOS. Текущая baseline-линия закрыла native staking/liquidity/governance migration и теперь смещена к завершению governance v1, полировке web-client и future-gated расширениям только при наличии policy или upstream необходимости.
locale: ru
canonical_page_id: status
translation_of: status.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../BACKLOG.md
  - ../../CHANGELOG.md
status: active
audience: newcomer
tags:
  - development
  - status
  - roadmap
related:
  - Трехуровневая валидация
  - Руководство контрибьютора
last_compiled: 2026-04-25
confidence: 0.96
---

# Статус разработки

## Кратко

DEOS находится в активной фазе стабилизации framework. Эталонная baseline-линия уже поставляет TMCTOL loop, AAA, multi-asset staking, bounded governance, runtime benchmark discipline, local/operator scripts и SvelteKit reference client.

Текущий акцент — кристаллизация: сделать shipped contract проще для эксплуатации и понимания, завершить узкую governance v1 rollout surface и держать future extensions gated реальным product pressure или upstream readiness.

## Что уже есть в базовой линии

Локальная сеть и reference client сейчас включают:

- **Core TMCTOL Loop**: однонаправленный minting, treasury-owned liquidity routing, fee burning и bounded economic invariants
- **AAA substrate**: детерминированные actors для burning, liquidity provisioning, bucket/treasury logic и native-staking LP donation, с portable generic staking tasks
- **Staking**: multi-asset share-vault staking с `stXXX` receipts, native `$NTVE -> stNTVE` liquid staking, locked `NTVE/stNTVE` LP nomination, native governance custody и native nomination reward settlement
- **Governance foundation**: domain-scoped primary/protection tracks, public cadence, typed payload kinds, invoice voting, runtime-upgrade authorization, bounded execution details и governance reward-memory
- **Web Client**: on-chain-first wallet, swap, staking, governance, wiki и execution-feedback surfaces на SvelteKit
- **Operator tooling**: runtime metadata export, benchmark/weight generation, local bootstrap, authorized-upgrade helpers и native staking bootstrap readiness/call-preparation helpers

## Недавно стабилизировано

Native staking/liquidity/governance migration baseline закрыта в текущей линии. Shipped contract теперь трактует `stNTVE` как liquid receipt, переносит collator security на locked `NTVE/stNTVE` LP, держит native reward settlement на native-specific paths и включает plan-only operator tooling для bootstrap canonical pool.

AAA staking также сужен обратно до portable contract: в AAA остаются generic `Stake { asset, amount }` и `Unstake { asset, shares }`, а DEOS-native staking routing и nomination policy живут в runtime adapters плюс staking/governance pallet-ах.

## Активный фокус: Governance v1

Ближайший roadmap сосредоточен на завершении самой маленькой честной **Governance v1** rollout surface:

- Держать execution authority привязанной к явным domains, cadence modes и payload kinds
- Добавлять только действительно delegated/domain-owned `L2ParameterChange` surfaces сверх текущей пары Axial Router
- Улучшать execution-side observability только когда появляются новые payload families или failure states
- Продолжать web-client governance UX только там, где proposal semantics или execution state требуют более ясной product composition

## Future-gated работа

Часть работы намеренно не входит в текущую launch baseline:

- **LP Donation Acquisition**: Native Staking LP Farmer уже поддерживает deterministic `$NTVE` acquisition в balanced `NTVE/stNTVE` donation. Swap/mixed-route acquisition остается future-gated, пока AAA policy не потребует route comparison, slippage bounds и fallback behavior.
- **Randomness / Relay Beacon**: Permissionless collators и advanced randomness отложены до появления настоящего parachain-consumable per-block protocol beacon upstream. Текущая линия использует trusted invulnerable collators и deterministic previous-block-hash fallback там, где это нужно.
- **Materialized Archive UX**: Browser не должен растягивать bounded on-chain retention в archive/search features. Это задача будущего materialized provider contract.

## Где смотреть актуальный план

За живым состоянием проекта лучше всего следить через root-файлы [`BACKLOG.md`](../../BACKLOG.md) и [`CHANGELOG.md`](../../CHANGELOG.md).

## Связанные страницы

- [Трехуровневая валидация](three-layer-validation.ru.md)
- [Руководство контрибьютора](../community/contributing.ru.md)

## Источники

- `BACKLOG.md`
- `CHANGELOG.md`
