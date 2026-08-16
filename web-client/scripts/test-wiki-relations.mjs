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

import {
  formatRussianRelation,
  russianRelationLabels,
  unknownRussianRelationLabel,
} from '../src/lib/wiki/relation-phrases.ts';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, '../..');
const graph = JSON.parse(
  readFileSync(resolve(repoRoot, 'wiki/_meta/graph.json'), 'utf8'),
);
const widget = readFileSync(
  resolve(repoRoot, 'web-client/src/lib/widgets/WikiWidget.svelte'),
  'utf8',
);
const titlesById = new Map(graph.nodes.map((node) => [node.id, node.title.ru]));
const relationTypes = [...new Set(graph.edges.map(({ type }) => type))].sort();

// These reviewer-selected examples are an independent acceptance fixture, not a
// claim that automation proves the fluency or meaning of all 92 labels. Human
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
      'index→first-steps',
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

test('defines exactly one concise canonical Russian label for all 92 graph relation types', () => {
  assert.equal(relationTypes.length, 92);
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
    'status-and-release-route': 'путь к сведениям о статусе и выпуске',
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
