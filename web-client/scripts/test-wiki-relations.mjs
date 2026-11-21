#!/usr/bin/env node

/*
Domain: Wiki relation rendering tests
Owns: Exhaustive label coverage, graph-role, endpoint-view, fallback, and targeted acceptance regressions.
Excludes: Full-label semantic or Russian-fluency approval, graph authoring, and Wiki content parity.
Zone: Client-local deterministic test entrypoint.
*/
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { auditTextDetailed } from '../../wiki/scripts/audit-native-russian.mjs';
import {
  formatRussianRelation,
  russianRelationLabels,
  unknownRussianRelationLabel,
} from '../src/lib/wiki/relation-phrases.ts';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, '../..');
const graph = JSON.parse(
  readFileSync(resolve(repoRoot, 'wiki/layers/deos/_meta/graph.json'), 'utf8'),
);
const widget = readFileSync(
  resolve(repoRoot, 'web-client/src/lib/widgets/WikiWidget.svelte'),
  'utf8',
);
const styleConfig = JSON.parse(
  readFileSync(resolve(repoRoot, 'wiki/native-russian-style.json'), 'utf8'),
);
const relationSource = readFileSync(
  resolve(scriptDir, '../src/lib/wiki/relation-phrases.ts'),
  'utf8',
);
const titlesById = new Map(graph.nodes.map((node) => [node.id, node.title.ru]));
const relationTypes = [...new Set(graph.edges.map(({ type }) => type))].sort();

function findBalancedObject(text, start) {
  let depth = 0;
  let quote = null;
  for (let index = start; index < text.length; index += 1) {
    const character = text[index];
    if (quote) {
      if (character === '\\') index += 1;
      else if (character === quote) quote = null;
    } else if (character === "'" || character === '"' || character === '`')
      quote = character;
    else if (character === '{') depth += 1;
    else if (character === '}' && --depth === 0) return index + 1;
  }
  return -1;
}

function stringRecords(
  source,
  rangeStart = 0,
  rangeEnd = source.length,
  valuesOnly = false,
) {
  const records = [];
  for (let index = rangeStart; index < rangeEnd; ) {
    const quote = source[index];
    if (quote !== "'" && quote !== '"' && quote !== '`') {
      index += 1;
      continue;
    }
    const start = index;
    let value = '';
    for (index += 1; index < rangeEnd; index += 1) {
      if (source[index] === '\\') {
        value += source[index] + (source[index + 1] ?? '');
        index += 1;
        continue;
      }
      if (source[index] === quote) break;
      value += source[index];
    }
    const isValue = source.slice(rangeStart, start).trimEnd().endsWith(':');
    if (!valuesOnly || isValue) {
      const prefix = source.slice(0, start + 1);
      records.push({
        value,
        start,
        line: (prefix.match(/\n/g)?.length ?? 0) + 1,
        column: start - prefix.lastIndexOf('\n') + 1,
      });
    }
    index += 1;
  }
  return records;
}

function extractRussianWidgetStrings(source) {
  const records = stringRecords(source).filter(({ value }) =>
    /\p{Script=Cyrillic}/u.test(value),
  );
  const widgetStart = source.indexOf('const widgetText');
  const localeStart = source.indexOf("currentLocale === 'ru'", widgetStart);
  const objectStart = source.indexOf('{', localeStart);
  const objectEnd = findBalancedObject(source, objectStart);
  if (widgetStart < 0 || localeStart < 0 || objectStart < 0 || objectEnd < 0)
    throw new Error('Russian WikiWidget copy object not found');
  for (const record of stringRecords(source, objectStart, objectEnd, true)) {
    if (!records.some(({ start }) => start === record.start))
      records.push(record);
  }
  return records
    .sort((left, right) => left.start - right.start)
    .map(({ start: _start, ...record }) => record);
}

