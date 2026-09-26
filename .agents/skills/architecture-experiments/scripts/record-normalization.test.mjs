import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { validate } from './record-normalization.mjs';
import { fileURLToPath } from 'node:url';

export function selfTest(realSkillDir) {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'deos-experiments-'));
  const skill = path.join(temp, '.agents/skills/architecture-experiments');
  const track = path.join(skill, 'tracks/actors');
  fs.mkdirSync(track, { recursive: true });
  fs.mkdirSync(path.join(skill, 'templates'));
  const template = fs.readFileSync(path.join(realSkillDir, 'templates/EXP-NNN.md'), 'utf8');
  fs.writeFileSync(path.join(skill, 'templates/EXP-NNN.md'), template);
  const sectionNames = [...template.matchAll(/^## (.*)$/gm)].map((m) => m[1]);
  const head = template.slice(0, template.indexOf('\n## '));
  const metaNames = [...head.matchAll(/^\| ([^|]+) \| .* \|$/gm)].map((m) => m[1].trim()).filter((n) => !['Field', '---'].includes(n));
  const relationNames = [...template.split('\n## Relations\n')[1].matchAll(/^- `([^`]+)`:/gm)].map((m) => m[1]);
  const disposition = ['Benchmark Evidence Status', 'Reassessment Trigger', 'Compared Observation IDs', 'Noise / Stability Evidence', 'Current Authority'];
  const link = (id) => `[${id}](./${id}.md)`;
  function record(id, mechanism, claims = 1) {
    const values = { Status: 'Measured', 'Physical mechanism': mechanism, 'Primary track': '[actors](./experiments.md)' };
    const proof = ['- `Freeze`: Frozen at fixture commit.', '', '| Obligation ID | Claim | Smallest falsifier | Evidence class | Status | Consumer |', '| --- | --- | --- | --- | --- | --- |',
      ...Array.from({ length: claims }, (_, i) => `| O${i + 1} | Bounded claim ${i + 1} | One legal witness | Native construction | Open | \`Area/Owner\` |`)].join('\n');
    const measurements = ['Fixture numbers.', '', '### Benchmark Evidence Disposition', '', ...disposition.map((n) => `- \`${n}\`: ${n === 'Benchmark Evidence Status' ? 'Qualified — fixture.' : 'None.'}`)].join('\n');
    const body = (name) => name === 'Proof Obligations' ? proof : name === 'Measurements' ? measurements : name === 'Relations' ? relationNames.map((n) => `- \`${n}\`: None.`).join('\n') : 'Bounded fixture text.';
    return `# ${id} — Fixture\n\n| Field | Value |\n| --- | --- |\n${metaNames.map((n) => `| ${n} | ${values[n] ?? 'Fixture'} |`).join('\n')}\n\n${sectionNames.map((n) => `## ${n}\n\n${body(n)}`).join('\n\n')}\n`;
  }
  const consolidated = path.join(track, 'EXP-001.md'), leaf = path.join(track, 'EXP-002.md'), index = path.join(track, 'experiments.md'), backlog = path.join(temp, 'BACKLOG.md');
  const indexSource = [
    '# Fixture track', '', '## Current Records', '',
    '| ID | Status | Decision | Consumers |', '| --- | --- | --- | --- |',
    `| ${link('EXP-001')} | Measured | Fixture umbrella | Baseline |`,
    `| ${link('EXP-002')} | Measured | Fixture leaf | \`Area/Owner\` |`, ''].join('\n');
  let count = 0;
  function reset() {
    for (const f of fs.readdirSync(track)) fs.rmSync(path.join(track, f), { recursive: true, force: true });
    fs.rmSync(path.join(skill, 'tracks/router'), { recursive: true, force: true });
    fs.writeFileSync(consolidated, record('EXP-001', 'Service ring', 2));
    fs.writeFileSync(leaf, record('EXP-002', 'Deadline pages'));
    fs.writeFileSync(index, indexSource);
    fs.writeFileSync(backlog, '# Backlog\n\n- [ ] `Area/Owner`: Fixture work.\n');
  }
  const change = (file, from, to) => {
    const source = fs.readFileSync(file, 'utf8');
    assert(source.includes(from), `fixture lacks ${from}`);
    fs.writeFileSync(file, source.replace(from, to));
  };
  function rejects(name, mutate, pattern) {
    reset(); mutate();
    const errors = validate(skill).errors.join('\n');
    assert.match(errors, pattern, `${name}: mutation was not rejected\n${errors}`); count++;
  }
  try {
    reset();
    assert.deepEqual(validate(skill).errors, []); count++;
    rejects('duplicate mechanism family', () => fs.writeFileSync(leaf, record('EXP-002', 'service ring')), /mechanism family already owned/);
    rejects('missing mechanism', () => change(leaf, '| Physical mechanism | Deadline pages |', '| Physical mechanism |  |'), /Physical mechanism is required|metadata fields/);
    reset();
    for (let i = 3; i <= 7; i++) {
      fs.writeFileSync(path.join(track, `EXP-00${i}.md`), record(`EXP-00${i}`, `Mechanism ${i}`));
      fs.appendFileSync(index, `| ${link(`EXP-00${i}`)} | Measured | Extra | Baseline |\n`);
    }
    assert(validate(skill).warnings.some((w) => w.includes('exceed the budget')), 'live record budget warning missing'); count++;
    rejects('duplicate ID across tracks', () => {
      const other = path.join(skill, 'tracks/router');
      fs.mkdirSync(other, { recursive: true });
      fs.writeFileSync(path.join(other, 'EXP-002.md'), record('EXP-002', 'Router topology').replace('[actors](./experiments.md)', '[router](./experiments.md)'));
    }, /global experiment ID/);
    rejects('pre-restart ID citation', () => change(leaf, '## Result\n\nBounded fixture text.', '## Result\n\nSee EXP-0120.'), /four-digit/);
    rejects('missing current row', () => change(index, `| ${link('EXP-002')} | Measured | Fixture leaf | \`Area/Owner\` |\n`, ''), /missing Current Records row/);
    rejects('status mismatch', () => change(index, '| Measured | Fixture leaf |', '| Accepted | Fixture leaf |'), /index status mismatch/);
    rejects('record without consumer', () => change(index, '| Fixture leaf | `Area/Owner` |', '| Fixture leaf | None |'), /current backlog consumer/);
    rejects('dangling consumer', () => change(backlog, '`Area/Owner`', '`Area/Other`'), /dangling backlog consumer/);
    rejects('unknown relation target', () => change(leaf, '- `Uses evidence from`: None.', '- `Uses evidence from`: EXP-099.'), /unknown or retired EXP-099/);
    rejects('hard cycle', () => {
      change(leaf, '- `Depends on`: None.', `- \`Depends on\`: ${link('EXP-001')}.`);
      change(consolidated, '- `Depends on`: None.', `- \`Depends on\`: ${link('EXP-002')}.`);
    }, /cycle/);
    rejects('section drift', () => change(leaf, '## Next Gradient', '## Follow-Up'), /sections differ/);
    rejects('disposition drift', () => change(leaf, '- `Noise / Stability Evidence`: None.\n', ''), /Disposition fields/);
    rejects('unfrozen measured', () => change(leaf, 'Frozen at fixture commit.', 'Not frozen.'), /must be frozen/);
    rejects('broken link', () => change(leaf, '## Result\n\nBounded fixture text.', '## Result\n\nSee [missing](./EXP-404.md).'), /broken link/);
    rejects('csv evidence', () => fs.writeFileSync(path.join(track, 'raw.csv'), 'a,b\n'), /CSV\/TSV/);
    console.log(`Experiment validator self-test passed: ${count} positive/negative cases`);
  } finally { fs.rmSync(temp, { recursive: true, force: true }); }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) selfTest(process.argv[2]);
