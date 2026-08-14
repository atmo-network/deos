---
page_type: overview
title: Стейкинг
summary: Стейкинг DEOS использует пулы долей с переносимыми квитанциями `stXXX` и сессионным снимком LP-безопасности. На этапе Phase 1 LP-backed selection и расчет нативных наград отключены до явного обновления runtime.
locale: ru
canonical_page_id: staking
translation_of: staking.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../template/pallets/staking/docs/specification.en.md
  - ../../template/pallets/staking/docs/architecture.en.md
  - ../../template/pallets/governance/docs/specification.en.md
status: active
audience: newcomer
tags:
  - overview
  - staking
  - receipts
related:
  - Домены Governance
  - Контур маршрутизации и минтинга
  - Базовые термины
  - FAQ для новичков
last_compiled: 2026-08-13
confidence: 0.85
---

# Стейкинг

## Кратко

Стейкинг DEOS — это multi-asset share-vault система. У каждого зарегистрированного staking-актива есть один детерминированный аккаунт пула и учет долей/receipt-токенов. Такая модель позволяет backing расти без записи наград каждому держателю отдельно.

Контракт нативного стейкинга отделяет ликвидный учет долей `$NTVE -> stNTVE` от номинации коллаторов. На этапе Phase 1 работают доверенные permissioned-коллаторы, а пользовательские LP-номинации и доступные к получению награды отключены. В Phase 2 может применяться заблокированный `NTVE/stNTVE` LP; обычный баланс `stNTVE` никогда не служит сигналом безопасности коллатора.

## Модель share-vault

Для каждого staking-актива система хранит:

- Один детерминированный аккаунт пула;
- Один объект состояния пула;
- Transferable receipt supply, если существует актив `stXXX`;
- Ограниченные read-поверхности для exchange rate, account value, custody, режима безопасности, готовности и идентичности сессии.

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

Сейчас runtime атомарно фиксирует для одной сессии ограниченный снимок участников, допустимых операторов-кандидатов, консервативной стоимости LP, governance-коэффициентов, весов аккаунтов и общего знаменателя. Финансирование, обязательства, хранение истории, получение, истечение и реинвестирование наград недоступны, пока не реализован полный ограниченный контракт Phase 2.

Устаревший общий механизм наград по блокам, курсор перехода эпох, вывод финансирования из баланса reward-account, bootstrap-вызов и пути получения удалены.

## Связь с governance-наградами

Staking и governance остаются отдельными подсистемами:

- Staking отвечает за математику пула, receipts, locked LP custody и сессионные снимки безопасности;
- Governance отвечает за bounded participation memory, vote-power policy, execution state и exported reward coefficients.

Для ненативных активов доходность share-vault остается ростом стоимости квитанции после прямого притока обеспечения и `sync_pool`; она не создает reward pot, обязательство, право требования или зависимость от приема событий. Награды `$NTVE` за номинацию остаются отдельным, привязанным к этапу потоком и не действуют в стартовой линии Phase 1 с доверенными коллаторами.

## Связанные страницы

- [Домены Governance](../concepts/governance-domains.ru.md)
- [Контур маршрутизации и минтинга](../concepts/routing-and-minting-loop.ru.md)
- [Базовые термины](../glossary/core-terms.ru.md)
- [FAQ для новичков](../faq/newcomer-faq.ru.md)
