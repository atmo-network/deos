---
page_type: concept
title: Пулы стейкинга
summary: Стейкинг DEOS использует multi-asset share-vault модель с передаваемыми receipt-токенами `stXXX`. Нативный `$NTVE` теперь минтит ликвидный `stNTVE`, а безопасность коллаторов и native nomination rewards идут через явно заблокированный `NTVE/stNTVE` LP, а не через live-привязку баланса `stNTVE`.
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
last_compiled: 2026-04-25
confidence: 0.94
---

# Пулы стейкинга

## Кратко

Стейкинг DEOS — это multi-asset share-vault система. У каждого зарегистрированного staking-актива есть пул, детерминированные аккаунты и учет долей/receipt-токенов, чтобы backing мог расти без записи наград каждому holder-у.

Текущая native staking линия специально разделена на две поверхности: `$NTVE -> stNTVE` — это ликвидный share-vault staking, а collator nomination и native reward exposure идут через заблокированный `NTVE/stNTVE` LP. Обычный баланс `stNTVE` не является сигналом безопасности коллатора.

## Модель share-vault

Для каждого staking-актива система хранит:

- Один детерминированный аккаунт пула
- Один объект состояния пула
- Transferable receipt supply, если существует актив `stXXX`
- Ограниченные read-поверхности для exchange rate, account value и reward claimability

Право собственности выражается долями. Приток средств в пул повышает стоимость каждой доли вместо fan-out записи по всем пользовательским аккаунтам.

## Receipt-токены `stXXX`

`stXXX` — это yield-bearing receipts для staking-пулов:

- Локальные и native receipts используют namespace `TYPE_STAKED`
- Foreign staking receipts используют `TYPE_STAKED_FOREIGN`
- Supply receipt-токена отслеживает выпущенные доли пула
- Стоимость доли растет, когда backing пула увеличивается, а receipt supply остается прежним

Для native staking конкретный receipt — это `stNTVE`.

## Native `$NTVE -> stNTVE`

Нативный вход теперь ликвидный и не требует выбора оператора:

```text
$NTVE
  -> Staking::stake_native(amount)
  -> mint stNTVE receipt shares
```

Это vault deposit и receipt mint, а не обычный AMM swap. Он увеличивает backing native staking pool и минтит receipt-доли по accounting-правилам staking-пула.

## Безопасность коллаторов идет через locked LP

Native collator backing больше не выводится из live-балансов `stNTVE` или transfer-driven native bindings. Текущий security path — это явная LP custody:

```text
$NTVE + stNTVE
  -> add liquidity to NTVE/stNTVE
  -> receive NTVE/stNTVE LP
  -> lock_native_lp_for_collator(lp_asset_id, amount, operator)
```

Заблокированный `NTVE/stNTVE` LP оценивается консервативно через runtime native-equivalent read model и питает ranking коллаторов / native nomination reward exposure.

## Governance custody

Та же native-value поверхность может блокироваться только для governance `NativeVotePower`, без nomination коллатора. В текущем runtime есть отдельные LP и native-asset custody paths для tactical protection voting, а unlock requests блокируются, пока активны governance lock horizons.

## Native nomination rewards

Native nomination rewards рассчитываются через native-specific claim paths. Generic same-asset reward settlement отвергает native staking asset, чтобы `$NTVE` nomination rewards не уходили через legacy auto-compound семантику.

Нативные settlement paths включают:

- `claim_nomination_reward(epoch)` для ликвидной выплаты `$NTVE`
- `claim_and_compound_nomination_reward(epoch, operator)` для превращения выплаты в locked LP
- `claim_nomination_reward_batch(epochs)` для ограниченного multi-epoch native claiming

## Связь с governance-наградами

Staking и governance остаются отдельными подсистемами:

- Staking отвечает за математику пула, receipts, locked LP custody, reward snapshots и settlement
- Governance отвечает за bounded participation memory, vote-power policy, execution state и exported reward coefficients

Для ненативных активов same-asset reward settlement по-прежнему может auto-compound в свежие receipts. Native `$NTVE` nomination rewards используют выделенные native paths выше.

## Связанные страницы

- [Домены Governance](governance-domains.ru.md)
- [Контур маршрутизации и минтинга](routing-and-minting-loop.ru.md)
- [Базовые термины](../glossary/core-terms.ru.md)
- [FAQ для новичков](../faq/newcomer-faq.ru.md)

## Источники

- `docs/staking.specification.en.md`
- `docs/staking.architecture.en.md`
- `docs/governance.specification.en.md`
