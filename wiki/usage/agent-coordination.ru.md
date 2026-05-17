---
page_type: usage
title: Координация агентов
summary: Как DEOS использует repo-local agent skills, ABC context files, wiki sync, validation gates и while-true execution, чтобы синхронизировать работу людей и агентов.
locale: ru
canonical_page_id: agent-coordination
translation_of: agent-coordination.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../docs/README.md
  - ../../.agents/skills/wiki-sync/SKILL.md
  - ../../.agents/skills/alignment/SKILL.md
status: active
audience: developer
tags:
  - usage
  - agents
  - coordination
  - validation
related:
  - Руководство контрибьютора
  - Troubleshooting validation
  - Generated Wiki
  - Трехуровневая валидация
last_compiled: 2026-05-17
confidence: 0.86
---

# Координация агентов

## Кратко

DEOS считает координацию агентов частью архитектуры. Репозиторий достаточно плотный, чтобы людям и агентам были нужны общие context files, повторяемые validation gates и локальные skills, где закреплены правила проекта.

Цель не в automation ради automation. Цель — держать изменения в согласии с framework contract, состоянием backlog и правдой wiki/docs.

## Поверхности координации

Главные поверхности:

- `AGENTS.md`: долговременный protocol проекта и architecture rules;
- `BACKLOG.md`: закрываемые открытые работы и внешние блокеры;
- `CHANGELOG.md`: история завершенных поставок;
- `/docs`: authoritative specification, architecture и implementation truth, из которого wiki берет provenance;
- `/.agents/skills/`: repo-local skills для alignment и wiki work;
- `/wiki`: generated domain graph для людей, агентов и reference client;
- Validation scripts и while-true gates, которые проверяют changed scope.

`/docs` — не просто еще одна папка с текстами. Для агента это primary truth surface: перед изменениями в `/template`, `/web-client`, `/scripts` или `/wiki` нужно найти соответствующий spec/architecture контекст и не подменять его более удобной wiki-выжимкой.

Поэтому context updates являются частью done, а не необязательной уборкой документации.

## Как должна идти agent work

1. Классифицировать touched surface: docs, template, web-client, scripts, simulator или wiki.
2. Прочитать owner context до редактирования.
3. Сделать smallest coherent change.
4. Запустить smallest meaningful validation.
5. Обновить backlog/changelog/wiki/context, если изменилась project truth.
6. Запустить repo-local completion gate при autonomous work.

Если задача обнаружила новый in-scope slice, он должен появиться в backlog или закрыться в том же pass. Evergreen rules должны жить в `AGENTS.md`, а не как бессмертные backlog items.

## Границы skills

- `Wiki-sync` владеет wiki trust, semantic projection и generated-wiki rules.
- `Alignment` владеет while-true checks, hallucination/boundary-drift memory и completion discipline.
- Generic coding work следует coding contract, но DEOS-specific architecture rules берутся из repository context.

Skills — cognitive infrastructure. Если skill кодирует durable project rule, ее нужно считать частью системы, а не disposable helper script.

## Связанные страницы

- [Руководство контрибьютора](../community/contributing.ru.md)
- [Troubleshooting validation](validation-troubleshooting.ru.md)
- [Generated Wiki](../concepts/generated-wiki.ru.md)
- [Metadata wiki-графа](wiki-graph-metadata.ru.md)
- [Трехуровневая валидация](../development/three-layer-validation.ru.md)
