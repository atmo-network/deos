#!/usr/bin/env bash
set -euo pipefail

skill_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
template="$skill_dir/templates/EXP-NNNN.md"
write_index=0
case "${1:-}" in
  '') ;;
  --write-index) write_index=1 ;;
  --help)
    printf 'Usage: %s [--write-index]\nValidates record shape, causal relations and dependency DAG; --write-index refreshes the graph inside the Actors index.\n' "${0##*/}"
    exit 0 ;;
  *) printf 'error: unknown argument: %s\n' "$1" >&2; exit 1 ;;
esac
if [[ "$#" -gt 1 ]]; then
  printf 'error: expected at most one argument\n' >&2
  exit 1
fi

if [[ ! -f "$template" ]]; then
  printf 'error: canonical template not found: %s\n' "$template" >&2
  exit 1
fi

mapfile -t records < <(find "$skill_dir/tracks" -mindepth 2 -maxdepth 2 -type f -name 'EXP-[0-9][0-9][0-9][0-9].md' -print | sort)
mapfile -t delimited_tables < <(find "$skill_dir/tracks" -type f \( -name '*.csv' -o -name '*.tsv' \) -print | sort)
if [[ "${#delimited_tables[@]}" -gt 0 ]]; then
  printf 'Experiment Record normalization failed: CSV/TSV evidence must be integrated as Markdown tables in the owning record:\n' >&2
  printf -- '- %s\n' "${delimited_tables[@]}" >&2
  exit 1
fi

node --input-type=module - "$template" "$write_index" "${records[@]}" <<'NODE'
import fs from 'node:fs';
import path from 'node:path';

const [template, writeIndex, ...records] = process.argv.slice(2);
const failures = [];
const expectedFields = extractFields(fs.readFileSync(template, 'utf8'), template);
const expectedSections = extractSections(fs.readFileSync(template, 'utf8'));
const expectedRelations = [...extractRelations(fs.readFileSync(template, 'utf8')).keys()];
const nodes = new Map();
const indexes = new Map();

