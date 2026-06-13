---
page_type: concept
title: Сценарии TOL buckets
summary: Конкретные сценарии для TMCTOL treasury-owned-liquidity buckets, включая bucket A, B, C, D, пути unwind и actor wakeups.
locale: ru
canonical_page_id: tol-bucket-scenarios
translation_of: tol-bucket-scenarios.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../docs/tmctol.specification.en.md
  - ../../docs/aaa.architecture.en.md
  - ../../docs/aaa.specification.en.md
  - ../../AGENTS.md
status: active
audience: newcomer
tags:
  - concept
  - tmctol
  - liquidity
  - buckets
  - aaa
related:
  - Стандарт TMCTOL
  - Сквозные сценарии
  - Architecture diagrams
  - AAA system
  - Token-driven automation
last_compiled: 2026-06-13
confidence: 0.86
---

# Сценарии TOL buckets

## Кратко

TMCTOL использует treasury-owned-liquidity buckets, чтобы капитал ликвидности работал продуктивно, но разные экономические намерения не смешивались. Важно не только то, что value попадает в buckets, а какой downstream lane просыпается, когда bucket становится actionable.

Базовая модель — четыре buckets. Bucket A усиливает immediate market liquidity, а buckets B/C/D сохраняют отдельные treasury- или governance-conditioned lanes.

## Bucket A: immediate liquidity

Bucket A — прямой liquidity lane. Когда minting или routing flows создают protocol-owned liquidity, bucket A ближе всего к immediate market depth и эффекту Gravity Well.

Сценарий:

```text
User demand -> route/mint -> reserves grow
  -> Bucket A receives liquidity share
  -> Market depth improves
  -> Future swaps see stronger protocol-owned liquidity
```

Bucket A проще всего понять: он напрямую усиливает market surface.

## Bucket B: segmented treasury lane

Bucket B хранит отдельное treasury-намерение. Он может накапливать liquidity или unwound value, не смешивая ее со всеми downstream purposes.

Сценарий:

```text
Protocol-owned LP matures or unwinds
  -> Bucket B lane receives value
  -> Paired treasury actor/account accumulates it
  -> Governance can reason about that lane separately
```

Разделение важно: governance не должна видеть все liquidity reserves как один безликий pot.

## Buckets C и D: wakeup scenarios

Buckets C и D легко недооценить, потому что их смысл — отложенное и раздельное действие. Они важны, когда autonomous actor или treasury lane просыпается, потому что в bucket накопилось достаточно value для bounded operation.

Пример C wakeup:

```text
Fees / unwound LP / routed value accumulate
  -> Bucket C crosses actor-specific threshold
  -> System AAA actor wakes
  -> Actor executes bounded plan
  -> Output lands in its paired treasury or liquidity lane
```

Пример D wakeup:

```text
Longer-tail accumulation continues
  -> Bucket D remains idle until actionable
  -> Wakeup condition becomes true
  -> Actor attempts execution
  -> Retry/cooldown handles unavailable markets or oracle gaps
```

Поэтому C/D lanes задают терпение и сегментацию. Protocol не обязан действовать немедленно, но накопленная value все равно может перейти в исполнимый flow, когда условия выполнены.

## Зачем paired treasuries

У каждого non-immediate bucket может быть dedicated paired treasury lane. Так provenance и governance intent остаются видимыми:

```text
Bucket B -> Treasury B lane
Bucket C -> Treasury C lane
Bucket D -> Treasury D lane
```

Downstream fork может менять policies, но должен сохранять идею: bucket provenance — часть economic contract, а не accounting decoration. Если bucket policy меняет wakeup thresholds, treasury lanes или actor plans, сначала проверяйте изменение against TMCTOL math, а затем against AAA execution behavior.

## Связанные страницы

- [Стандарт TMCTOL](tmctol-standard.ru.md)
- [Сквозные сценарии](end-to-end-flows.ru.md)
- [Architecture diagrams](architecture-diagrams.ru.md)
- [AAA system](../overview/aaa-system.ru.md)
- [Token-driven automation](token-driven-automation.ru.md)