// These reviewer-selected examples are an independent acceptance fixture, not a
// claim that automation proves the fluency or meaning of every label. Human
// bilingual review owns that full-surface judgement.
const targetedAcceptanceExamples = {
  'current-documentation-surface': {
    label: 'представление статуса в текущей документации',
    edges: ['status→generated-wiki'],
  },
  'current-product-surface': {
    label: 'представление статуса в текущем клиенте',
    edges: ['status→reference-client'],
  },
  entrypoint: {
    label: 'переход к',
    edges: [
      'index→actor',
      'index→actor-system',
      'index→asset-identity',
      'index→core-terms',
      'index→deos-framework',
      'index→governance',
      'index→governance-domains',
      'index→newcomer-faq',
      'index→physics-vs-politics',
      'index→randomness-strategy',
      'index→read-model-split',
      'index→reference-client',
      'index→router',
      'index→routing-and-minting-loop',
      'index→runtime-patterns',
      'index→staking',
      'index→start-here',
      'index→tmctol-standard',
      'index→token-driven-automation',
      'index→token-minting-curve',
      'index→typed-observations',
    ],
  },
  'explains-protection-bias': {
    label: 'объяснение приоритета защиты',
    edges: ['physics-vs-politics→governance-domains'],
  },
  'implements-read-model-honesty': {
    label: 'реализация честного представления данных для чтения',
    edges: ['reference-client→read-model-split'],
  },
  'overview-parent': {
    label: 'включение в общий обзор',
    edges: ['governance-domains→governance'],
  },
  'publishes-observations': {
    label: 'публикация типизированных наблюдений',
    edges: ['router→typed-observations'],
  },
  requires: {
    label: 'требование',
    edges: [
      'forking-deos→parachain-context',
      'forking-deos→three-layer-validation',
    ],
  },
  'system-context': {
    label: 'размещение в контексте системы',
    edges: ['actor→actor-system'],
  },
};

const staleForbiddenLabels = [
  'текущий раздел документации для',
  'текущая часть продукта для',
  'точка входа к',
  'объяснение защитного уклона',
  'честное представление данных для чтения',
  'обзор для',
  'публикация наблюдений для',
  'необходимый механизм',
  'системный контекст для',
];

function canonicalRelation(edge) {
  return {
    source: titlesById.get(edge.from),
    label: russianRelationLabels[edge.type],
    target: titlesById.get(edge.to),
  };
}

function relationFromEndpointView(edge, selectedId) {
  assert.ok(
    edge.from === selectedId || edge.to === selectedId,
    `${selectedId} is not an endpoint of ${edge.from} → ${edge.to}`,
  );
  return formatRussianRelation(
    edge.type,
    titlesById.get(edge.from),
    titlesById.get(edge.to),
  );
}

test('defines exactly one concise canonical Russian label for every graph relation type', () => {
  assert.ok(relationTypes.length > 0);
  assert.deepEqual(Object.keys(russianRelationLabels).sort(), relationTypes);
  for (const type of relationTypes) {
    const label = russianRelationLabels[type];
    assert.equal(label, label.trim(), `${type}: surrounding whitespace`);
    assert.ok(label.length > 0, `${type}: empty label`);
    assert.ok(label.length <= 56, `${type}: label is not concise`);
    assert.doesNotMatch(label, /[.!?]$/u, `${type}: label must be nominal`);
    assert.doesNotMatch(
      label,
      /(?:эта|этой|связанн(?:ая|ой|ую)) страница/iu,
      `${type}: label depends on the selected endpoint view`,
    );
  }
});

test('renders exact source/type/target roles for both endpoint views of every actual edge', () => {
  assert.ok(graph.edges.length > 0);
  for (const [index, edge] of graph.edges.entries()) {
    const expected = canonicalRelation(edge);
    assert.ok(
      expected.source,
      `edge ${index}: missing RU source title for ${edge.from}`,
    );
    assert.ok(
      expected.target,
      `edge ${index}: missing RU target title for ${edge.to}`,
    );
    assert.ok(expected.label, `edge ${index}: missing label for ${edge.type}`);
    assert.deepEqual(
      relationFromEndpointView(edge, edge.from),
      expected,
      `edge ${index}: outgoing view ${edge.from} → ${edge.to}`,
    );
    assert.deepEqual(
      relationFromEndpointView(edge, edge.to),
      expected,
      `edge ${index}: incoming view ${edge.from} → ${edge.to}`,
    );
  }
});

