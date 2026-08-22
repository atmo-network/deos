#!/usr/bin/env node

import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';

const skill = path.resolve(import.meta.dirname, '..');
const root = path.resolve(skill, '../../..');
const baseline = 'v0.7.22';
const candidate = '0.7.23';
const output = path.join(skill, 'evidence/runtime-weight-delta-ledger.md');
const check = process.argv.includes('--check');
const weightFiles = [
  ['Asset Registry', 'template/runtime/src/weights/pallet_asset_registry.rs'],
  ['Actors', 'template/runtime/src/weights/pallet_deos_actors.rs'],
  ['Governance', 'template/runtime/src/weights/pallet_governance.rs'],
  ['Oracle', 'template/runtime/src/weights/pallet_oracle.rs'],
  ['Router', 'template/runtime/src/weights/pallet_deos_router.rs'],
  ['Staking', 'template/runtime/src/weights/pallet_staking.rs'],
  ['TMC', 'template/runtime/src/weights/pallet_tmc.rs'],
];

function gitShow(ref, file) {
  return execFileSync('git', ['show', `${ref}:${file}`], { cwd: root, encoding: 'utf8' });
}

function functionBodies(source) {
  const methods = new Map();
  const pattern = /\n\s*fn (\w+)\(([^)]*)\) -> Weight \{/g;
  for (const match of source.matchAll(pattern)) {
    const open = source.indexOf('{', match.index);
    let depth = 0;
    let end = open;
    for (; end < source.length; end += 1) {
      if (source[end] === '{') depth += 1;
      if (source[end] === '}') depth -= 1;
      if (depth === 0) {
        end += 1;
        break;
      }
    }
    methods.set(match[1], source.slice(open, end));
  }
  return methods;
}

function number(value) {
  return Number(value.replaceAll('_', ''));
}

function formatNumber(value) {
  return value.toLocaleString('en-US');
}

function sumTerms(terms) {
  const combined = new Map();
  for (const [variable, value] of terms) {
    combined.set(variable, (combined.get(variable) ?? 0) + value);
  }
  const fixed = combined.get('') ?? 0;
  const parts = [];
  if (fixed || combined.size === 1) parts.push(formatNumber(fixed));
  for (const [variable, value] of combined) {
    if (!variable) continue;
    parts.push(`${formatNumber(value)}·${variable}`);
  }
  return parts.join(' + ') || '0';
}

function weightFormula(body) {
  const refTerms = [];
  const proofTerms = [];
  const weightPattern = /Weight::from_parts\(([\d_]+), ([\d_]+)\)(?:\.saturating_mul\((\w+)\.into\(\)\))?/g;
  for (const match of body.matchAll(weightPattern)) {
    const variable = match[3] ?? '';
    const refTime = number(match[1]);
    const proof = number(match[2]);
    if (refTime) refTerms.push([variable, refTime]);
    if (proof) proofTerms.push([variable, proof]);
  }
  const db = (kind) => {
    const terms = [];
    const fixed = new RegExp(`\\.${kind}\\(([\\d_]+)\\)`, 'g');
    const scaled = new RegExp(
      `\\.${kind}\\(\\(([\\d_]+)_u64\\)\\.saturating_mul\\((\\w+)\\.into\\(\\)\\)\\)`,
      'g',
    );
    for (const match of body.matchAll(fixed)) terms.push(['', number(match[1])]);
    for (const match of body.matchAll(scaled)) terms.push([match[2], number(match[1])]);
    return sumTerms(terms);
  };
  return {
    refTime: sumTerms(refTerms),
    proof: sumTerms(proofTerms),
    reads: db('reads'),
    writes: db('writes'),
    baseRefTime: refTerms.find(([variable]) => !variable)?.[1] ?? 0,
  };
}

function delta(oldValue, newValue) {
  if (!oldValue) return 'new';
  const pct = ((newValue - oldValue) / oldValue) * 100;
  if (Math.abs(pct) < 0.005) return '0.00%';
  return `${pct > 0 ? '+' : ''}${pct.toFixed(2)}%`;
}

function reason(pallet, method) {
  if (pallet === 'Asset Registry') return 'I';
  if (pallet === 'Governance') return method.startsWith('service_') ? 'P' : 'C';
  if (['Oracle', 'Router', 'Staking', 'TMC'].includes(pallet)) return 'C';
  if (['fee_collection', 'task_dex_exact_in', 'task_dex_exact_out'].includes(method)) return 'O';
  if (method === 'scheduler_actor_state_probe') return 'M';
  return 'C';
}

