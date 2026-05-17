---
page_type: getting-started
title: DEOS за 60 секунд
summary: Короткая внешняя точка входа, которая объясняет главный мем DEOS, границу продукта, стандарт TMCTOL и ценность фреймворка до входа в полный архитектурный граф.
locale: ru
canonical_page_id: deos-in-60-seconds
translation_of: deos-in-60-seconds.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../README.md
  - ../../docs/core.architecture.en.md
  - ../../docs/tmctol.specification.en.md
status: active
audience: newcomer
tags:
  - onboarding
  - positioning
  - overview
related:
  - Обзор фреймворка DEOS
  - Чем DEOS не является
  - Стандарт TMCTOL
  - Карта доменов
last_compiled: 2026-05-17
confidence: 0.88
---

# DEOS за 60 секунд

## Самая короткая версия

DEOS — это форкаемый runtime-фреймворк для программируемых экономик. Он соединяет выпуск токена, protocol-owned liquidity, маршрутизацию, staking, governance и автоматизированных actors в детерминированную институциональную машину.

TMCTOL — первый экономический стандарт поверх DEOS: mint-only curve плюс treasury-owned liquidity, сжигание комиссий, bucketed policy и ограниченный governance-контроль.

## Мем

DEOS заменяет ручное DAO-управление казначейством детерминированными экономическими контурами.

Вместо того чтобы просить комитет импровизировать каждое treasury-действие, DEOS кодирует переиспользуемые экономические потоки:

- Спрос может минтить через curve;
- Эмиссия может строить protocol-owned liquidity;
- Маршрутизация может выбирать рыночную или протокольную ликвидность;
- Комиссии могут питать burn, staking или treasury lanes;
- Actors могут просыпаться от балансов и исполнять ограниченные планы;
- Governance может настраивать явные домены, не притворяясь владельцем всего.

## Почему это важно

Большинство токеновых проектов разносит экономику, governance, ликвидность, автоматизацию и клиентскую правду по слабым социальным обещаниям. DEOS пытается сделать эти связи явными внутри runtime и окружающего validation stack.

Это не делает рынки предсказуемыми. Это делает protocol reactions читаемыми, ограниченными и пригодными для форка.

## Что читать дальше

- [Обзор фреймворка DEOS](../overview/deos-framework.ru.md)
- [Чем DEOS не является](../concepts/what-deos-is-not.ru.md)
- [Стандарт TMCTOL](../concepts/tmctol-standard.ru.md)
- [Карта доменов](../concepts/domain-map.ru.md)
