---
page_type: usage
title: Форк DEOS
summary: Практическая карта того, что downstream-команда меняет, сохраняет и проверяет при превращении DEOS в конкретную экосистему.
locale: ru
canonical_page_id: forking-deos
translation_of: forking-deos.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../README.md
  - ../../docs/README.md
  - ../../docs/aaa.embedding.en.md
  - ../../template/README.md
  - ../../web-client/README.md
  - ../../docs/tmctol.specification.en.md
status: active
audience: developer
tags:
  - usage
  - forkability
  - framework
  - downstream
related:
  - Обзор фреймворка DEOS
  - Структура репозитория
  - Технологический стек
  - Токеновые поверхности
  - Трехуровневая валидация
last_compiled: 2026-06-13
confidence: 0.86
---

# Форк DEOS

## Кратко

DEOS предназначен для форков командами, которые запускают конкретные экосистемы. Форк должен сохранять ясными reusable framework contracts и заменять то, что относится к конкретной экосистеме: продукт, токены, governance и операторскую политику.

Короткое правило: меняйте identity и policy; сохраняйте bounded mechanics и validation discipline.

## Что обычно меняется

Downstream fork обычно определяет:

- Chain identity, branding, endpoints, bootnodes и operator runbooks;
- Названия токенов, ticker presentation, launch allocation и продуктовый нарратив;
- Concrete governance domains, распределение protection-власти и bootstrap handoff plan;
- Продуктовые поверхности экосистемы, dApps, portfolio/indexer needs и materialized providers;
- Deployment parameters, collator/operator assumptions и monitoring setup;
- Client copy, default endpoints, wallet presets и user-facing flows.

Это продуктовые и экосистемные решения. Они не должны незаметно возвращаться в DEOS как hardcoded framework assumptions.

## Что должно оставаться стабильным

Форк должен сохранять базовую framework discipline, пока нет сильных причин менять ее:

- Deterministic protocol-managed economic reactions;
- Bounded runtime read surfaces versus materialized/indexed views;
- Явные AAA actor roles и execution-plan boundaries;
- Проверка математики TMCTOL до runtime changes;
- Разделение governance domains и protection;
- Staking share-vault и receipt accounting invariants;
- Zero-warning runtime/client hygiene и trust validation для wiki content.

Если fork меняет эти mechanics, это уже не только rebranding DEOS. Это изменение framework contract, которое нужно проверять на economic, runtime и integration layers.

## Минимальный профиль форка

DEOS fork не должен запускаться простой заменой названий. До launch-ready состояния downstream-команде нужны явные решения по экономическому, governance, runtime, клиентскому и операторскому профилю, который превращает framework в конкретную экосистему.

| Область | Минимальное решение форка |
| --- | --- |
| Native asset | Название, ticker, decimals, allocation, роль в staking/governance |
| Foreign collateral set | Какие assets можно register, route или использовать как collateral |
| TMC curve params | Initial price, slope, supply assumptions, launch immutability policy |
| TOL distribution | Bucket split, paired treasuries, reserve/lane semantics |
| Bucket policies | Какой bucket будит какого actor, threshold, retry и treasury lane |
| Router fee | Границы fee, burn/sink routing, governance mutability |
| Governance domain pairs | Primary/protection tokens, payload kinds, cadence, execution authority |
| Staking receipt policy | Receipt namespaces, native receipt, LP custody, reward paths |
| Materialized provider policy | Какие user flows требуют indexers или archive/search providers |
| Collator/randomness posture | Trusted phase, upgrade path, relay/protocol randomness dependency |
| Client/product surface | Default endpoints, wallet presets, copy, dApps, risk wording |
| Validation baseline | Simulator, runtime tests, client validation, wiki trust, operator smoke checks |

Если строка не решена, fork все еще prototype. Если строка решена, но не проверена, fork не готов к launch.

## Checklist форка

1. Переименуйте public identity, не переименовывая вслепую TMCTOL-specific standard concepts.
2. Решите, какие assets и governance surfaces являются ecosystem-specific.
3. Задайте launch parameters и считайте launch physics immutable, если только более сильный constitutional contract не говорит иначе.
4. Проверьте System AAA actor roles и уберите assumptions, подходящие только reference ecosystem.
5. Для переиспользования AAA реализуйте host-runtime adapters и держите business policy в adapters или genesis actor configuration, а не в core `pallet-aaa`.
6. Классифицируйте каждый client datum как direct on-chain projection или materialized/indexed view.
7. Обновите scripts, metadata export, endpoints и operator documentation.
8. Запускайте минимально достаточную validation, затем поднимайтесь выше, если пересекаются math/runtime/client boundaries.

## Что можно возвращать upstream

Хорошие upstream contributions — это framework-hardening changes: tests, честность client read-model, safer scripts, более ясные docs/wiki, лучшие adapter boundaries, benchmark fixes и bug fixes в reusable pallets.

Downstream-specific business logic, dApp behavior, токеновый нарратив и ecosystem policy обычно должны оставаться в fork.

## Связанные страницы

- [Обзор фреймворка DEOS](../overview/deos-framework.ru.md)
- [Чем DEOS не является](../concepts/what-deos-is-not.ru.md)
- [Структура репозитория](../implementation/repository-structure.ru.md)
- [Технологический стек](../implementation/tech-stack.ru.md)
- [Parachain context](../concepts/parachain-context.ru.md)
- [Токеновые поверхности](../concepts/token-surfaces.ru.md)
- [Трехуровневая валидация](../development/three-layer-validation.ru.md)
