---
type: overview
title: Вики DEOS
description: Самостоятельная карта знаний о фреймворке DEOS и стандарте TMCTOL, устроенная как связный граф понятий, а не перечень ссылок на документацию.
locale: ru
canonical_page_id: index
translation_of: index.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - resource: ../docs/README.md
  - resource: ../README.md
status: stable
audience: newcomer
tags:
  - overview
  - onboarding
  - deos
related:
  - Карта доменов
  - Обзор фреймворка DEOS
  - Первые шаги
  - Система Actors
  - Управление
  - Экономика созидателей $BLDR
  - Базовые термины
  - Вопросы новичков
---

# Вики DEOS

## Кратко

DEOS — фреймворк среды исполнения для программируемых экономик, на основе которого можно создавать собственные экосистемы. Он объединяет выпуск токенов, ликвидность во владении протокола, маршрутизацию, стейкинг, управление и автоматизированных Actors в единую систему с детерминированными правилами. TMCTOL — первый стандарт на базе DEOS: однонаправленная эмиссия по кривой, ликвидность, принадлежащая казне, сжигание комиссий, правила распределения по корзинам и управление в чётко заданных пределах.

Главная идея: DEOS заменяет ситуативное управление казной DAO детерминированными экономическими контурами.

Эта вики — самостоятельная карта знаний о фреймворке. Она собрана из источников, в которых хранится истина проекта, но основной путь чтения остаётся внутри вики: страницы объясняют понятия напрямую и ведут к связанным страницам, не вынуждая читателя постоянно обращаться к исходным документам.

## С чего начать

- [С чего начать](getting-started/start-here.ru.md) — выберите цель: понять DEOS за 10 минут, запустить его локально или безопасно изменить экономику в производном проекте;
- [DEOS за 60 секунд](getting-started/deos-in-60-seconds.ru.md) — краткая главная идея перед знакомством с архитектурой;
- [Первые шаги](getting-started/first-steps.ru.md) — более широкий вводный маршрут;
- [Маршруты чтения](getting-started/reading-paths.ru.md) — подбор страниц по роли и задаче;
- [Карта доменов](concepts/domain-map.ru.md) — основная карта предметных областей;
- [Базовые термины](glossary/core-terms.ru.md) — словарь для всего графа.

## Для быстрой оценки

- [Предложение для партнёров](getting-started/partner-pitch.ru.md) — зачем DEOS может быть нужен партнёрской команде;
- [Краткое резюме](getting-started/executive-summary.ru.md) — одностраничный обзор для представителей экосистемы и инвесторов;
- [Сквозные сценарии](concepts/end-to-end-flows.ru.md) — конкретные процессы маршрутизации, Actors, корзин, стейкинга и проверки;
- [Архитектурные схемы](concepts/architecture-diagrams.ru.md) — компактные текстовые карты связей между подсистемами;
- [Обзор фреймворка DEOS](overview/deos-framework.ru.md) — что представляет собой фреймворк.

## Основные предметные области

- [Стандарт TMCTOL](concepts/tmctol-standard.ru.md) — экономический стандарт и правила обращения токена;
- [Сценарии корзин TOL](concepts/tol-bucket-scenarios.ru.md) — конкретные пробуждения корзин A/B/C/D и направления средств казны;
- [Экономика созидателей $BLDR](concepts/builder-economy.ru.md) — оплата полезной работы, целевое финансирование и модель основателя как первого участника труда;
- [Роли токенов](concepts/token-surfaces.ru.md) — назначение Native, VETO, BLDR, расписок и LP;
- [Система Actors](overview/actor-system.ru.md) — автономные исполнители протокола;
- [DEOS Router](overview/router.ru.md) — маршрутизация, комиссии и выбор ликвидности;
- [Типизированные наблюдения](overview/typed-observations.ru.md) — актуальные скалярные данные и доставка изменений;
- [Управление](overview/governance.ru.md) — полномочия и защита по отдельным доменам;
- [Стейкинг](overview/staking.ru.md) — расписки, назначение LP и вознаграждения;
- [Эталонный клиент](overview/reference-client.ru.md) — браузерный продукт с приоритетом данных из блокчейна и средством чтения вики.

## Экономика и среда исполнения

- [Токен-управляемая автоматизация](concepts/token-driven-automation.ru.md)
- [Контур маршрутизации и эмиссии](concepts/routing-and-minting-loop.ru.md)
- [Token Minting Curve](overview/token-minting-curve.ru.md)
- [Формулы TMCTOL](math/tmctol-formulas.ru.md)
- [Экономические пороги](concepts/economic-thresholds.ru.md)
- [Уровни экономических утверждений](concepts/economic-claim-levels.ru.md)
- [Карта инвариантов и угроз](concepts/invariant-map.ru.md)
- [Идентичность активов](overview/asset-identity.ru.md)
- [Принципы среды исполнения](overview/runtime-patterns.ru.md)
- [DEOS в экосистеме парачейнов](concepts/parachain-context.ru.md)
- [Стратегия использования случайности](overview/randomness-strategy.ru.md)

## Управление, данные для чтения и клиент

- [Домены управления](concepts/governance-domains.ru.md)
- [Экономическая физика прежде политики](comparisons/physics-vs-politics.ru.md)
- [Разделение данных для чтения](concepts/read-model-split.ru.md)
- [Собранная вики](concepts/generated-wiki.ru.md)

## Работа с проектом и его состояние

- [Статус разработки](development/status.ru.md)
- [Трёхуровневая проверка](development/three-layer-validation.ru.md)
- [Слой скриптов](usage/scripts-layer.ru.md)
- [Создание форка DEOS](usage/forking-deos.ru.md)
- [Структура репозитория](implementation/repository-structure.ru.md)
- [Технологический стек](implementation/tech-stack.ru.md)
- [Руководство участника](community/contributing.ru.md)
- [Вопросы новичков](faq/newcomer-faq.ru.md)

## Как читать эту вики

- Сначала переходите по ссылкам внутри вики: она должна быть понятна сама по себе;
- Возвращайтесь к [Карте доменов](concepts/domain-map.ru.md), если отдельная страница кажется слишком узкой;
- Открывайте [Базовые термины](glossary/core-terms.ru.md), когда встречаете много новых понятий;
- Сверяйтесь со [Статусом разработки](development/status.ru.md), чтобы отличать реализованную основу от работы, зависящей от будущих условий;
- Метаданные страницы указывают происхождение сведений, но не задают обязательный путь чтения.

## Связанные страницы

- [Карта доменов](concepts/domain-map.ru.md)
- [Обзор фреймворка DEOS](overview/deos-framework.ru.md)
- [Первые шаги](getting-started/first-steps.ru.md)
- [Система Actors](overview/actor-system.ru.md)
- [Управление](overview/governance.ru.md)
- [Экономика созидателей $BLDR](concepts/builder-economy.ru.md)
- [Базовые термины](glossary/core-terms.ru.md)
- [Вопросы новичков](faq/newcomer-faq.ru.md)