test('matches targeted reviewer acceptance examples without claiming full-label semantic proof', () => {
  const expected = {
    'answers-with': 'ответ с помощью',
    'automated-by': 'автоматизация посредством',
    defines: 'определение',
    extends: 'дополнение',
    'depends-on': 'зависимость от',
    'depends-on-assets': 'зависимость от активов',
    'explained-by': 'объяснение посредством',
    'framed-by': 'представление через',
    guides: 'направление к',
    'implemented-by': 'реализация посредством',
    'implemented-in-standard': 'реализация в стандарте',
    qualifies: 'ограничение области утверждения',
    'rendered-by-client': 'отображение клиентом',
    'route-fork': 'путь к созданию производной системы',
    'route-local-run': 'путь к локальному запуску',
    'route-understand': 'путь к изучению системы',
    'subsystem-view': 'выделение подсистемы',
    'instance-view': 'представление отдельного экземпляра',
    'release-status-boundary': 'граница статуса и выпуска',
    'related-subsystem': 'связанная подсистема',
  };
  for (const [type, label] of Object.entries(expected)) {
    assert.equal(russianRelationLabels[type], label, type);
    assert.deepEqual(formatRussianRelation(type, 'Источник', 'Цель'), {
      source: 'Источник',
      label,
      target: 'Цель',
    });
  }

  for (const [type, golden] of Object.entries(targetedAcceptanceExamples)) {
    assert.equal(russianRelationLabels[type], golden.label, `${type}: label`);
    const actualEdges = graph.edges
      .filter((edge) => edge.type === type)
      .map((edge) => `${edge.from}→${edge.to}`)
      .sort();
    assert.deepEqual(actualEdges, [...golden.edges].sort(), `${type}: edges`);
  }
});

test('rejects stale reviewer-identified relation phrases', () => {
  const labels = new Set(Object.values(russianRelationLabels));
  for (const phrase of staleForbiddenLabels) {
    assert.equal(labels.has(phrase), false, phrase);
  }
});

test('unknown relation uses a neutral label while retaining explicit canonical endpoints', () => {
  assert.equal(unknownRussianRelationLabel, 'неуточнённая связь');
  assert.deepEqual(
    formatRussianRelation('unknown-relation', 'Источник', 'Цель'),
    {
      source: 'Источник',
      label: 'неуточнённая связь',
      target: 'Цель',
    },
  );
});

test('extracts only displayed Russian WikiWidget string values', () => {
  const source = `const widgetText = currentLocale === 'ru' ? {\n  provenance: 'Происхождение сведений',\n  confidence: 'Степень обоснованности',\n  helper: 'runtime routing',\n} : { provenance: 'Compiled provenance' };`;
  const records = extractRussianWidgetStrings(source);
  assert.deepEqual(
    records.map(({ value }) => value),
    ['Происхождение сведений', 'Степень обоснованности', 'runtime routing'],
  );
  assert.deepEqual(
    records.map(({ line }) => line),
    [2, 3, 4],
  );
  assert.deepEqual(
    records.flatMap(({ value }) =>
      auditTextDetailed(value, styleConfig).findings.map(({ match }) => match),
    ),
    ['runtime routing'],
  );
});

test('does not scan Svelte and TypeScript identifiers around the Russian object', () => {
  const source = `let provenance = true;\nconst widgetText = currentLocale === 'ru' ? { title: 'Вики' } : { title: 'Wiki' };\nconst confidence = 1;`;
  assert.deepEqual(
    extractRussianWidgetStrings(source).map(({ value }) => value),
    ['Вики'],
  );
});

test('keeps rendered Russian copy free of reviewed heuristic debt', () => {
  const inputs = [
    [extractRussianWidgetStrings(widget), 'WikiWidget.svelte'],
    [
      stringRecords(relationSource).filter(({ value }) =>
        /\p{Script=Cyrillic}/u.test(value),
      ),
      'relation-phrases.ts',
    ],
  ];
  for (const [records, source] of inputs) {
    assert.ok(records.length > 0, `${source} has no extracted Russian copy`);
    const findings = records.flatMap(
      ({ value, line, column }) =>
        auditTextDetailed(value, styleConfig, source, {
          baseLine: line,
          baseColumn: column,
        }).findings,
    );
    assert.deepEqual(findings, []);
  }
});

test('WikiWidget constructs incoming and outgoing items from canonical edge endpoints', () => {
  assert.match(widget, /const source = itemsById\.get\(edge\.from\)/u);
  assert.match(widget, /const target = itemsById\.get\(edge\.to\)/u);
  assert.match(
    widget,
    /formatRelation\(edge\.type, source\.title, target\.title\)/u,
  );
  assert.doesNotMatch(widget, /WikiRelationDirection|forward|inverse/u);
  assert.doesNotMatch(widget, /item\.relation\b/u);
  assert.match(widget, /\{widgetText\.relationSource\}:/u);
  assert.match(widget, /\{relation\.source\}/u);
  assert.match(widget, /\{widgetText\.relationLabel\}:/u);
  assert.match(widget, /\{relation\.label\}/u);
  assert.match(widget, /\{widgetText\.relationTarget\}:/u);
  assert.match(widget, /\{relation\.target\}/u);
  assert.doesNotMatch(
    widget,
    /class="[^"]*uppercase[^"]*"[^>]*>\s*\{item\.relation/u,
  );
});
