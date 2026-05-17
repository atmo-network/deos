---
page_type: usage
title: Метаданные wiki-графа
summary: Как читать метаданные generated wiki, включая navigation, state, graph, aliases и locales.
locale: ru
canonical_page_id: wiki-graph-metadata
translation_of: wiki-graph-metadata.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../wiki/_meta/navigation.json
  - ../../wiki/_meta/state.json
  - ../../wiki/_meta/graph.json
  - ../../wiki/_meta/aliases.json
  - ../../wiki/_meta/locales.json
  - ../../.agents/skills/wiki-sync/SKILL.md
status: active
audience: developer
tags:
  - usage
  - wiki
  - metadata
  - graph
related:
  - Generated Wiki
  - Координация агентов
  - Reference Client
  - Troubleshooting validation
last_compiled: 2026-05-17
confidence: 0.86
---

# Метаданные wiki-графа

## Кратко

Wiki — это не только markdown-страницы. В `/wiki/_meta` лежат общие метаданные, чтобы reference client, агенты и проверочные скрипты читали один и тот же граф доменов.

Markdown-страницы — слой объяснения для людей. Метаданные — карта для программ и агентов.

## Главные файлы

- `Navigation.json`: упорядоченные разделы чтения и навигации для UI и входных страниц.
- `State.json`: реестр страниц со status, audience, confidence, paths и provenance.
- `Graph.json`: nodes и typed edges между wiki-страницами.
- `Aliases.json`: псевдонимы, которые ведут альтернативные названия к canonical page ids.
- `Locales.json`: карта локалей от page ids к локализованным markdown-файлам.

## Как читать page id

Page id — стабильная идентичность wiki-страницы. Файлы для конкретных языков — только представления этой идентичности.

```text
page id: token-surfaces
  en -> concepts/token-surfaces.en.md
  ru -> concepts/token-surfaces.ru.md
```

Ссылки в тексте могут вести на локальные markdown-пути, но метаданные должны держать стабильные page ids, чтобы инструменты графа не ломались при изменении формулировок.

## Graph edges

`graph.json` описывает связи вроде `uses`, `extends`, `frames`, `guides` или `recommends`. Это не runtime-зависимости, а связи для чтения и понимания концептов.

Edges помогают отвечать на вопросы:

- Какая страница владеет этим concept;
- Какие страницы newcomer должен читать дальше;
- Какие implementation pages опираются на эту domain idea;
- Какие страницы надо проверить, когда меняется concept.

## Provenance и confidence

`state.json` хранит provenance и confidence. Provenance указывает на authoritative sources, а confidence показывает читателям и агентам, насколько зрелая эта проекция.

Wiki-страница может быть ясной и полезной, но все равно указывать на `/docs` как source of truth. Это намеренное разделение ролей.

## Связанные страницы

- [Generated Wiki](../concepts/generated-wiki.ru.md)
- [Координация агентов](agent-coordination.ru.md)
- [Reference Client](../overview/reference-client.ru.md)
- [Troubleshooting validation](validation-troubleshooting.ru.md)
