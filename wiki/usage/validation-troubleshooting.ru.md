---
page_type: usage
title: Troubleshooting validation
summary: Что делать, когда в DEOS падает validation, включая wiki trust, context, web-client, Domain DAG, simulator, Rust и ошибки script layer.
locale: ru
canonical_page_id: validation-troubleshooting
translation_of: validation-troubleshooting.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../web-client/README.md
  - ../../scripts/README.md
  - ../../.agents/skills/wiki-sync/SKILL.md
  - ../../.agents/skills/alignment/SKILL.md
  - ../../scripts/validate-local.sh
status: active
audience: developer
tags:
  - usage
  - validation
  - troubleshooting
  - development
related:
  - Трехуровневая валидация
  - Слой скриптов
  - Технологический стек
  - Статус разработки
last_compiled: 2026-06-13
confidence: 0.86
---

# Troubleshooting validation

## Кратко

Когда validation падает, сначала определите слой. Не переходите к проверкам всего дерева, пока не ясно, где сбой: форма документации, wiki trust, владение frontend-срезами, поведение runtime, математика или локальные инструменты.

Правило простое: исправьте минимальную честную поверхность, затем перезапустите gate, который упал.

## Быстрая triage

- **Wiki trust падает**: смотрите указанный file/line. Частые причины: raw HTML, dangerous links, inline DOM handlers или лишние двоеточия в scalar-строках frontmatter.
- **Context validation падает**: проверьте `AGENTS.md`, `BACKLOG.md`, `CHANGELOG.md`, README-ссылки, покрытие docs index и сломанные markdown-ссылки.
- **Domain DAG падает**: найдите import или ownership boundary. Обычно fix — перенести код в owner slice, а не добавить generic shared bucket.
- **Web-client check/build падает**: отделите Svelte syntax, TypeScript contracts, generated descriptors, adapter boundaries и formatting issues.
- **Simulator падает**: это feedback от economic truth. Сначала перепроверьте formulas, thresholds, precision и invariant assumptions.
- **Rust tests или clippy падают**: начинайте с targeted crate fixes. Поднимайтесь до workspace checks, если diff пересекает runtime или pallet boundaries.
- **Script smoke падает**: запустите `--help`, проверьте prerequisites, environment variables и использование `_common.sh`.
- **While-true gate падает**: pass не завершен. Исправьте падающий слой, обновите backlog/context/changelog, если failure вскрыл durable drift, затем запустите gate снова.

## Recovery pattern

1. Скопируйте точную failing command и первую ошибку, по которой понятно, что чинить.
2. Классифицируйте failure layer.
3. Исправьте owner surface, а не место, где только проявился симптом.
4. Перезапустите smallest failing command.
5. Запускайте aggregate gate только после исчезновения local failure.
6. Для release/wiki/context work завершайте `./.agents/skills/alignment/scripts/while-true-gate.sh --skip-simulator`, если math/runtime behavior не менялся.
7. Обновите wiki, backlog или changelog, если failure вскрыл durable contract gap.

## Чего избегать

- Не скрывайте Domain DAG boundary issue созданием `shared/`.
- Не меняйте runtime math ради broken simulator expectation без доказательства formula.
- Не делайте indexers silent dependency, чтобы избежать bounded read surfaces.
- Не считайте pass завершенным, если while-true gate падает после local checks.

## Связанные страницы

- [Трехуровневая валидация](../development/three-layer-validation.ru.md)
- [Слой скриптов](scripts-layer.ru.md)
- [Технологический стек](../implementation/tech-stack.ru.md)
- [Статус разработки](../development/status.ru.md)
