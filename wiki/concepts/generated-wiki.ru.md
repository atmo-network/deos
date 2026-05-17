---
page_type: concept
title: Generated Wiki
summary: Generated Wiki — самодостаточный доменный слой объяснений поверх источников проекта. Он нужен, чтобы читатель понимал DEOS внутри wiki, а не ходил по файловым путям документации.
locale: ru
canonical_page_id: generated-wiki
translation_of: generated-wiki.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../docs/README.md
  - ../../wiki/_meta/navigation.json
  - ../../wiki/_meta/state.json
  - ../../wiki/_meta/graph.json
status: active
audience: newcomer
tags:
  - wiki
  - onboarding
  - metadata
related:
  - Карта доменов
  - Эталонный клиент
  - UI Kit и Domain DAG
  - Первые шаги
  - Базовые термины
last_compiled: 2026-05-17
confidence: 0.94
---

# Generated Wiki

## Кратко

Wiki DEOS — самодостаточный слой объяснений. Она опирается на проектную правду репозитория, но читателю не нужно выходить из wiki, чтобы понять страницу перед собой.

Ее задача — превратить DEOS из дерева файлов в смысловой граф доменов для людей, агентов и эталонного клиента: экономическая физика, автономные акторы, маршрутизация, governance, staking, модели чтения, клиент, инструменты и future gates.

## Контракт страницы

Хорошая wiki-страница:

- Объясняет локальное понятие напрямую;
- Называет домен, которому оно принадлежит;
- Связывает себя с соседними wiki-страницами;
- Не повторяет целиком объяснение, которым владеет другая страница;
- Держит происхождение источников в метаданных, а не превращает source documents в маршрут чтения.

Wiki может объединять несколько исходных понятий на одной странице, если так граница домена становится яснее. Используйте [Карту доменов](domain-map.ru.md) как верхнеуровневого владельца доменной топологии.

## Метаданные и клиент

Эталонный клиент читает compiled manifests из `wiki/_meta/`:

- `navigation.json` для разделов и кратких описаний;
- `aliases.json` для поисковых терминов;
- `graph.json` для типизированных связей между страницами;
- `state.json` для статуса, confidence, sources и audience;
- `locales.json` для поддерживаемых локалей и путей.

Эти манифесты помогают навигации и поиску. Основной текст все равно должен быть самодостаточным.

## Граница доверия и развитие

Веб-клиент напрямую рендерит repo-local wiki markdown, потому что wiki считается проверенным содержимым репозитория, а не пользовательским вводом. Безопасность держится на repository validation: запрещены raw HTML blocks, опасные URL-схемы, inline DOM event handlers и frontmatter summary lines с лишними value-side colons.

Когда wiki развивается, сначала обновляйте страницу-владельца, повторы заменяйте ссылками к владельцу, provenance держите в метаданных и проверяйте trust contract плюс форму ссылок.

## Связанные страницы

- [Карта доменов](domain-map.ru.md)
- [Эталонный клиент](../overview/reference-client.ru.md)
- [Metadata wiki-графа](../usage/wiki-graph-metadata.ru.md)
- [UI Kit и Domain DAG](ui-kit-and-domain-dag.ru.md)
- [Первые шаги](../getting-started/first-steps.ru.md)
- [Базовые термины](../glossary/core-terms.ru.md)
