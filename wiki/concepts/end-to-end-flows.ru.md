---
page_type: concept
title: Сквозные сценарии
summary: Конкретные walkthroughs, которые связывают пользовательские действия, runtime routing, пробуждение AAA акторов, buckets, read-model surfaces и выбор validation внутри DEOS.
locale: ru
canonical_page_id: end-to-end-flows
translation_of: end-to-end-flows.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../docs/axial-router.architecture.en.md
  - ../../docs/aaa.architecture.en.md
  - ../../docs/tmctol.specification.en.md
  - ../../docs/staking.architecture.en.md
status: active
audience: newcomer
tags:
  - concept
  - flows
  - routing
  - aaa
  - onboarding
related:
  - Карта доменов
  - Контур маршрутизации и минтинга
  - Система AAA
  - AA-Актор
  - Стандарт TMCTOL
  - Пулы стейкинга
  - Разделение read-model
last_compiled: 2026-07-20
confidence: 0.85
---

# Сквозные сценарии

## Кратко

Эта страница закрывает промежуток между концептуальными страницами и файлами реализации. Она показывает, как пользовательское действие или протокольное событие проходит через DEOS как конкретный flow.

Примеры упрощены, но каждый шаг называет ответственный домен и страницу-владельца для деталей.

## Swap через Axial Router

1. Пользователь запрашивает preview swap в эталонном клиенте.
2. Клиент читает ограниченные route/asset данные и помечает результат как живую on-chain истину, а не archive analytics.
3. Axial Router сравнивает пути через market liquidity и protocol liquidity.
4. Если выгоднее TMC path, маршрут идет через curve minting. Если выгоднее XYK path, он идет через market liquidity.
5. Комиссии маршрутизатора поступают в настроенный контур Burn Actor; сжигание происходит только после того, как обеспеченный средствами и допущенный к расписанию актор завершит исполнение.
6. Клиент показывает execution progress через общий feedback, а не через отдельный transaction log в каждом widget.

Страницы-владельцы: [Контур маршрутизации и минтинга](routing-and-minting-loop.ru.md), [Axial Router](../overview/axial-router.ru.md), [Разделение read-model](read-model-split.ru.md), [Эталонный клиент](../overview/reference-client.ru.md).

## Цепочка пробуждения акторов

1. Для System Actor наступает настроенное условие запуска: входящий баланс у всеядного актора или ограниченное расписание у актора, работающего по таймеру.
2. Планировщик AAA допускает актор, только если это разрешают правила жизненного цикла, паузы, комиссии и ограниченного исполнения.
3. Актор выполняет типизированный план: свап, сжигание, добавление или снятие ликвидности, разделенную передачу, стейкинг или анстейкинг.
4. Его результат может поступить на счет другого актора, запускаемого входящим балансом, и пробудить его.
5. Более крупное поведение протокола складывается из малых ограниченных шагов, но остается читаемым как граф акторов.

Страницы-владельцы: [Система AAA](../overview/aaa-system.ru.md), [AA-Актор](../overview/aa-actor.ru.md), [Токен-управляемая автоматизация](token-driven-automation.ru.md).

## TOL bucket и treasury lane

1. Mint-side reserve flow увеличивает protocol-owned liquidity.
2. TOL bucket policy сегментирует эту liquidity, чтобы не все reserves выполняли одну и ту же работу.
3. Bucket или LP-unwind actor может проснуться, когда появляется нужный balance или приходит нужный schedule.
4. Unwound assets идут в paired treasury lanes, а не смешиваются в один общий account.
5. Governance может рассуждать о segmented treasury surfaces, не владея launch physics кривой.

Это намеренно domain-level walkthrough. Bucket ratios и formulas принадлежат [Стандарту TMCTOL](tmctol-standard.ru.md) и [Формулам TMCTOL](../math/tmctol-formulas.ru.md).

## Native staking и безопасность коллаторов

1. Пользователь стейкает `$NTVE` и получает liquid `stNTVE` receipt shares.
2. Collator security не выводится из wallet `stNTVE` balances.
3. Security path использует явную locked `NTVE/stNTVE` LP custody.
4. Native nomination reward paths отделены от generic same-asset staking rewards.
5. Governance-conditioned participation может влиять на reward coefficients, но governance и staking остаются разными подсистемами.

Страница-владелец: [Пулы стейкинга](staking-pools.ru.md).

## Правило проверки

Когда меняется flow, проверяйте самый высокий затронутый слой:

- Изменилась formula или invariant -> сначала simulator;
- Изменилось pallet behavior -> targeted Rust tests/benchmarks;
- Изменилась runtime interaction -> integration checks;
- Изменилась клиентская surface -> web-client validation и честность read-model;
- Изменилась wiki explanation -> trusted wiki validation.

Смотрите [Трехуровневую валидацию](../development/three-layer-validation.ru.md).

## Связанные страницы

- [Карта доменов](domain-map.ru.md)
- [Контур маршрутизации и минтинга](routing-and-minting-loop.ru.md)
- [Система AAA](../overview/aaa-system.ru.md)
- [AA-Актор](../overview/aa-actor.ru.md)
- [Стандарт TMCTOL](tmctol-standard.ru.md)
- [Пулы стейкинга](staking-pools.ru.md)
- [Разделение read-model](read-model-split.ru.md)
