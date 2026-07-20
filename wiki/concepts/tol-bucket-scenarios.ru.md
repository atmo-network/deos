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
last_compiled: 2026-07-20
confidence: 0.85
---

# Сценарии TOL buckets

## Кратко

TMCTOL использует buckets ликвидности под контролем казны, чтобы разделять экономические назначения и сохранять происхождение резервов. Текущая эталонная топология отличает неизменяемое хранение в Bucket A от необязательных контуров разматывания и казны B/C/D.

Состояние активации имеет значение: Bucket A служит неизменяемым хранилищем, а Bucket B, C, D и связанные с ними казначейские акторы начинают с плана `Noop`. Для последующего разматывания или казначейского действия нужен явно заданный ограниченный план после подготовки пула и казны; порог баланса не включает эти контуры автоматически.

## Bucket A: immediate liquidity

Bucket A — прямой liquidity lane. Когда minting или routing flows создают protocol-owned liquidity, bucket A ближе всего к immediate market depth и эффекту Gravity Well.

Сценарий:

```text
Спрос пользователя -> маршрут/минтинг -> резерв поступает Liquidity Actor
  -> после активации пула актор добавляет сбалансированную ликвидность
  -> полученный LP поступает в неизменяемый Bucket A
  -> глубина рынка отражает завершенную операцию
```

Bucket A хранит полученный LP; сам он не добавляет ликвидность и не исполняет последующий план.

## Необязательные Buckets B, C и D

Buckets B, C и D сохраняют отдельные политические контуры, но в текущей genesis-конфигурации каждый из них получает расписание по таймеру с планом `Noop`. Связанные акторы Treasury B/C/D также начинают с `Noop`; Bucket D остается явно бездействующим резервом.

Архитектура предоставляет семейство ограниченных планов разматывания, которое после подготовки может снять заданную долю LP и направить возвращенные активы в связанную казну. Наличие такой возможности не означает, что она уже включена:

```text
явное политическое решение и подтверждение готовности
  -> установка ограниченного плана разматывания по таймеру
  -> актор снимает заданную долю LP
  -> возвращенные активы поступают в связанный контур казны
```

Текущий контракт не задает автоматического пробуждения C или D по порогу баланса.

## Зачем paired treasuries

В эталонной топологии у каждого такого bucket есть отдельный счет связанной казны. Эти контуры сохраняют видимыми происхождение средств и политическое назначение, даже пока их акторы остаются `Noop`:

```text
Bucket B -> Treasury B lane
Bucket C -> Treasury C lane
Bucket D -> Treasury D lane
```

Производный форк может менять политику, но должен сохранять происхождение средств в buckets как часть экономического контракта, а не украшение учета. При активации или изменении казначейских контуров и планов акторов необходимо отдельно проверить математику TMCTOL и поведение исполнения AAA.

## Связанные страницы

- [Стандарт TMCTOL](tmctol-standard.ru.md)
- [Сквозные сценарии](end-to-end-flows.ru.md)
- [Architecture diagrams](architecture-diagrams.ru.md)
- [AAA system](../overview/aaa-system.ru.md)
- [Token-driven automation](token-driven-automation.ru.md)
