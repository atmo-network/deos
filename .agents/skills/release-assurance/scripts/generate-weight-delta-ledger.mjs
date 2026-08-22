#!/usr/bin/env node

import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';

const skill = path.resolve(import.meta.dirname, '..');
const root = path.resolve(skill, '../../..');
const baseline = 'v0.7.20';
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
    rows.push({ pallet, method, previous, current, reason: reason(pallet, method) });
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
  'The candidate files were generated with `frame-omni-bencher 0.22.0` against production runtime Wasm at 50 steps and 20 repeats. Asset Registry and Governance used compact Wasm SHA-256 `b87e7eacebd99fe4e272fd5363e23c75c6693bef2b495d68e39ce16623b39a12`; Oracle, Router, Staking, and TMC used `fd9445658d448278e3f78cda80db488c5cdcdff6550121eb4dbb16494e0f857b`; the final Actors cancellation/wakeup refresh used `af07e3836198baff08830b439fcd9697082285bfc10c4b2f95957969d684c1db`. After version and accepted files were integrated, the production release candidate rebuilt as `7117a599485125acf3e20095aea0d42a29900fe6f067dc24681103669108204e`. Exact release identity remains conditional on the final full-evidence gate and signed release attestation.',
  '',
  'Interpretation codes: `I` is the reserved-location identity guard remeasurement; `C` is a correctness-driven canonical-state, arithmetic, custody, rollback, or scheduler measurement; `P` is bounded phased Governance service; `M` is the merged complete actor-state probe replacing partial probes; `O` is measured duplicate-work deletion or lazy-read optimization.',
  '',
  '## Changed Production Paths',
  '',
  '| Pallet | Weight method | RefTime: 0.7.20 → candidate | Base delta | ProofSize: 0.7.20 → candidate | Reads: 0.7.20 → candidate | Writes: 0.7.20 → candidate | Code |',
  '| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |',
];
for (const row of rows) {
  const old = row.previous;
  lines.push(
    `| ${row.pallet} | \`${row.method}\` | \`${old?.refTime ?? '—'} → ${row.current.refTime}\` | ${old ? delta(old.baseRefTime, row.current.baseRefTime) : 'new'} | \`${old?.proof ?? '—'} → ${row.current.proof}\` | \`${old?.reads ?? '—'} → ${row.current.reads}\` | \`${old?.writes ?? '—'} → ${row.current.writes}\` | ${row.reason} |`,
  );
}
lines.push(
  '',
  '## Interpretation',
  '',
  'The large scheduler wakeup, page-drain, fanout, and Continuation completion increases are accepted correctness costs rather than regressions on unchanged semantics. The prior paths probed only hot, contract, or selected Continuation partitions; the candidate charges the complete five-partition canonical actor-state classification and corruption rejection on every affected branch. `continuation_cancel` additionally measures exact middle-page wakeup invalidation before a retained pending signal is re-primed, preventing a stale physical slot from conflicting with the new live pointer. Governance service increases similarly pay for chronological phased progress, retained same-epoch suffixes, aggregate custody reconciliation, and checked arithmetic. No database or ProofSize increase is hidden inside a RefTime percentage.',
  '',
  'The measured optimization requirement is satisfied independently in multiple production paths. Ledger-only fee collection removes queue signaling and cuts base RefTime by 51.25%. Fresh independent Oracle observations skip duplicate reserve lookup, reducing exact-input Router task RefTime by 6.03% and exact-output by 6.93% in this tag-to-candidate ledger while preserving identical ProofSize and database envelopes. Canonical loaded-state carry also removes repeated actor-state reads from live-head execution; owning Actors architecture evidence records the matched slope comparison.',
  '',
  'Asset Registry coefficients are unchanged or lower apart from run minima comments, so the host-reserved `Here` rejection adds no database access. Governance base coefficients without a service-topology change remain within 4.11% except `cast_vote`; its 11.16% increase is explained by checked aggregate custody and replacement-vote reconciliation. The larger per-ballot slopes for `resolve_proposal_from_votes` and its force variant pay for checked tally folds and typed overflow instead of saturating vote totals. The final Oracle refresh lowers every base coefficient without increasing database or ProofSize envelopes. Router direct mint adds one read and 0.54% base RefTime for the independent reference guard. TMC distribution adds one read and 6.05% base RefTime to prevalidate the checked cumulative native-mint total before any mint; the four writes remain unchanged. Staking paths are lower or unchanged at the base except that retained-epoch work is now represented by explicit RefTime slopes in epoch opening/settlement rather than hidden in a fixed coefficient. The candidate rejects any future unexplained positive delta: regenerate this file, inspect each formula and storage annotation, and update semantics or code rather than accepting benchmark noise by default.',
  '',
  '## Retired Weight Owners',
  '',
  retired.length ? retired.map((item) => `- ${item}`).join('\n') : '- None.',
  '',
  'The two retired Actors partition probes are replaced by `scheduler_actor_state_probe`; no runtime binding retains either partial owner.',
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
