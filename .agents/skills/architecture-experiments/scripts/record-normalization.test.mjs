import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { validate } from './record-normalization.mjs';
import { fileURLToPath } from 'node:url';

export function selfTest(realSkillDir) {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'deos-proof-graph-'));
  const skill = path.join(temp, '.agents/skills/architecture-experiments');
  const track = path.join(skill, 'tracks/actors');
  fs.mkdirSync(track, { recursive: true });
  fs.mkdirSync(path.join(skill, 'templates'));
  const template = fs.readFileSync(path.join(realSkillDir, 'templates/EXP-NNNN.md'), 'utf8');
  fs.writeFileSync(path.join(skill, 'templates/EXP-NNNN.md'), template);
  const sectionNames = [...template.matchAll(/^## (.*)$/gm)].map((m) => m[1]);
  const metaNames = [...template.matchAll(/^\| ([^|]+) \| .* \|$/gm)].map((m) => m[1]);
  const firstSection = template.indexOf('\n## ');
  const metadata = metaNames.filter((name) => template.slice(0, firstSection).includes(`| ${name} |`) && !['Field', '---'].includes(name));
  const relationNames = [...template.split('\n## Relations\n')[1].matchAll(/^- `([^`]+)`:/gm)].map((m) => m[1]);
  const link = (id) => `[${id}](./${id}.md)`;
  function record(id, kind, parent = '', child = '') {
    const values = { Status: 'Measuring', 'Record kind': kind, 'Former IDs': 'None', 'Parent question': parent ? link(parent) : 'None', 'Primary track': '[actors](./experiments.md)', 'Architecture release / experiment campaign': 'fixture' };
    const rel = { 'Depends on': child ? link(child) : 'None', 'Decomposes into': child ? link(child) : 'None', 'Satisfies obligation': parent ? `${link(parent)} O1` : 'None' };
    const proof = ['- `Freeze`: Frozen at Measuring on fixture source.', '- `Review triggers`: None.', '- `Decomposition review`: None.', '', '| Obligation ID | Claim | Smallest falsifier | Evidence class | Owning Experiment | Required/conditional | Downstream consequence | Status |', '| --- | --- | --- | --- | --- | --- | --- | --- |', `| O1 | A bounded proof | One legal witness | Native construction | ${child ? link(child) : 'Self'} | Required | Decides admission | Open |`].join('\n');
    return `# ${id} — Fixture\n\n| Field | Value |\n| --- | --- |\n${metadata.map((n) => `| ${n} | ${values[n] ?? 'None'} |`).join('\n')}\n\n${sectionNames.map((name) => `## ${name}\n\n${name === 'Proof Obligations' ? proof : name === 'Relations' ? relationNames.map((n) => `- \`${n}\`: ${rel[n] ?? 'None'}.`.replace('None..', 'None.')).join('\n') : 'Bounded fixture evidence.'}`).join('\n\n')}\n`;
  }
  const parentId = 'EXP-0090', childId = 'EXP-0091', parentFile = path.join(track, `${parentId}.md`), childFile = path.join(track, `${childId}.md`);
  const indexFile = path.join(track, 'experiments.md');
  const parent = record(parentId, 'Synthesis', '', childId), child = record(childId, 'Leaf', parentId);
  const initialIndex = `# Test index\n\n| ID | Release | Status |\n| --- | --- | --- |\n| ${link(parentId)} | fixture | Measuring |\n| ${link(childId)} | fixture | Measuring |\n\n<!-- experiment-graph-campaign: fixture -->\n<!-- experiment-dependencies:start -->\n<!-- experiment-dependencies:end -->\n`;
  let count = 0;
  const options = { repoFiles: [parentFile, childFile, indexFile] };
  function reset() {
    fs.writeFileSync(parentFile, parent); fs.writeFileSync(childFile, child); fs.writeFileSync(indexFile, initialIndex);
    fs.rmSync(path.join(skill, 'migrations'), { force: true, recursive: true });
    fs.rmSync(path.join(skill, 'tracks/router'), { force: true, recursive: true });
    const result = validate(skill, { ...options, writeIndex: true });
    assert.deepEqual(result.errors, [], result.errors.join('\n'));
  }
  function rejects(name, mutate, pattern) {
    reset(); mutate();
    const errors = validate(skill, options).errors.join('\n');
    assert.match(errors, pattern, `${name}: mutation was not rejected\n${errors}`); count++;
  }
  const change = (file, from, to) => fs.writeFileSync(file, fs.readFileSync(file, 'utf8').replace(from, to));
  function router(id, materialized = false) {
    const directory = path.join(skill, 'tracks/router');
    fs.mkdirSync(directory, { recursive: true });
    fs.writeFileSync(path.join(directory, 'experiments.md'), `| ID | Release | Status |\n| --- | --- | --- |\n| ${materialized ? link(id) : id} | fixture | ${materialized ? 'Measuring' : 'Proposed'} |\n`);
    if (materialized) fs.writeFileSync(path.join(directory, `${id}.md`), record(id, 'Leaf').replace('[actors](./experiments.md)', '[router](./experiments.md)'));
  }
  try {
    reset(); assert.deepEqual(validate(skill, options).errors, []); count++;
    rejects('cross-track record collision', () => router(childId, true), /global experiment ID/);
    rejects('cross-track index-only collision', () => router(childId), /global experiment ID/);
    rejects('two index-only tracks collide', () => {
      fs.appendFileSync(indexFile, '\n| EXP-0092 | fixture | Proposed |\n');
      router('EXP-0092');
    }, /global experiment ID/);
    reset(); router('EXP-0092');
    assert.deepEqual(validate(skill, options).errors, []); count++;
    rejects('missing kind', () => change(childFile, '| Record kind | Leaf |\n', ''), /Record kind/);
    rejects('invalid kind', () => change(childFile, '| Record kind | Leaf |', '| Record kind | Notebook |'), /Record kind/);
    rejects('unowned synthesis obligation', () => change(parentFile, `| ${link(childId)} | Required |`, '| None | Required |'), /owning experiment/);
    rejects('missing parent backlink', () => change(childFile, `| Parent question | ${link(parentId)} |`, '| Parent question | None |'), /not reciprocal/);
    rejects('missing child edge', () => change(parentFile, `- \`Decomposes into\`: ${link(childId)}.`, '- `Decomposes into`: None.'), /not reciprocal/);
    rejects('wrong obligation backlink', () => change(childFile, `${link(parentId)} O1`, `${link(parentId)} O2`), /does not satisfy obligation/);
    rejects('extra unknown obligation', () => change(childFile, `${link(parentId)} O1`, `${link(parentId)} O1; ${link(parentId)} O2`), /unknown or differently owned O2/);
    rejects('hard cycle', () => change(childFile, '- `Depends on`: None.', `- \`Depends on\`: ${link(parentId)}.`), /hard dependency DAG: cycle/);
    rejects('decomposition self cycle', () => change(parentFile, `- \`Decomposes into\`: ${link(childId)}.`, `- \`Decomposes into\`: ${link(parentId)}.`), /decomposition graph: cycle/);
    rejects('stale projection', () => change(indexFile, '**Evidence flow**', '**Changed graph**'), /stale graph projection/);
    rejects('raw synthesis table', () => change(parentFile, '## Measurements\n\nBounded fixture evidence.', '## Measurements\n\n| RefTime | Reads |\n| --- | --- |\n| 100 | 2 |'), /primary benchmark table/);
    rejects('raw timing sample table', () => change(parentFile, '## Measurements\n\nBounded fixture evidence.', '## Measurements\n\n| Branch | Time sample / storage-root ns |\n| --- | --- |\n| A | 100 / 2 |'), /primary benchmark table/);
    rejects('synthesis command', () => change(parentFile, '## Measurements\n\nBounded fixture evidence.', '## Measurements\n\nRun `scripts/benchmarks.sh --extrinsic foo`.'), /primary benchmark command/);
    rejects('synthesis candidate', () => change(parentFile, '## Baseline and Candidates\n\nBounded fixture evidence.', '## Baseline and Candidates\n\n| A | new candidate |'), /primary benchmark candidate/);
    rejects('unreviewed branch growth', () => change(childFile, '## Measurements\n\nBounded fixture evidence.', `## Measurements\n\n${Array.from({ length: 7 }, (_, i) => `### Proof ${i}\n\nWitness.`).join('\n\n')}`), /six proof subsections/);
    for (const trigger of ['new mandatory obligation', 'new Weight owner', 'new reachable domain', 'new production selector']) rejects(trigger, () => change(childFile, '- `Review triggers`: None.', `- \`Review triggers\`: ${trigger}.`), /trigger without review/);
    rejects('unfrozen measuring', () => change(childFile, 'Frozen at Measuring on fixture source.', 'Not frozen.'), /must be frozen/);
    rejects('multiple leaf claims', () => change(childFile, '| O1 | A bounded proof', '| O2 | Another claim | One witness | Native | Self | Required | Consumer | Open |\n| O1 | A bounded proof'), /one proof claim/);
    rejects('broken body anchor', () => change(childFile, '## Measurements\n\nBounded fixture evidence.', `## Measurements\n\n[Parent](./${parentId}.md#missing-proof).`), /broken body anchor/);
    reset();
    change(parentFile, '## Measurements\n\nBounded fixture evidence.', '## Measurements\n\n| Phase | Child claim |\n| --- | --- |\n| Admission | Independent RefTime and ProofSize refusal |');
    assert.deepEqual(validate(skill, options).errors, []); count++;
    reset();
    change(childFile, '## Measurements\n\nBounded fixture evidence.', `## Measurements\n\n${'Retained bounded evidence. '.repeat(2000)}`);
    const large = validate(skill, options); assert.equal(large.errors.length, 0); assert(large.warnings.some((w) => w.includes('large record'))); count++;

    // Former identity checks use an actual frozen baseline buffer, never the live file.
    const formerId = 'EXP-0089';
    function migrated(status = 'Proposed') {
      reset();
      change(childFile, '| Former IDs | None |', `| Former IDs | actors/${formerId} |`);
      const baseline = Buffer.from(child.replaceAll(childId, formerId).replace('| Status | Measuring |', `| Status | ${status} |`));
      fs.mkdirSync(path.join(skill, 'migrations'));
      fs.writeFileSync(path.join(skill, 'migrations/fixture.json'), JSON.stringify({ baseline_commit: 'fixture', renumberings: [{ former: `actors/${formerId}`, current: `actors/${childId}` }], files: [{ path: `.agents/skills/architecture-experiments/tracks/actors/${formerId}.md`, status, mode: '0644', bytes: baseline.length, sha256: createHash('sha256').update(baseline).digest('hex') }] }));
      return { ...options, gitRead: () => baseline };
    }
    const good = migrated(); assert.deepEqual(validate(skill, good).errors, []); count++;
    router(formerId);
    assert(validate(skill, good).errors.some((e) => e.includes('without baseline qualification'))); count++;
    fs.rmSync(path.join(skill, 'tracks/router'), { recursive: true });
    fs.appendFileSync(indexFile, `\nFormer provisional identity actors/${formerId} now names ${childId}.\n`);
    assert.deepEqual(validate(skill, good).errors, []); count++;
    fs.appendFileSync(indexFile, `\nContinue actors/${formerId}.\n`);
    assert(validate(skill, good).errors.some((e) => e.includes('live former-ID'))); count++;
    for (const state of ['Accepted', 'Rejected', 'Inconclusive', 'Superseded', 'Invalidated', 'Interpreted']) {
      const opts = migrated(state);
      assert(validate(skill, opts).errors.some((e) => e.includes('non-provisional ID')), state); count++;
    }
    // Explicit compaction may reuse only a baseline-qualified provisional identity.
    const qualifiedOptions = migrated();
    const fixtureManifest = path.join(skill, 'migrations/fixture.json');
    const qualified = JSON.parse(fs.readFileSync(fixtureManifest, 'utf8'));
    const epoch = '1'.repeat(40);
    qualified.baseline_commit = epoch;
    qualified.renumberings[0].former += `@${epoch}`;
    fs.writeFileSync(fixtureManifest, JSON.stringify(qualified));
    change(childFile, `actors/${formerId} |`, `actors/${formerId}@${epoch} |`);
    const reusedFile = path.join(track, `${formerId}.md`);
    fs.writeFileSync(reusedFile, record(formerId, 'Leaf'));
    fs.appendFileSync(indexFile, `\n| ${link(formerId)} | fixture | Measuring |\n`);
    assert.deepEqual(validate(skill, qualifiedOptions).errors, []); count++;
    fs.rmSync(path.join(skill, 'migrations'), { recursive: true });
    assert.deepEqual(validate(skill, qualifiedOptions).errors, []); count++;
    assert(validate(skill, { ...qualifiedOptions, gitRead: () => { throw new Error('Missing baseline'); } }).errors.some((e) => e.includes('unavailable Git baseline'))); count++;
    for (const status of ['Accepted', 'Rejected', 'Inconclusive', 'Superseded', 'Invalidated', 'Interpreted']) {
      const bytes = qualifiedOptions.gitRead().toString().replace('| Status | Proposed |', `| Status | ${status} |`);
      assert(validate(skill, { ...qualifiedOptions, gitRead: () => Buffer.from(bytes) }).errors.some((e) => e.includes('non-provisional ID')), status); count++;
    }
    fs.mkdirSync(path.join(skill, 'migrations'));
    fs.writeFileSync(fixtureManifest, JSON.stringify(qualified));
    change(childFile, `actors/${formerId}@${epoch} |`, `actors/${formerId} |`);
    assert(validate(skill, qualifiedOptions).errors.some((e) => e.includes('without baseline qualification'))); count++;
    change(childFile, `actors/${formerId} |`, `actors/${formerId}@${epoch} |`);
    qualified.baseline_commit = '2'.repeat(40);
    fs.writeFileSync(fixtureManifest, JSON.stringify(qualified));
    assert(validate(skill, qualifiedOptions).errors.some((e) => e.includes('qualifier mismatch'))); count++;
    fs.rmSync(reusedFile);
    reset();
    const table = '| RefTime | Reads |\n| --- | --- |\n| 100 | 2 |';
    const baselineSource = child.replace('## Measurements\n\nBounded fixture evidence.', `## Measurements\n\n${table}`);
    const marker = '<!-- extracted: fixture -->';
    fs.writeFileSync(childFile, baselineSource.replace(table, `${marker}\n${table}\n<!-- extracted:end -->`));
    const sourcePath = `.agents/skills/architecture-experiments/tracks/actors/${childId}.md`;
    const start = baselineSource.split('\n').indexOf('| RefTime | Reads |') + 1;
    const digest = (s) => createHash('sha256').update(s).digest('hex');
    const manifest = { baseline_commit: 'fixture', files: [{ path: sourcePath, status: 'Measuring', mode: '0644', bytes: Buffer.byteLength(baselineSource), sha256: digest(baselineSource) }], extractions: [{ source: sourcePath, start_line: start, end_line: start + 2, sha256: digest(table + '\n'), owner: `actors/${childId}`, marker, classification: 'primary-evidence', payload_sha256: digest(table) }], coverage: { 91: { measurement_and_derived_lines: [start, start + 2] } } };
    fs.mkdirSync(path.join(skill, 'migrations'));
    const manifestFile = path.join(skill, 'migrations/extraction.json');
    const save = () => fs.writeFileSync(manifestFile, JSON.stringify(manifest));
    save();
    const extractionOptions = { ...options, gitRead: () => Buffer.from(baselineSource) };
    assert.deepEqual(validate(skill, extractionOptions).errors, []); count++;
    change(childFile, '| 100 | 2 |', '| 101 | 2 |');
    assert(validate(skill, extractionOptions).errors.some((e) => e.includes('payload identity mismatch'))); count++;
    manifest.extractions[0].payload_sha256 = digest(table.replace('100', '101')); save();
    assert(validate(skill, extractionOptions).errors.some((e) => e.includes('decision evidence changed or missing'))); count++;
    manifest.files[0].status = 'Proposed'; save();
    assert(validate(skill, extractionOptions).errors.some((e) => e.includes('baseline status mismatch'))); count++;
    manifest.files[0].status = 'Measuring';
    delete manifest.extractions[0].owner;
    manifest.extractions[0].classification = 'compact-provenance';
    manifest.extractions[0].reason = 'Ordinary chronology'; save();
    assert(validate(skill, extractionOptions).errors.some((e) => e.includes('cannot be discarded'))); count++;
    console.log(`Proof-graph validator self-test passed: ${count} positive/negative cases`);
  } finally { fs.rmSync(temp, { recursive: true, force: true }); }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) selfTest(process.argv[2]);
