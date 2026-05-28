---
page_type: getting-started
title: Маршруты чтения
summary: Самодостаточные маршруты по wiki для новичков, экономических изменений, runtime-задач, governance, клиентской работы, проверки статуса и инструментов.
locale: ru
canonical_page_id: reading-paths
translation_of: reading-paths.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../docs/README.md
  - ../../README.md
  - ../../BACKLOG.md
  - ../../CHANGELOG.md
  - ../../web-client/README.md
  - ../../template/README.md
  - ../../scripts/README.md
status: active
audience: newcomer
tags:
  - onboarding
  - reading-path
  - documentation
related:
  - Начните здесь
  - Карта доменов
  - Первые шаги
  - Generated Wiki
  - Статус разработки
  - Обзор фреймворка DEOS
  - Базовые термины
last_compiled: 2026-05-17
confidence: 0.91
---

# Маршруты чтения

## Кратко

У DEOS есть несколько связанных доменов: идентичность фреймворка, экономическая физика, автономные акторы, маршрутизация, governance, стейкинг, модели чтения, клиент, инструменты и будущие внешние условия. Не нужно читать все за один проход.

Если нужен самый короткий вход, сначала используйте [Начните здесь](start-here.ru.md). Эта страница нужна после выбора onboarding-пути, когда требуется более широкий role-based маршрут чтения.

## Если вы совсем новичок

1. [Начните здесь](start-here.ru.md)
2. [DEOS за 60 секунд](deos-in-60-seconds.ru.md)
3. [Для кого DEOS](who-deos-is-for.ru.md)
4. [Partner Pitch](partner-pitch.ru.md)
5. [Executive Summary](executive-summary.ru.md)
6. [Маршрут оценки для партнера](../usage/partner-evaluation-route.ru.md)
7. [Обзор фреймворка DEOS](../overview/deos-framework.ru.md)
8. [Карта доменов](../concepts/domain-map.ru.md)
9. [Архитектурные схемы](../concepts/architecture-diagrams.ru.md)
10. [Базовые термины](../glossary/core-terms.ru.md)
11. [Сквозные сценарии](../concepts/end-to-end-flows.ru.md)
12. [Стандарт TMCTOL](../concepts/tmctol-standard.ru.md)
13. [Токен-управляемая автоматизация](../concepts/token-driven-automation.ru.md)
14. [FAQ для новичков](../faq/newcomer-faq.ru.md)

Этот маршрут дает словарь проекта до того, как появятся имена pallet-ов, детали runtime или термины конкретной реализации.

## Если вы оцениваете DEOS извне

1. [DEOS за 60 секунд](deos-in-60-seconds.ru.md)
2. [Для кого DEOS](who-deos-is-for.ru.md)
3. [Partner Pitch](partner-pitch.ru.md)
4. [Executive Summary](executive-summary.ru.md)
5. [Маршрут оценки для партнера](../usage/partner-evaluation-route.ru.md)
6. [DEOS vs DAO Treasury](../comparisons/deos-vs-dao-treasury.ru.md)
7. [Чем DEOS не является](../concepts/what-deos-is-not.ru.md)
8. [Стандарт TMCTOL](../concepts/tmctol-standard.ru.md)
9. [Уровни экономических утверждений](../concepts/economic-claim-levels.ru.md)
10. [Threat Model](../concepts/threat-model.ru.md)
11. [Минимальный профиль форка](../usage/minimal-fork-profile.ru.md)
12. [Эталонный клиент](../overview/reference-client.ru.md)

Этот маршрут для партнеров, ecosystem readers и technical evaluators, которым сначала нужны мем, границы, карта рисков и обязанности форка, а уже потом topology реализации.

## Если вы меняете экономику

1. [Карта доменов](../concepts/domain-map.ru.md)
2. [Стандарт TMCTOL](../concepts/tmctol-standard.ru.md)
3. [Формулы TMCTOL](../math/tmctol-formulas.ru.md)
4. [Экономические пороги](../concepts/economic-thresholds.ru.md)
5. [Уровни экономических утверждений](../concepts/economic-claim-levels.ru.md)
6. [Карта инвариантов](../concepts/invariant-map.ru.md)
7. [Threat Model](../concepts/threat-model.ru.md)
8. [Сценарии TOL buckets](../concepts/tol-bucket-scenarios.ru.md)
9. [Сквозные сценарии](../concepts/end-to-end-flows.ru.md)
10. [Token Minting Curve](../overview/token-minting-curve.ru.md)
11. [Axial Router](../overview/axial-router.ru.md)
12. [Трехуровневая валидация](../development/three-layer-validation.ru.md)

