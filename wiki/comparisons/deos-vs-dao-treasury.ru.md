---
page_type: comparison
title: DEOS vs DAO Treasury
summary: Короткое сравнение discretionary DAO treasury management и DEOS-подхода с детерминированными экономическими контурами.
locale: ru
canonical_page_id: deos-vs-dao-treasury
translation_of: deos-vs-dao-treasury.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../README.md
  - ../../docs/manifesto.en.md
  - ../../docs/governance.specification.en.md
  - ../../docs/tmctol.specification.en.md
status: active
audience: partner
tags:
  - positioning
  - governance
  - treasury
  - comparison
related:
  - Partner Pitch
  - Physics vs Politics
  - Чем DEOS не является
  - Уровни экономических утверждений
last_compiled: 2026-05-17
confidence: 0.84
---

# DEOS vs DAO Treasury

## Контраст

Обычная DAO treasury часто сначала является политической поверхностью управления: voters или delegates решают, когда тратить средства, делать buyback, поддерживать ликвидность, платить contributors или менять стимулы.

DEOS рассматривает базовый treasury loop сначала как экономическую инфраструктуру. Governance остается, но поведение по умолчанию зашито в детерминированные контуры: minting, protocol-owned liquidity, routing, fee burn, bucket policy, staking и automated actors.

## Committee treasury vs circuit treasury

- **Кто действует?**
  - DAO treasury по умолчанию: multisigs, delegates, committees, внешние operators.
  - DEOS по умолчанию: runtime pallets и AAA actors.
- **Когда действует?**
  - DAO treasury по умолчанию: после proposals, meetings, scripts или ручного execution.
  - DEOS по умолчанию: через ограниченные protocol triggers, schedules и движение токенов.
- **Какое утверждение?**
  - DAO treasury по умолчанию: “DAO будет ответственно управлять средствами”.
  - DEOS по умолчанию: “этот mechanism исполняется при таких явных условиях”.
- **Что может сломаться?**
  - DAO treasury по умолчанию: politics, coordination, discretion, слабое execution.
  - DEOS по умолчанию: mechanism design, parameter choice, ограниченные runtime surfaces.
- **Что должна делать governance?**
  - DAO treasury по умолчанию: решать много обычных operations.
  - DEOS по умолчанию: контролировать ограниченные policy surfaces и исключительные изменения.

## Почему DEOS всё равно нужна governance

DEOS не против governance. Он против mystery-governance.

Governance остается нужна для launch parameters, domain ownership, protected upgrades, границ treasury policy и emergency choices. Разница в том, что governance не должна каждую неделю вручную воспроизводить базовый экономический цикл.

## Честная суть

DEOS не делает экономику безрисковой. Он делает protocol-managed часть экономики менее discretionary.

Это важно, потому что внешний читатель может проверить mechanism, протестировать его, форкнуть и отказаться от него. Настроение будущего комитета нельзя проверить с такой же строгостью.

## Читать дальше

- [Partner Pitch](../getting-started/partner-pitch.ru.md)
- [Physics vs Politics](physics-vs-politics.ru.md)
- [Чем DEOS не является](../concepts/what-deos-is-not.ru.md)
- [Уровни экономических утверждений](../concepts/economic-claim-levels.ru.md)
