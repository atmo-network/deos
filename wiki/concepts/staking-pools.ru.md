---
page_type: concept
title: Пулы стейкинга
summary: Стейкинг DEOS использует пулы долей с переносимыми квитанциями `stXXX`. На этапе Phase 1 действует ликвидный учет `$NTVE -> stNTVE`, но пользовательские LP-номинации и доступные к получению награды остаются отключенными до явного обновления runtime для Phase 2.
locale: ru
canonical_page_id: staking-pools
translation_of: staking-pools.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../docs/staking.specification.en.md
  - ../../docs/staking.architecture.en.md
  - ../../docs/governance.specification.en.md
status: active
audience: newcomer
tags:
  - concept
  - staking
  - receipts
related:
  - Домены Governance
  - Контур маршрутизации и минтинга
  - Базовые термины
  - FAQ для новичков
last_compiled: 2026-07-20
confidence: 0.85
---

# Пулы стейкинга

## Кратко

Стейкинг DEOS — это multi-asset share-vault система. У каждого зарегистрированного staking-актива есть пул, детерминированные аккаунты и учет долей/receipt-токенов. Такая модель позволяет backing расти без записи наград каждому держателю отдельно.

Контракт нативного стейкинга отделяет ликвидный учет долей `$NTVE -> stNTVE` от номинации коллаторов. На этапе Phase 1 работают доверенные permissioned-коллаторы, а пользовательские LP-номинации и доступные к получению награды отключены. В Phase 2 может применяться заблокированный `NTVE/stNTVE` LP; обычный баланс `stNTVE` никогда не служит сигналом безопасности коллатора.

## Модель share-vault

Для каждого staking-актива система хранит:

- Один детерминированный аккаунт пула;
- Один объект состояния пула;
- Transferable receipt supply, если существует актив `stXXX`;
- Ограниченные read-поверхности для exchange rate, account value и reward claimability.

Право собственности выражается долями. Приток средств в пул повышает стоимость каждой доли вместо веерной записи по всем пользовательским аккаунтам.

## Receipt-токены `stXXX`

`stXXX` — это yield-bearing receipts для staking-пулов:

- Local и native receipts используют namespace `TYPE_STAKED`;
- Foreign staking receipts используют `TYPE_STAKED_FOREIGN`;
- Supply receipt-токена отслеживает выпущенные доли пула;
- Стоимость доли растет, когда backing пула увеличивается, а receipt supply остается прежним.

Для native staking конкретный receipt — `stNTVE`.

## Native `$NTVE -> stNTVE`

Нативный вход теперь ликвидный и не требует выбора оператора:

```text
$NTVE
  -> Staking::stake_native(amount)
  -> mint stNTVE receipt shares
```

Это vault deposit и receipt mint, а не обычный AMM swap. Он увеличивает backing native staking pool и минтит receipt-доли по учетным правилам staking-пула.

## Граница этапов для безопасности коллаторов

На этапе Phase 1 используются доверенные permissioned-коллаторы. Пользовательская экономика номинаций и доступные к получению награды на этом этапе не действуют.

Явный контракт Phase 2 использует хранение LP вместо текущих балансов `stNTVE` или привязок, возникающих при передаче:

```text
$NTVE + stNTVE
  -> add liquidity to NTVE/stNTVE
  -> receive NTVE/stNTVE LP
  -> lock_native_lp_for_collator(lp_asset_id, amount, operator)
```

Runtime содержит ограниченные поверхности хранения и оценки заблокированного `NTVE/stNTVE` LP, но стартовый контракт держит номинации и поток их наград отключенными до явного обновления runtime для Phase 2.

## Governance custody

Та же native-value поверхность может блокироваться только для governance `NativeVotePower`, без nomination коллатора. В текущем runtime есть отдельные LP и native-asset custody paths для tactical protection voting, а unlock requests блокируются, пока активны governance lock horizons.

## Награды за нативную номинацию в Phase 2

Спецификация резервирует отдельные пути получения нативных наград для Phase 2. Общий расчет награды в том же активе отвергает нативный staking asset, чтобы награды `$NTVE` не уходили через устаревшую семантику автоматического реинвестирования.

Реализованная поверхность расчета включает:

- `claim_nomination_reward(epoch)` для ликвидной выплаты `$NTVE`;
- `claim_and_compound_nomination_reward(epoch, operator)` для превращения выплаты в locked LP;
- `claim_nomination_reward_batch(epochs)` для ограниченного multi-epoch native claiming.

## Связь с governance-наградами

Staking и governance остаются отдельными подсистемами:

- Staking отвечает за математику пула, receipts, locked LP custody, reward snapshots и settlement;
- Governance отвечает за bounded participation memory, vote-power policy, execution state и exported reward coefficients.

Для ненативных активов расчет награды в том же активе по-прежнему может автоматически выпускать новые квитанции. Награды `$NTVE` за номинацию остаются отдельным, привязанным к этапу потоком и не действуют в стартовой линии Phase 1 с доверенными коллаторами.

## Связанные страницы

- [Домены Governance](governance-domains.ru.md)
- [Контур маршрутизации и минтинга](routing-and-minting-loop.ru.md)
- [Базовые термины](../glossary/core-terms.ru.md)
- [FAQ для новичков](../faq/newcomer-faq.ru.md)