const rows = [];
const retired = [];
const sourceHashes = [];
for (const [pallet, file] of weightFiles) {
  const oldSource = gitShow(baseline, file);
  const newSource = await readFile(path.join(root, file), 'utf8');
  sourceHashes.push(`${file}:${createHash('sha256').update(newSource).digest('hex')}`);
  const oldMethods = functionBodies(oldSource);
  const newMethods = functionBodies(newSource);
  for (const [method, body] of newMethods) {
    const current = weightFormula(body);
    const previous = oldMethods.has(method) ? weightFormula(oldMethods.get(method)) : null;
    if (!previous || JSON.stringify(previous) !== JSON.stringify(current)) {
      rows.push({ pallet, method, previous, current, reason: reason(pallet, method) });
    }
  }
  for (const method of oldMethods.keys()) {
    if (!newMethods.has(method)) retired.push(`${pallet} \`${method}\``);
  }
}

const lines = [
  '# Runtime Weight Delta Ledger',
  '',
  '## Evidence Boundary',
  '',
  `This generated ledger compares the production Weight implementations in Git tag \`${baseline}\` with the candidate worktree. RefTime formulas exclude database Weight; reads and writes are therefore recorded independently. ProofSize is the generated conservative estimate. A parameterized formula records its generated slope rather than collapsing it to an unstated component value.`,
  '',
  `Candidate release: \`${candidate}\`. The locally validated production runtime was generated with \`./scripts/03-build-runtime.sh\`; compact Wasm SHA-256 is \`4b04e98b598cb0e72516e12382b742858ba720631f769b60be433d7e1acd989a\`. The accepted benchmark owners use \`frame-omni-bencher 0.22.0\` / CLI \`58.0.0\`, \`50\` steps, \`20\` repeats, compiled Wasm execution, RocksDB, 1,024 MiB cache, host \`fedora\`, and CPU \`AMD Ryzen 7 4800H with Radeon Graphics\`; each generated method records date, reads, writes, measured ProofSize, and conservative ProofSize in its authoritative source. The benchmark-runtime Wasm and production Wasm are distinct evidence identities. Exact candidate commit/tree identity remains unavailable until the validated worktree is committed through the authorized release gate.`, 
  '',
  'Interpretation codes classify changed paths only: `I` identity guard; `C` correctness; `P` bounded service topology; `M` merged canonical work; `O` measured optimization.',
  '',
  '## Changed Production Paths',
  '',
  `| Pallet | Weight method | RefTime: ${baseline} → ${candidate} candidate | Base delta | ProofSize: ${baseline} → ${candidate} candidate | Reads: ${baseline} → ${candidate} candidate | Writes: ${baseline} → ${candidate} candidate | Code |`,
  '| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |',
];
for (const row of rows) {
  const old = row.previous;
  lines.push(
    `| ${row.pallet} | \`${row.method}\` | \`${old?.refTime ?? '—'} → ${row.current.refTime}\` | ${old ? delta(old.baseRefTime, row.current.baseRefTime) : 'new'} | \`${old?.proof ?? '—'} → ${row.current.proof}\` | \`${old?.reads ?? '—'} → ${row.current.reads}\` | \`${old?.writes ?? '—'} → ${row.current.writes}\` | ${row.reason} |`,
  );
}
if (!rows.length) {
  lines.push('| None | No production Weight source delta | `—` | — | `—` | `—` | `—` | — |');
}
lines.push(
  '',
  '## Interpretation',
  '',
  rows.length
    ? 'Every listed dimension requires review against the owning implementation and benchmark evidence. Positive deltas remain unexplained until the release candidate records their measured reason; this generated comparison does not accept them by itself.'
    : `The ${candidate} candidate currently has no production Weight source delta from ${baseline}. Regenerate after accepted Weight changes; this source comparison does not substitute for final production-Wasm benchmark provenance.`,
  '',
  '## Retired Weight Owners',
  '',
  retired.length ? retired.map((item) => `- ${item}`).join('\n') : '- None.',
  '',
  'Any retired owner requires implementation review before release acceptance; absence from the candidate alone does not prove safe replacement.',
  '',
  '## Reproduction',
  '',
  '- Regenerate: `./.agents/skills/release-assurance/scripts/weight-delta-ledger.sh`',
  '- Verify freshness: `./.agents/skills/release-assurance/scripts/weight-delta-ledger.sh --check`',
  '- Reproduce production weights through `./scripts/benchmarks.sh` and the owning Benchmarking Skill; focused outputs do not replace complete generated pallet files.',
  '',
  `Candidate weight source identity: \`${createHash('sha256').update(sourceHashes.join('\n')).digest('hex')}\`.`,
  '',
);

const generated = `${lines.join('\n')}\n`;
if (check) {
  let existing = '';
  try {
    existing = await readFile(output, 'utf8');
  } catch {}
  if (existing !== generated) {
    console.error('[ERROR] Runtime Weight delta ledger is stale');
    process.exit(1);
  }
  console.log('[SUCCESS] Runtime Weight delta ledger is current');
} else {
  await writeFile(output, generated);
  console.log(`[SUCCESS] Wrote ${path.relative(root, output)}`);
}
