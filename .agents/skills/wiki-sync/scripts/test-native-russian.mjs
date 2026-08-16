#!/usr/bin/env node

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import {
  auditText,
  auditTextDetailed,
  extractManifestEvidence,
  extractRussianWidgetStrings,
  stripNonProse,
} from './audit-native-russian.mjs';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(scriptDir, '../../../..');
const config = JSON.parse(readFileSync(resolve(scriptDir, '../native-russian-style.json'), 'utf8'));

function matches(source, customConfig = config, options = {}) {
  return auditText(source, customConfig, 'fixture', options).map(({ match }) => match);
}

function parseRussianRelationLabels() {
  const source = readFileSync(resolve(projectRoot, 'web-client/src/lib/wiki/relation-phrases.ts'), 'utf8');
  const labels = new Map();
  for (const match of source.matchAll(/^\s*(?:'([^']+)'|([A-Za-z-]+)):\s*'([^']+)',$/gm)) {
    labels.set(match[1] ?? match[2], match[3]);
  }
  return labels;
}

test('detects reviewed borrowings, noun chains, and mixed-language calques', () => {
  const source = 'Нужны architecture graph, onboarding, read-model, runtime routing, platform-паттерны и substrate-привычки.';
  const found = matches(source);
  for (const expected of ['architecture graph', 'onboarding', 'read-model', 'runtime routing', 'platform-паттерны', 'substrate-привычки']) {
    assert.ok(found.includes(expected), `missing ${expected}`);
  }
});

test('detects every named false-negative family', () => {
  const source = [
    'governance-cell subject power surfaces payload-family cadence execution authority',
    'runtime routing validation fallback collator relay-chain beacon launch-line',
    'product widgets layout feedback owner surfaces floor/ceiling thresholds',
    'акторы абстракции аккаунта, заблокированная весом голова, обычные очередь с пробуждениями, грязная лента',
    'breaker Weight tombstones каденции кулдауна бэк-оффа, чистые владельцы, находки о циклах',
    'канонические поверхности среды исполнения, поверхность-владелец, изменённая поверхность, доверительная граница',
    'Продуктовая поверхность в браузере, проверенный Markdown, Шаблоны среды исполнения, Экономика $BLDR',
    'Создание проекта на базе DEOS; материализованными поверхностями; поверхностью управления и заблокированных LP',
  ].join('\n');
  const found = matches(source).join(' | ');
  for (const expected of [
    'governance-cell', 'subject', 'power surfaces', 'payload-family', 'cadence', 'execution authority',
    'runtime routing', 'validation', 'fallback', 'collator', 'relay-chain beacon', 'launch-line',
    'product widgets', 'layout', 'feedback', 'owner surfaces', 'floor/ceiling thresholds',
    'акторы абстракции аккаунта', 'заблокированная весом голова', 'обычные очередь с пробуждениями',
    'грязная лента', 'breaker', 'Weight', 'tombstones', 'каденции', 'кулдауна', 'бэк-оффа',
    'чистые владельцы', 'находки о циклах', 'канонические поверхности среды исполнения',
    'поверхность-владелец', 'изменённая поверхность', 'доверительная граница',
    'Продуктовая поверхность в браузере', 'проверенный Markdown', 'Шаблоны среды исполнения',
    'Создание проекта на базе', 'материализованными поверхностями',
    'поверхностью управления и заблокированных',
  ]) {
    assert.ok(found.includes(expected), `missing ${expected}`);
  }
});

test('masks fenced and inline code without changing length or newline positions', () => {
  const source = 'До\n```ts\nvalidation\nruntime\n```\nПосле `onboarding` и debt.';
  const masked = stripNonProse(source, config.canonical_terms);
  assert.equal(masked.length, source.length);
  assert.deepEqual([...masked.matchAll(/\n/g)].map((match) => match.index), [...source.matchAll(/\n/g)].map((match) => match.index));
  assert.deepEqual(matches(source).filter((value) => value !== 'debt'), []);
});

test('masks nested Markdown link destinations while retaining displayed labels', () => {
  const source = '[runtime routing [onboarding](docs/onboarding.md)](../read-model.en.md)';
  assert.deepEqual(matches(source).sort(), ['onboarding', 'runtime routing'].sort());
});

test('masks relative, repository, HTTP, and file URLs and paths', () => {
  const source = 'docs/onboarding.md ../read-model.en.md ./runtime/path /tmp/validation https://example.test/on-chain file:///tmp/runtime';
  assert.equal(matches(source).length, 0);
  assert.equal(stripNonProse(source, config.canonical_terms).length, source.length);
});

test('masks unmarked source declarations and records', () => {
  const source = 'const confidence = 1; provenance: true\nprovenance: true\nОбычный provenance текст';
  const found = matches(source);
  assert.deepEqual(found, ['provenance']);
});

test('uses exact identifier boundaries rather than substring deletion', () => {
  const boundaryConfig = {
    canonical_terms: ['Pass', 'Intent', 'LP'],
    forbidden: [
      { pattern: 'Password|Intentional|HELP|Pass|Intent|LP', guidance: 'fixture', class: 'fixture' },
    ],
  };
  assert.deepEqual(matches('Password Intentional HELP Pass Intent LP', boundaryConfig), ['Password', 'Intentional', 'HELP']);
});

test('deduplicates overlaps by source span and retains the longest match', () => {
  const overlapConfig = {
    canonical_terms: [],
    forbidden: [
      { pattern: '\\brepo-local\\b', guidance: 'short', class: 'short' },
      { pattern: '\\btrusted repo-local markdown\\b', guidance: 'long', class: 'long' },
    ],
  };
  const result = auditTextDetailed('trusted repo-local markdown', overlapConfig);
  assert.equal(result.raw.length, 2);
  assert.deepEqual(result.findings.map(({ match }) => match), ['trusted repo-local markdown']);
});

test('reports real Markdown lines when frontmatter and fences precede debt', () => {
  const source = '---\ntitle: DEOS\ncanonical_page_id: runtime\n---\n```text\nonboarding\n```\nСтрока runtime';
  const findings = auditText(source, config, 'page', { frontmatter: true });
  assert.equal(findings.find(({ match }) => match === 'runtime')?.line, 8);
});

test('protects canonical exact display fields and link titles but audits the same phrase in prose', () => {
  const source = '---\ntitle: Token Minting Curve\ndescription: Кратко\n---\n[Token Minting Curve](page.ru.md) и обычный текст Token Minting Curve без пояснения.';
  assert.deepEqual(matches(source, config, { frontmatter: true }), ['Token Minting Curve']);
});

test('protects canonical terms in code spans rather than broad prose substrings', () => {
  assert.deepEqual(matches('Тип `Actor Contract` называется Actor Contract без пояснения.'), ['Actor Contract']);
});

test('separates Russian search alias keys from displayed manifest prose', () => {
  const manifest = {
    aliases: { ru: { 'English search alias': 'page-id', 'Русская метка': 'other-id' } },
  };
  const evidence = extractManifestEvidence(manifest, 'aliases.json');
  assert.equal(evidence.displayRecords.length, 0);
  assert.deepEqual(evidence.searchAliases.map(({ value }) => value), ['English search alias', 'Русская метка']);
  assert.equal(evidence.searchAliases[0].pointer, '/aliases/ru/English search alias');
});

test('reports manifest display strings by escaped JSON Pointer', () => {
  const evidence = extractManifestEvidence({ title: { ru: 'runtime routing' }, 'a/b': { ru: 'validation' } }, 'graph.json');
  assert.deepEqual(evidence.displayRecords.map(({ pointer }) => pointer), ['/title/ru', '/a~1b/ru']);
});

test('extracts only displayed Russian WikiWidget string values', () => {
  const source = `const widgetText = currentLocale === 'ru' ? {\n  provenance: 'Происхождение сведений',\n  confidence: 'Степень обоснованности',\n  helper: 'runtime routing',\n} : { provenance: 'Compiled provenance' };`;
  const records = extractRussianWidgetStrings(source);
  assert.deepEqual(records.map(({ value }) => value), ['Происхождение сведений', 'Степень обоснованности', 'runtime routing']);
  assert.deepEqual(records.map(({ line }) => line), [2, 3, 4]);
  assert.deepEqual(records.flatMap(({ value }) => matches(value)), ['runtime routing']);
});

test('does not scan Svelte and TypeScript identifiers around the Russian object', () => {
  const source = `let provenance = true;\nconst widgetText = currentLocale === 'ru' ? { title: 'Вики' } : { title: 'Wiki' };\nconst confidence = 1;`;
  assert.deepEqual(extractRussianWidgetStrings(source).map(({ value }) => value), ['Вики']);
});

test('covers every graph relation with one compact endpoint-independent Russian label', () => {
  const widget = readFileSync(resolve(projectRoot, 'web-client/src/lib/widgets/WikiWidget.svelte'), 'utf8');
  const graph = JSON.parse(readFileSync(resolve(projectRoot, 'wiki/_meta/graph.json'), 'utf8'));
  const labels = parseRussianRelationLabels();
  const relationTypes = new Set(graph.edges.map(({ type }) => type));
  assert.equal(relationTypes.size, 92);
  assert.equal(labels.size, 92);
  assert.deepEqual([...relationTypes].filter((type) => !labels.has(type)), []);
  for (const [type, label] of labels) {
    assert.ok(label, `${type} lacks a label`);
    assert.ok(label.length <= 56, `${type} is not compact`);
    assert.doesNotMatch(label, /(?:эта|этой|связанн(?:ая|ой|ую)) страница/iu);
  }
  assert.match(widget, /itemsById\.get\(edge\.from\)/);
  assert.match(widget, /itemsById\.get\(edge\.to\)/);
  assert.match(widget, /formatRelation\(edge\.type, source\.title, target\.title\)/);
  assert.doesNotMatch(widget, /forward|inverse/);
  assert.match(widget, /\{relation\.source\}/);
  assert.match(widget, /\{relation\.label\}/);
  assert.match(widget, /\{relation\.target\}/);
});

test('keeps targeted reviewer acceptance labels without claiming full-surface fluency', () => {
  const labels = parseRussianRelationLabels();
  const expected = {
    'current-documentation-surface': 'представление статуса в текущей документации',
    'current-product-surface': 'представление статуса в текущем клиенте',
    entrypoint: 'переход к',
    'explains-protection-bias': 'объяснение приоритета защиты',
    'implements-read-model-honesty': 'реализация честного представления данных для чтения',
    'instance-view': 'представление отдельного экземпляра',
    'overview-parent': 'включение в общий обзор',
    'publishes-observations': 'публикация типизированных наблюдений',
    requires: 'требование',
    'subsystem-view': 'выделение подсистемы',
    'system-context': 'размещение в контексте системы',
  };
  for (const [type, label] of Object.entries(expected)) {
    assert.equal(labels.get(type), label, type);
  }
});

test('guards reviewed Actors grammar and Russian formula explanations', () => {
  const actorsFiles = [
    'wiki/concepts/end-to-end-flows.ru.md',
    'wiki/development/status.ru.md',
    'wiki/concepts/token-driven-automation.ru.md',
  ];
  for (const path of actorsFiles) {
    const source = readFileSync(resolve(projectRoot, path), 'utf8');
    assert.doesNotMatch(source, /разреженная Continuation/u, path);
    assert.match(source, /разреженное состояние Continuation/u, path);
  }
  const status = readFileSync(resolve(projectRoot, 'wiki/development/status.ru.md'), 'utf8');
  assert.doesNotMatch(status, /конкретных источника субсидии/u);
  assert.match(status, /конкретного источника субсидии и правила расчёта суммы/u);

  const actorSystem = readFileSync(resolve(projectRoot, 'wiki/overview/actor-system.ru.md'), 'utf8');
  assert.doesNotMatch(actorSystem, /все еще/u);
  assert.match(actorSystem, /всё ещё/u);
  assert.doesNotMatch(actorSystem, /Наблюдение, ручной, только периодический/u);
  assert.doesNotMatch(actorSystem, /в переиспользуемом состоянии измененного канала/u);
  assert.match(actorSystem, /переиспользуемом состоянии с последним изменением канала/u);

  const staking = readFileSync(resolve(projectRoot, 'wiki/overview/staking.ru.md'), 'utf8');
  assert.doesNotMatch(staking, /Устаревшие общий механизм/u);
  assert.match(staking, /Удалены общий механизм поблочных наград/u);

  const formulas = readFileSync(resolve(projectRoot, 'wiki/math/tmctol-formulas.ru.md'), 'utf8');
  assert.match(formulas, /Инвариант XYK: k = R_native × R_foreign \(постоянная величина\)/u);
  assert.match(formulas, /После продажи ΔS нативных токенов:/u);
  assert.match(formulas, /Цена = R_foreign' \/ R_native'/u);
  assert.doesNotMatch(formulas, /XYK Invariant|After selling|\bPrice\s*=/u);
});

test('keeps reviewed Russian navigation labels equal to canonical page titles', () => {
  const fixtures = [
    ['wiki/index.ru.md', 'Токен-управляемая автоматизация', 'concepts/token-driven-automation.ru.md'],
    ['wiki/index.ru.md', 'Контур маршрутизации и эмиссии', 'concepts/routing-and-minting-loop.ru.md'],
    ['wiki/index.ru.md', 'Token Minting Curve', 'overview/token-minting-curve.ru.md'],
    ['wiki/index.ru.md', 'Создание форка DEOS', 'usage/forking-deos.ru.md'],
    ['wiki/index.ru.md', 'Руководство участника', 'community/contributing.ru.md'],
    ['wiki/getting-started/reading-paths.ru.md', 'Токен-управляемая автоматизация', '../concepts/token-driven-automation.ru.md'],
    ['wiki/getting-started/reading-paths.ru.md', 'Контур маршрутизации и эмиссии', '../concepts/routing-and-minting-loop.ru.md'],
    ['wiki/getting-started/reading-paths.ru.md', 'Token Minting Curve', '../overview/token-minting-curve.ru.md'],
    ['wiki/getting-started/reading-paths.ru.md', 'Создание форка DEOS', '../usage/forking-deos.ru.md'],
    ['wiki/concepts/domain-map.ru.md', 'Токен-управляемая автоматизация', 'token-driven-automation.ru.md'],
    ['wiki/concepts/domain-map.ru.md', 'Контур маршрутизации и эмиссии', 'routing-and-minting-loop.ru.md'],
    ['wiki/concepts/domain-map.ru.md', 'Token Minting Curve', '../overview/token-minting-curve.ru.md'],
    ['wiki/concepts/economic-claim-levels.ru.md', 'Карта инвариантов и угроз', 'invariant-map.ru.md'],
  ];
  for (const [path, title, target] of fixtures) {
    const source = readFileSync(resolve(projectRoot, path), 'utf8');
    assert.ok(source.includes(`[${title}](${target})`), `${path} lacks canonical link title ${title}`);
  }
  const claims = readFileSync(resolve(projectRoot, 'wiki/concepts/economic-claim-levels.ru.md'), 'utf8');
  assert.equal([...claims.matchAll(/\[Карта инвариантов и угроз\]\(invariant-map\.ru\.md\)/g)].length, 1);
  assert.doesNotMatch(claims, /(?:^|\n)\s*- (?:\[)?Карта инвариантов(?:\]|$)/u);
});

test('masks exact systems identifiers in code while rejecting them in prose', () => {
  assert.deepEqual(matches('`Weight` `breaker` `tombstones`'), []);
  assert.deepEqual(matches('Weight breaker tombstones'), ['Weight breaker tombstones']);
  assert.deepEqual(matches('Экономика $BLDR'), []);
});

test('preserves natural Russian alternatives', () => {
  assert.equal(matches('Собранная вики показывает проекцию данных для чтения, материализованное представление и происхождение сведений.').length, 0);
});