Экономическая работа должна сохранять различие между формулами, поведением runtime и интеграционными эффектами. Маршрут wiki должен показать, какой домен вы меняете, до запуска более глубокого набора проверок.

## Если вы меняете поведение runtime

1. [Паттерны runtime](../overview/runtime-patterns.ru.md)
2. [Система AAA](../overview/aaa-system.ru.md)
3. [Сквозные сценарии](../concepts/end-to-end-flows.ru.md)
4. [Идентичность активов](../overview/asset-identity.ru.md)
5. [Контур маршрутизации и минтинга](../concepts/routing-and-minting-loop.ru.md)
6. [Разделение read-model](../concepts/read-model-split.ru.md)
7. [Трехуровневая валидация](../development/three-layer-validation.ru.md)

Работа с runtime сначала должна определить затронутый домен, затем понять, является ли изменение только математическим, локальным для pallet-а, интегрированным в runtime или видимым в клиенте.

## Если вы меняете governance

1. [Обзор Governance](../overview/governance-overview.ru.md)
2. [Домены Governance](../concepts/governance-domains.ru.md)
3. [Physics-first против politics-first](../comparisons/physics-vs-politics.ru.md)
4. [Пулы стейкинга](../concepts/staking-pools.ru.md)
5. [Разделение read-model](../concepts/read-model-split.ru.md)
6. [Базовые термины](../glossary/core-terms.ru.md)

Работа с governance должна держать отдельно конституционную защиту, primary tracks, типизированные семейства payload и полномочия исполнения.

## Если вы меняете веб-клиент

1. [Эталонный клиент](../overview/reference-client.ru.md)
2. [UI Kit и Domain DAG](../concepts/ui-kit-and-domain-dag.ru.md)
3. [Разделение read-model](../concepts/read-model-split.ru.md)
4. [Generated Wiki](../concepts/generated-wiki.ru.md)
5. [Слой скриптов](../usage/scripts-layer.ru.md)
6. [Статус разработки](../development/status.ru.md)

Работа с клиентом должна сохранять повторное использование UI Kit, правила владения Domain DAG, честное происхождение данных в read-model и доверенную границу рендеринга wiki.

## Если вы проверяете текущий статус или историю релизов

1. [Статус разработки](../development/status.ru.md)
2. [Карта доменов](../concepts/domain-map.ru.md)
3. [FAQ для новичков](../faq/newcomer-faq.ru.md)
4. [Generated Wiki](../concepts/generated-wiki.ru.md)
5. [Базовые термины](../glossary/core-terms.ru.md)

Работа со статусом должна отделять поставленную базовую линию, открытый backlog, завершенные поставки и будущие внешне-зависимые задачи. Wiki объясняет эту границу, но не превращается в заметки к релизам.

## Если вы работаете со скриптами или локальными инструментами

1. [Слой скриптов](../usage/scripts-layer.ru.md)
2. [Трехуровневая валидация](../development/three-layer-validation.ru.md)
3. [Паттерны runtime](../overview/runtime-patterns.ru.md)
4. [Статус разработки](../development/status.ru.md)
5. [Структура репозитория](../implementation/repository-structure.ru.md)
6. [Технологический стек](../implementation/tech-stack.ru.md)
7. [Parachain context](../concepts/parachain-context.ru.md)
8. [Форк DEOS](../usage/forking-deos.ru.md)
9. [Минимальный профиль форка](../usage/minimal-fork-profile.ru.md)
10. [Чем DEOS не является](../concepts/what-deos-is-not.ru.md)

Работа с инструментами и fork work должна оставаться ограниченной, явной и честной по требованиям запуска, сохраненным framework contracts и поведению.

## Связанные страницы

- [Начните здесь](start-here.ru.md)
- [DEOS за 60 секунд](deos-in-60-seconds.ru.md)
- [Для кого DEOS](who-deos-is-for.ru.md)
- [Partner Pitch](partner-pitch.ru.md)
- [Executive Summary](executive-summary.ru.md)
- [Маршрут оценки для партнера](../usage/partner-evaluation-route.ru.md)
- [DEOS vs DAO Treasury](../comparisons/deos-vs-dao-treasury.ru.md)
- [Карта доменов](../concepts/domain-map.ru.md)
- [Первые шаги](first-steps.ru.md)
- [Generated Wiki](../concepts/generated-wiki.ru.md)
- [Статус разработки](../development/status.ru.md)
- [Обзор фреймворка DEOS](../overview/deos-framework.ru.md)
- [Базовые термины](../glossary/core-terms.ru.md)
