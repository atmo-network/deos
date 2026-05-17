---
page_type: usage
title: Маршрут оценки для партнера
summary: Короткий маршрут из пяти страниц для партнерских команд, которые оценивают, стоит ли форкать DEOS и строить downstream-экосистему поверх него.
locale: ru
canonical_page_id: partner-evaluation-route
translation_of: partner-evaluation-route.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../README.md
  - ../../AGENTS.md
  - ../../BACKLOG.md
  - ../../wiki/getting-started/executive-summary.en.md
status: active
audience: partner
tags:
  - onboarding
  - partners
  - adoption
  - fork
related:
  - Executive Summary
  - Partner Pitch
  - Минимальный профиль форка
  - Форк DEOS
last_compiled: 2026-05-17
confidence: 0.84
---

# Маршрут оценки для партнера

## Назначение

Этот маршрут для команды, которая спрашивает: “Стоит ли нам форкать DEOS для собственной экосистемы?”

Первый проход должен быть коротким. Не начинайте со всех pallets, runtime details или economic thresholds. Начните с вопроса принятия, а в framework graph переходите только если совпадение реально.

## Маршрут из пяти страниц

1. [DEOS за 60 секунд](../getting-started/deos-in-60-seconds.ru.md) — понять главный meme.
2. [Executive Summary](../getting-started/executive-summary.ru.md) — увидеть, что поставлено, что не поставлено и почему важны Polkadot/Substrate.
3. [Partner Pitch](../getting-started/partner-pitch.ru.md) — понять ценность для партнера и контур 30/90 дней.
4. [DEOS vs DAO Treasury](../comparisons/deos-vs-dao-treasury.ru.md) — сравнить комитетское управление treasury и deterministic circuits.
5. [Минимальный профиль форка](minimal-fork-profile.ru.md) — проверить минимальную credible fork shape.

## Контрольная точка

После этих пяти страниц партнерская команда должна ответить:

- Наша целевая система действительно protocol economy, а не просто приложение?
- Нам нужны runtime-level economic circuits, или обычного smart-contract stack достаточно?
- TMCTOL подходит нашей launch thesis, или нужен другой standard поверх DEOS?
- Какие данные должны быть on-chain, а какие может материализовать indexer?
- Какой user-facing product, dApps и комьюнити-нарратив мы строим downstream сами?

## Если ответ да

Продолжайте с [Форк DEOS](forking-deos.ru.md), [Карта доменов](../concepts/domain-map.ru.md) и [Трехслойная валидация](../development/three-layer-validation.ru.md).
