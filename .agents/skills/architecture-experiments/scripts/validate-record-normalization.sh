#!/usr/bin/env bash
set -euo pipefail

skill_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
template="$skill_dir/templates/EXP-NNNN.md"

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

node --input-type=module - "$template" "${records[@]}" <<'NODE'
import fs from 'node:fs';
import path from 'node:path';

const [template, ...records] = process.argv.slice(2);
const failures = [];
const expectedFields = extractFields(fs.readFileSync(template, 'utf8'), template);
const expectedSections = extractSections(fs.readFileSync(template, 'utf8'));

for (const record of records) {
  const source = fs.readFileSync(record, 'utf8');
  const fields = extractFields(source, record);
  const sections = extractSections(source);
  const track = path.basename(path.dirname(record));
  const primaryTrack = fieldValue(source, 'Primary track');

  compare(record, 'metadata fields', expectedFields, fields);
  compare(record, 'second-level sections', expectedSections, sections);

  if (primaryTrack !== `[${track}](./experiments.md)`) {
    failures.push(`${record}: Primary track must be [${track}](./experiments.md), found: ${primaryTrack}`);
  }
}

if (failures.length > 0) {
  console.error('Experiment Record normalization failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(`Experiment Record normalization passed: ${records.length} record(s), ${expectedFields.length} metadata field(s), ${expectedSections.length} section(s)`);

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