for (const record of records) {
  const source = fs.readFileSync(record, 'utf8');
  const fields = extractFields(source, record);
  const sections = extractSections(source);
  const track = path.basename(path.dirname(record));
  const primaryTrack = fieldValue(source, 'Primary track');
  const relations = extractRelations(source);
  const id = path.basename(record, '.md');
  const indexFile = path.join(path.dirname(record), 'experiments.md');
  if (!indexes.has(track)) indexes.set(track, fs.readFileSync(indexFile, 'utf8'));
  const index = indexes.get(track);
  const indexRow = index.split('\n').find((line) => line.startsWith(`| [${id}](`));

  compare(record, 'metadata fields', expectedFields, fields);
  compare(record, 'second-level sections', expectedSections, sections);
  compare(record, 'relation fields', expectedRelations, [...relations.keys()]);
  const relationCount = [...(source.split(/^## Relations\s*$/m)[1] ?? '').matchAll(/^- `([^`]+)`:/gm)].length;
  if (relationCount !== relations.size) failures.push(`${record}: duplicate relation field`);
  if (!indexRow || indexRow.split('|')[3]?.trim() !== fieldValue(source, 'Status')) {
    failures.push(`${record}: missing index row or record/index status mismatch`);
  }
  nodes.set(`${track}/${id}`, { track, id, record, source, relations });

  if (primaryTrack !== `[${track}](./experiments.md)`) {
    failures.push(`${record}: Primary track must be [${track}](./experiments.md), found: ${primaryTrack}`);
  }
}

for (const [track, index] of indexes) {
  const table = index.match(/^\| ID \| Depends on \| Uses evidence from \|.*\n\|[^\n]+\n((?:\| EXP-\d{4} \|[^\n]+\n)+)/m);
  if (!table) continue;
  const header = table[0].split('\n')[0].split('|').slice(1, -1).map((cell) => cell.trim());
  for (const row of table[1].trimEnd().split('\n')) {
    const values = row.split('|').slice(1, -1).map((cell) => cell.trim());
    const id = values[0];
    const key = `${track}/${id}`;
    if (nodes.has(key)) {
      failures.push(`${key}: index-only relations duplicate an owning record`);
      continue;
    }
    if (values.length !== header.length) failures.push(`${key}: malformed index-only relation row`);
    nodes.set(key, {
      track, id, record: path.resolve(path.dirname(template), '../tracks', track, 'experiments.md'),
      source: '', relations: new Map(header.slice(1).map((name, i) => [name, values[i + 1] ?? ''])),
    });
  }
}
const dependencies = new Map();
for (const [key, node] of nodes) {
  for (const [relation, value] of node.relations) {
    if (!value) failures.push(`${node.record}: empty ${relation}; use None when absent`);
    const targets = relationTargets(node, value);
    for (const target of targets) {
      const [track, id] = target.split('/');
      if (!nodes.has(target) && !indexes.get(track)?.split('\n').some((line) => line.startsWith(`| ${id} |`))) {
        failures.push(`${node.record}: ${relation} references unknown experiment ${target}`);
      }
    }
    if (relation === 'Depends on') dependencies.set(key, targets);
  }
}
const visited = new Set();
const visiting = new Set();
function visit(key, chain = []) {
  if (visiting.has(key)) {
    failures.push(`dependency cycle: ${[...chain, key].join(' -> ')}`);
    return;
  }
  if (visited.has(key)) return;
  visiting.add(key);
  for (const target of dependencies.get(key) ?? []) visit(target, [...chain, key]);
  visiting.delete(key);
  visited.add(key);
}
for (const key of nodes.keys()) visit(key);

const indexFile = path.join(path.dirname(template), '../tracks/actors/experiments.md');
const indexSource = fs.readFileSync(indexFile, 'utf8');
const graph = renderActorsGraph();
const graphPattern = /<!-- experiment-dependencies:start -->[\s\S]*?<!-- experiment-dependencies:end -->/;
const retainedGraph = indexSource.match(graphPattern)?.[0];
if (!retainedGraph) {
  failures.push(`${indexFile}: missing dependency graph markers`);
} else if (failures.length === 0 && writeIndex === '1') {
  fs.writeFileSync(indexFile, indexSource.replace(graphPattern, graph));
} else if (retainedGraph !== graph) {
  failures.push(`${indexFile}: stale dependency graph; run this validator with --write-index`);
}

if (failures.length > 0) {
  console.error('Experiment Record normalization failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(`Experiment Record normalization passed: ${records.length} record(s), ${expectedFields.length} metadata field(s), ${expectedSections.length} section(s), ${expectedRelations.length} relation(s); dependency DAG and index graph valid`);

function extractRelations(source) {
  const section = source.split(/^## Relations\s*$/m)[1] ?? '';
  return new Map([...section.matchAll(/^- `([^`]+)`: *(.*)$/gm)].map((m) => [m[1], m[2].trim()]));
}

function relationTargets(node, value) {
  const targets = new Set();
  const plain = value.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_, label, url) => {
    if (/^\.{1,2}\//.test(url)) {
      const file = path.resolve(path.dirname(node.record), url.split('#')[0]);
      if (!fs.existsSync(file)) failures.push(`${node.record}: broken relation link ${url}`);
      if (/\/EXP-\d{4}\.md$/.test(file)) targets.add(`${path.basename(path.dirname(file))}/${path.basename(file, '.md')}`);
      else for (const id of label.match(/EXP-\d{4}/g) ?? []) targets.add(`${node.track}/${id}`);
    }
    return '';
  });
  for (const id of plain.match(/EXP-\d{4}/g) ?? []) targets.add(`${node.track}/${id}`);
  return targets;
}

function renderActorsGraph() {
  const selected = [...nodes.values()].filter((n) => n.source && n.track === 'actors' && Number(n.id.slice(4)) >= 23);
  const keys = new Set(selected.map((n) => `actors/${n.id}`));
  const name = (id) => `E${id.slice(4)}`;
  const lines = ['<!-- experiment-dependencies:start -->', '```mermaid', 'flowchart TD'];
  for (const n of selected) {
    const title = n.source.split('\n')[0].split(' — ')[1].replaceAll('"', "'");
    const label = n.id === 'EXP-0025' ? 'C1 FROZEN / architecture only' : `${title} / ${fieldValue(n.source, 'Status')}`;
    lines.push(`  ${name(n.id)}["${n.id}: ${label}"]`);
  }
  for (const n of selected) {
    for (const dep of dependencies.get(`actors/${n.id}`) ?? []) {
      if (keys.has(dep)) lines.push(`  ${name(dep.split('/')[1])} --> ${name(n.id)}`);
    }
    if (n.id === 'EXP-0025') {
      for (const input of relationTargets(n, n.relations.get('Uses evidence from') ?? '')) {
        if (keys.has(input) && !dependencies.get(`actors/${n.id}`)?.has(input)) {
          lines.push(`  ${name(input.split('/')[1])} -. negative evidence .-> ${name(n.id)}`);
        }
      }
    }
  }
  lines.push('  E0033 -. measured owner only .-> R["Conditional residual / no ID allocated"]', '```', '<!-- experiment-dependencies:end -->');
  return lines.join('\n');
}

function extractFields(source, file) {
  const table = source.match(/^\| Field \| Value \|\n\| --- \| --- \|\n((?:\|.*\n)+)/m);
  if (!table) {
    failures.push(`${file}: missing canonical metadata table immediately under the title`);
    return [];
  }
  return [...table[1].matchAll(/^\| ([^|]+) \| .* \|$/gm)].map((match) => match[1].trim());
}

function extractSections(source) {
  return [...source.matchAll(/^## ([^#].*)$/gm)].map((match) => match[1].trim());
}

function fieldValue(source, name) {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return source.match(new RegExp(`^\\| ${escaped} \\| (.*) \\|$`, 'm'))?.[1].trim() ?? '<missing>';
}

function compare(file, label, expected, actual) {
  if (JSON.stringify(expected) === JSON.stringify(actual)) return;
  failures.push(`${file}: ${label} differ from template\n  expected: ${expected.join(' | ')}\n  actual:   ${actual.join(' | ')}`);
}
NODE
