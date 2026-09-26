import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const statuses = new Set(['Proposed', 'Prepared', 'Measuring', 'Measured', 'Interpreted', 'Accepted', 'Rejected', 'Inconclusive', 'Superseded', 'Invalidated']);
const kinds = new Set(['Leaf', 'Consolidated']);
const proofFields = ['Obligation ID', 'Claim', 'Smallest falsifier', 'Evidence class', 'Status', 'Consumer'];
const dispositionFields = ['Benchmark Evidence Status', 'Reassessment Trigger', 'Compared Observation IDs', 'Noise / Stability Evidence', 'Current Authority'];
const dispositionStatuses = ['Authoritative', 'Qualified', 'Historical', 'Superseded', 'Invalidated', 'Inconclusive', 'Not applicable'];
const currentFields = ['ID', 'Status', 'Kind', 'Decision', 'Consumers'];
const archiveFields = ['ID', 'Release', 'Status', 'Title', 'Current owner'];
const cells = (line) => line.split(/(?<!\\)\|/).slice(1, -1).map((s) => s.trim());
const fields = (s) => [...(s.match(/^\| Field \| Value \|\n\| --- \| --- \|\n((?:\|.*\n)+)/m)?.[1] ?? '').matchAll(/^\| ([^|]+) \| (.*) \|$/gm)].map((m) => [m[1].trim(), m[2].trim()]);
const section = (s, h) => s.split(`\n## ${h}\n`)[1]?.split(/\n## /)[0]?.trim() ?? '';
const relations = (s) => [...section(s, 'Relations').matchAll(/^- `([^`]+)`: *(.*)$/gm)].map((m) => [m[1], m[2].trim()]);
const headings = (s) => [...s.matchAll(/^## ([^#].*)$/gm)].map((m) => m[1].trim());
const note = (s, name) => s.split('\n').find((l) => l.startsWith(`- \`${name}\`:`))?.split('`:').slice(1).join('`:').trim() ?? '';
const ids = (value) => new Set((value ?? '').match(/EXP-\d{4}/g) ?? []);
const table = (source, heading, columns) => {
  const body = section(source, heading).split('\n').filter((l) => l.startsWith('|'));
  if (!body.length) return null;
  return { header: cells(body[0]), rows: body.slice(2).map(cells), ok: JSON.stringify(cells(body[0])) === JSON.stringify(columns) };
};
function walk(dir) {
  if (!fs.existsSync(dir)) return [];
  return fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name)).flatMap((e) => {
    const p = path.join(dir, e.name);
    return e.isDirectory() ? walk(p) : e.isFile() ? [p] : [];
  });
}

