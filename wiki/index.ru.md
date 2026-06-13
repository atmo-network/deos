---
page_type: overview
title: Вики DEOS
summary: Самодостаточная карта знаний фреймворка DEOS и стандарта TMCTOL, организованная как плотный wiki-граф, а не список ссылок на docs.
locale: ru
canonical_page_id: index
translation_of: index.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../docs/README.md
  - ../README.md
status: active
audience: newcomer
tags:
  - overview
  - onboarding
  - deos
related:
  - Карта доменов
  - Обзор фреймворка DEOS
  - Первые шаги
  - Система AAA
  - Обзор Governance
  - Базовые термины
  - FAQ для новичков
last_compiled: 2026-05-17
confidence: 0.95
---

# Вики DEOS

## Кратко

DEOS — это форкаемый runtime-фреймворк для программируемых экономик: выпуск токена, protocol-owned liquidity, маршрутизация, staking, governance и автоматизированные actors становятся одной детерминированной институциональной машиной. TMCTOL — первый стандарт поверх него: mint-only curve плюс treasury-owned liquidity, сжигание комиссий, bucketed policy и ограниченный governance-контроль.

Мем: DEOS заменяет ручное DAO-управление казначейством детерминированными экономическими контурами.

Эта wiki — самодостаточный продукт знаний для понимания этого фреймворка. Она основана на проектной правде репозитория, но путь чтения должен оставаться внутри wiki: страницы объясняют понятия напрямую и ведут к другим wiki-страницам, а не требуют перехода к исходным документам.

## Начните здесь

- [Начните здесь](getting-started/start-here.ru.md) — выберите один путь: понять DEOS за 10 минут, поднять локально или безопасно форкнуть и изменить экономику
- [DEOS за 60 секунд](getting-started/deos-in-60-seconds.ru.md) — внешний крючок перед архитектурным графом
- [Первые шаги](getting-started/first-steps.ru.md) — более широкий маршрут новичка после onboarding-шлюза
- [Маршруты чтения](getting-started/reading-paths.ru.md) — маршруты по wiki для разных задач
- [Карта доменов](concepts/domain-map.ru.md) — главная карта доменов знаний
- [Базовые термины](glossary/core-terms.ru.md) — словарь для всего графа

## Быстрые маршруты оценки

- [Partner Pitch](getting-started/partner-pitch.ru.md) — внешняя страница о том, почему это важно для партнёрских команд
- [Executive Summary](getting-started/executive-summary.ru.md) — одностраничное резюме для читателей из экосистемы и инвесторов
- [Сквозные сценарии](concepts/end-to-end-flows.ru.md) — конкретные проходы через маршрутизацию, actors, корзины, staking и проверку
- [Архитектурные схемы](concepts/architecture-diagrams.ru.md) — компактные текстовые карты связей подсистем
- [Обзор фреймворка DEOS](overview/deos-framework.ru.md) — что такое фреймворк

## Доменные хабы

- [Стандарт TMCTOL](concepts/tmctol-standard.ru.md) — экономический стандарт и законы токена
- [Сценарии TOL buckets](concepts/tol-bucket-scenarios.ru.md) — конкретные пробуждения корзин A/B/C/D и каналы treasury
- [Токеновые поверхности](concepts/token-surfaces.ru.md) — роли Native, VETO, BLDR, расписок и LP
- [Система AAA](overview/aaa-system.ru.md) — автономные protocol actors
- [Axial Router](overview/axial-router.ru.md) — маршруты, комиссии и решения о протокольной ликвидности
- [Обзор Governance](overview/governance-overview.ru.md) — полномочия по доменам и защита
- [Пулы стейкинга](concepts/staking-pools.ru.md) — расписки staking, номинирование LP и вознаграждения
- [Эталонный клиент](overview/reference-client.ru.md) — on-chain-first браузерный продукт и wiki reader

## Экономика и runtime-понятия

- [Токен-управляемая автоматизация](concepts/token-driven-automation.ru.md)
- [Контур маршрутизации и минтинга](concepts/routing-and-minting-loop.ru.md)
- [Token Minting Curve](overview/token-minting-curve.ru.md)
- [Формулы TMCTOL](math/tmctol-formulas.ru.md)
- [Экономические пороги](concepts/economic-thresholds.ru.md)
- [Уровни экономических утверждений](concepts/economic-claim-levels.ru.md)
- [Карта инвариантов](concepts/invariant-map.ru.md)
- [Карта инвариантов и угроз](concepts/invariant-map.ru.md)
- [Чем DEOS не является](concepts/what-deos-is-not.ru.md)
- [Идентичность активов](overview/asset-identity.ru.md)
- [Паттерны runtime](overview/runtime-patterns.ru.md)
- [Контекст parachain](concepts/parachain-context.ru.md)
- [Стратегия случайности](overview/randomness-strategy.ru.md)

## Governance, модели чтения и форма клиента

- [Домены Governance](concepts/governance-domains.ru.md)
- [Executive Summary](getting-started/executive-summary.ru.md)
- [Physics-first против politics-first](comparisons/physics-vs-politics.ru.md)
- [Разделение read-model](concepts/read-model-split.ru.md)
- [UI Kit и Domain DAG](concepts/ui-kit-and-domain-dag.ru.md)
- [Generated Wiki](concepts/generated-wiki.ru.md)
- [Metadata wiki-графа](usage/wiki-graph-metadata.ru.md)

## Процессы и статус

- [Статус разработки](development/status.ru.md)
- [Трехуровневая валидация](development/three-layer-validation.ru.md)
- [Слой скриптов](usage/scripts-layer.ru.md)
- [Troubleshooting проверки](usage/validation-troubleshooting.ru.md)
- [Координация агентов](usage/agent-coordination.ru.md)
- [Форк DEOS](usage/forking-deos.ru.md)
- [Форк DEOS](usage/forking-deos.ru.md)
- [Структура репозитория](implementation/repository-structure.ru.md)
- [Технологический стек](implementation/tech-stack.ru.md)
- [Руководство контрибьютора](community/contributing.ru.md)
- [FAQ для новичков](faq/newcomer-faq.ru.md)

## Как читать эту wiki

- Сначала идите по wiki-ссылкам. Wiki должна быть понятна без выхода из wiki.
- Используйте [Карту доменов](concepts/domain-map.ru.md), когда страница кажется слишком локальной.
- Используйте [Базовые термины](glossary/core-terms.ru.md), когда словарь становится плотным.
- Используйте [Статус разработки](development/status.ru.md), чтобы отделять поставленную основу от будущей внешне-зависимой работы.
- Считайте метаданные страницы источниками, а не обязательным маршрутом чтения.

## Связанные страницы

- [Карта доменов](concepts/domain-map.ru.md)
- [Обзор фреймворка DEOS](overview/deos-framework.ru.md)
- [Первые шаги](getting-started/first-steps.ru.md)
- [Система AAA](overview/aaa-system.ru.md)
- [Обзор Governance](overview/governance-overview.ru.md)
- [Базовые термины](glossary/core-terms.ru.md)
- [FAQ для новичков](faq/newcomer-faq.ru.md)