export function validate(skillDir) {
  const errors = [], warnings = [], records = new Map(), archived = new Map(), owners = new Map();
  const fail = (key, message) => errors.push(`${key}: ${message}`);
  const template = fs.readFileSync(path.join(skillDir, 'templates/EXP-NNNN.md'), 'utf8');
  const root = path.resolve(skillDir, '../../..');
  const backlogPath = path.join(root, 'BACKLOG.md');
  const backlog = fs.existsSync(backlogPath) ? fs.readFileSync(backlogPath, 'utf8') : '';
  const backlogOwners = new Set([...backlog.matchAll(/^- \[ \] `([^`]+)`: /gm)].map((m) => m[1]));
  const files = walk(path.join(skillDir, 'tracks'));
  for (const file of files) if (/\.(csv|tsv)$/i.test(file)) fail(file, 'CSV/TSV evidence belongs inline as a Markdown table');

  const indexes = new Map();
  for (const file of files.filter((p) => path.basename(p) === 'experiments.md')) {
    const track = path.basename(path.dirname(file)), source = fs.readFileSync(file, 'utf8');
    const current = table(source, 'Current Records', currentFields), archive = table(source, 'Archive', archiveFields);
    if (current && !current.ok) fail(file, `Current Records columns must be ${currentFields.join(' | ')}`);
    if (archive && !archive.ok) fail(file, `Archive columns must be ${archiveFields.join(' | ')}`);
    indexes.set(track, { file, source, current: current?.ok ? current.rows : [], archive: archive?.ok ? archive.rows : [] });
    for (const row of archive?.ok ? archive.rows : []) {
      const [id, , status, title, owner] = row;
      if (row.length !== archiveFields.length || row.some((v) => !v)) { fail(file, 'incomplete archive row'); continue; }
      if (!/^EXP-\d{4}$/.test(id)) fail(file, `invalid archive ID ${id}`);
      if (!statuses.has(status)) fail(file, `${id} has invalid archived status ${status}`);
      if (!title) fail(file, `${id} lacks a title`);
      if (archived.has(id) || owners.has(id)) fail(file, `global experiment ID ${id} is duplicated`);
      archived.set(id, { track, file, owner: [...ids(owner)][0] ?? null });
      owners.set(id, `${track}/archive`);
    }
  }

  for (const file of files.filter((p) => /\/[^/]+\/EXP-\d{4}\.md$/.test(p))) {
    const source = fs.readFileSync(file, 'utf8'), track = path.basename(path.dirname(file)), id = path.basename(file, '.md'), key = `${track}/${id}`;
    const meta = new Map(fields(source)), rel = new Map(relations(source));
    if (owners.has(id)) fail(key, `global experiment ID ${id} already owned by ${owners.get(id)}`);
    owners.set(id, key);
    const compare = (label, expected, actual) => {
      if (JSON.stringify(expected) !== JSON.stringify(actual)) fail(key, `${label} differ from template (${expected.join(' | ')})`);
    };
    compare('metadata fields', fields(template).map(([k]) => k), [...meta.keys()]);
    compare('sections', headings(template), headings(source));
    compare('relation fields', relations(template).map(([k]) => k), [...rel.keys()]);
    if (meta.get('Primary track') !== `[${track}](./experiments.md)`) fail(key, 'Primary track must link its sibling index');
    const status = meta.get('Status'), kind = meta.get('Record kind');
    if (!statuses.has(status)) fail(key, 'invalid Status');
    if (!kinds.has(kind)) fail(key, 'Record kind must be Leaf or Consolidated');
    const proofs = section(source, 'Proof Obligations'), rows = proofs.split('\n').filter((l) => l.startsWith('|'));
    const obligations = rows.slice(2).map(cells);
    if (JSON.stringify(rows.length ? cells(rows[0]) : []) !== JSON.stringify(proofFields)) fail(key, `proof columns must be ${proofFields.join(' | ')}`);
    if (!obligations.length) fail(key, 'finite Proof Obligations required');
    if (kind === 'Leaf' && obligations.length !== 1) fail(key, 'Leaf must own exactly one obligation');
    if (kind === 'Consolidated' && obligations.length < 2) fail(key, 'Consolidated must own at least two obligations');
    if (new Set(obligations.map((r) => r[0])).size !== obligations.length) fail(key, 'duplicate obligation ID');
    for (const row of obligations) {
      if (row.length !== proofFields.length || row.some((v) => !v)) fail(key, 'incomplete proof obligation');
      if (!/^O\d+$/.test(row[0] ?? '')) fail(key, `invalid obligation ID ${row[0]}`);
    }
    if (!['Proposed', 'Prepared'].includes(status) && !/^Frozen\b/.test(note(proofs, 'Freeze'))) fail(key, 'Measuring-or-later obligations must be frozen');
    const absorbs = ids(meta.get('Absorbs'));
    if (kind === 'Consolidated' && !absorbs.size) fail(key, 'Consolidated must name the archived records it absorbs');
    for (const absorbed of absorbs) {
      if (archived.get(absorbed)?.owner !== id) fail(key, `absorbed ${absorbed} must be archived with current owner ${id}`);
    }
    const disposition = source.split(/^### Benchmark Evidence Disposition\s*$/m)[1]?.split(/^#{2,3} /m)[0];
    if (disposition === undefined) fail(key, 'missing Benchmark Evidence Disposition');
    else {
      const entries = [...disposition.matchAll(/^- `([^`]+)`: *(.+)$/gm)].map((m) => [m[1].trim(), m[2].trim()]);
      if (JSON.stringify(entries.map(([k]) => k)) !== JSON.stringify(dispositionFields)) fail(key, `Benchmark Evidence Disposition fields differ from template (${dispositionFields.join(' | ')})`);
      if (!dispositionStatuses.some((s) => (entries[0]?.[1] ?? '').toLowerCase().startsWith(s.toLowerCase()))) fail(key, 'Benchmark Evidence Disposition status is invalid');
    }
    if (Buffer.byteLength(source) > 40000) warnings.push(`${key}: large record (${Buffer.byteLength(source)} bytes); keep only decision-relevant evidence`);
    records.set(key, { key, id, track, file, source, meta, rel });
  }

  for (const [id, entry] of archived) {
    if (entry.owner && !records.has(`${entry.track}/${entry.owner}`)) fail(entry.file, `${id} names missing current owner ${entry.owner}`);
    if (entry.owner && !ids(records.get(`${entry.track}/${entry.owner}`)?.meta.get('Absorbs')).has(id)) fail(entry.file, `${id} current owner ${entry.owner} does not absorb it`);
  }

  for (const [track, index] of indexes) {
    const listed = new Set();
    for (const row of index.current) {
      const [cell, status, kind, decision, consumers] = row;
      const id = [...ids(cell)][0], key = `${track}/${id}`, record = records.get(key);
      if (row.length !== currentFields.length || row.some((v) => !v)) { fail(index.file, 'incomplete current-record row'); continue; }
      listed.add(key);
      if (!record) { fail(index.file, `current row ${id} has no record file`); continue; }
      if (status !== record.meta.get('Status')) fail(key, 'index status mismatch');
      if (kind !== record.meta.get('Record kind')) fail(key, 'index kind mismatch');
      if (!decision) fail(key, 'index decision missing');
      const named = [...consumers.matchAll(/`([^`]+)`/g)].map((m) => m[1]);
      const baseline = /\bBaseline\b/.test(consumers);
      if (!named.length && !baseline) fail(key, 'a live record needs a current backlog consumer or the Baseline role; archive it otherwise');
      for (const owner of named) if (!backlogOwners.has(owner)) fail(key, `dangling backlog consumer ${owner}`);
    }
    for (const record of records.values()) if (record.track === track && !listed.has(record.key)) fail(record.key, 'missing Current Records row');
  }

  const known = (id) => owners.has(id);
  const hard = new Map();
  for (const record of records.values()) {
    for (const [relation, value] of record.rel) {
      if (!value) fail(record.key, `empty ${relation}; use None`);
      for (const target of ids(value)) if (!known(target)) fail(record.key, `${relation} references unknown ${target}`);
    }
    hard.set(record.id, [...ids(record.rel.get('Depends on'))].filter((id) => records.has(`${record.track}/${id}`)));
    for (const m of record.source.matchAll(/\[[^\]]+\]\((\.{1,2}\/[^)]+)\)/g)) {
      const [relative, anchor] = m[1].split('#'), target = path.resolve(path.dirname(record.file), relative);
      if (!fs.existsSync(target)) { fail(record.key, `broken link ${m[1]}`); continue; }
      if (anchor && target.endsWith('.md')) {
        const slugs = [...fs.readFileSync(target, 'utf8').matchAll(/^#{1,6} (.+)$/gm)].map((h) => h[1].toLowerCase().replace(/[^\p{L}\p{N}_\- ]/gu, '').replaceAll(' ', '-'));
        if (!slugs.includes(anchor)) fail(record.key, `broken anchor ${m[1]}`);
      }
    }
  }
  const done = new Set(), active = new Set();
  const visit = (id, chain) => {
    if (active.has(id)) { fail('hard dependency DAG', `cycle ${[...chain, id].join(' -> ')}`); return; }
    if (done.has(id)) return;
    active.add(id);
    for (const next of hard.get(id) ?? []) visit(next, [...chain, id]);
    active.delete(id); done.add(id);
  };
  for (const id of hard.keys()) visit(id, []);

  return { errors: [...new Set(errors)], warnings: [...new Set(warnings)], recordCount: records.size, archivedCount: archived.size };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [skillDir, mode] = process.argv.slice(2);
  if (mode === '--self-test') {
    execFileSync(process.execPath, [path.join(skillDir, 'scripts/record-normalization.test.mjs'), path.resolve(skillDir)], { stdio: 'inherit' });
  } else {
    const result = validate(path.resolve(skillDir));
    for (const w of result.warnings) console.error(`warning: ${w}`);
    for (const e of result.errors) console.error(`error: ${e}`);
    if (result.errors.length) process.exitCode = 1;
    else console.log(`Experiment records valid: ${result.recordCount} live, ${result.archivedCount} archived`);
  }
}
