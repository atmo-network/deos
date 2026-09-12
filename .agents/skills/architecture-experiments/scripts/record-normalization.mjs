import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const sealed = new Set(['Accepted', 'Rejected', 'Inconclusive', 'Superseded', 'Invalidated']);
const provisional = new Set(['Proposed', 'Prepared', 'Measuring', 'Measured']);
const statuses = new Set([...sealed, ...provisional, 'Interpreted']);
const proofFields = ['Obligation ID', 'Claim', 'Smallest falsifier', 'Evidence class', 'Owning Experiment', 'Required/conditional', 'Downstream consequence', 'Status'];
const dispositionFields = ['Benchmark Evidence Status', 'Reassessment Trigger', 'Compared Observation IDs', 'Noise / Stability Evidence', 'Current Authority'];
const dispositionStatuses = ['Authoritative', 'Qualified', 'Historical', 'Superseded', 'Invalidated', 'Inconclusive', 'Not applicable'];
const graphPattern = /<!-- experiment-dependencies:start -->[\s\S]*?<!-- experiment-dependencies:end -->/;
const hash = (value) => createHash('sha256').update(value).digest('hex');
const cells = (line) => line.split(/(?<!\\)\|/).slice(1, -1).map((s) => s.trim());
const fields = (s) => [...(s.match(/^\| Field \| Value \|\n\| --- \| --- \|\n((?:\|.*\n)+)/m)?.[1] ?? '').matchAll(/^\| ([^|]+) \| (.*) \|$/gm)].map((m) => [m[1].trim(), m[2].trim()]);
const section = (s, h) => s.split(`\n## ${h}\n`)[1]?.split(/\n## /)[0]?.trim() ?? '';
const relations = (s) => [...section(s, 'Relations').matchAll(/^- `([^`]+)`: *(.*)$/gm)].map((m) => [m[1], m[2].trim()]);
const headings = (s) => [...s.matchAll(/^## ([^#].*)$/gm)].map((m) => m[1].trim());
const note = (s, name) => s.split('\n').find((l) => l.startsWith(`- \`${name}\`:`))?.split('`:').slice(1).join('`:').trim() ?? '';
function walk(dir) {
  if (!fs.existsSync(dir)) return [];
  return fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name)).flatMap((e) => {
    const p = path.join(dir, e.name);
    return e.isDirectory() ? walk(p) : e.isFile() ? [p] : [];
  });
}

export function validate(skillDir, { writeIndex = false, repoFiles, gitRead } = {}) {
  const errors = [], warnings = [], nodes = new Map(), indexes = new Map();
  const template = fs.readFileSync(path.join(skillDir, 'templates/EXP-NNNN.md'), 'utf8');
  const root = path.resolve(skillDir, '../../..');
  const fail = (key, message) => errors.push(`${key}: ${message}`);
  const compare = (key, label, a, b) => {
    if (JSON.stringify(a) !== JSON.stringify(b)) fail(key, `${label} differ from template (${b.join(' | ')})`);
  };
  const files = walk(path.join(skillDir, 'tracks'));
  for (const file of files) {
    if (/\.(csv|tsv)$/i.test(file)) fail(file, 'CSV/TSV evidence belongs inline in its Leaf');
    if (path.basename(file) === 'experiments.md') indexes.set(path.basename(path.dirname(file)), { file, source: fs.readFileSync(file, 'utf8') });
  }
  for (const file of files.filter((p) => /\/[^/]+\/EXP-\d{4}\.md$/.test(p))) {
    const source = fs.readFileSync(file, 'utf8'), track = path.basename(path.dirname(file)), id = path.basename(file, '.md');
    const meta = new Map(fields(source)), rel = new Map(relations(source)), key = `${track}/${id}`;
    const proofs = section(source, 'Proof Obligations'), rows = proofs.split('\n').filter((l) => l.startsWith('|'));
    const obligations = rows.slice(2).map(cells), kind = meta.get('Record kind');
    const node = { key, track, id, file, source, meta, rel, obligations };
    nodes.set(key, node);
    compare(key, 'metadata fields', fields(template).map(([k]) => k), fields(source).map(([k]) => k));
    compare(key, 'sections', headings(template), headings(source));
    compare(key, 'relation fields', relations(template).map(([k]) => k), relations(source).map(([k]) => k));
    if (meta.get('Primary track') !== `[${track}](./experiments.md)`) fail(key, 'Primary track must link its sibling index');
    if (!statuses.has(meta.get('Status'))) fail(key, 'invalid Status');
    if (!['Leaf', 'Synthesis'].includes(kind)) fail(key, 'Record kind must be Leaf or Synthesis');
    const indexRow = indexes.get(track)?.source.split('\n').find((l) => l.startsWith(`| [${id}](`));
    if (!indexRow || cells(indexRow)[2] !== meta.get('Status')) fail(key, 'missing index row or status mismatch');
    compare(key, 'proof columns', proofFields, rows.length ? cells(rows[0]) : []);
    if (!obligations.length) fail(key, 'finite Proof Obligations required');
    if (kind === 'Leaf' && obligations.length !== 1) fail(key, 'Leaf must own one proof claim');
    if (new Set(obligations.map((r) => r[0])).size !== obligations.length) fail(key, 'duplicate obligation ID');
    for (const row of obligations) {
      if (row.length !== 8 || row.some((v) => !v)) fail(key, 'incomplete proof obligation');
      if (!/^O\d+$/.test(row[0])) fail(key, 'invalid obligation ID');
      if (!/^(Required|Conditional: .+)/.test(row[5] ?? '')) fail(key, `${row[0]} must be Required or name its condition`);
    }
    for (const n of ['Freeze', 'Review triggers', 'Decomposition review']) if (!note(proofs, n)) fail(key, `missing ${n}`);
    if (!['Proposed', 'Prepared'].includes(meta.get('Status')) && !/^Frozen\b/.test(note(proofs, 'Freeze'))) fail(key, 'Measuring-or-later obligations must be frozen');
    const disposition = source.split(/^### Benchmark Evidence Disposition\s*$/m)[1]?.split(/^#{2,3} /m)[0];
    if (disposition !== undefined) {
      const entries = [...disposition.matchAll(/^- `([^`]+)`: *(.+)$/gm)].map((m) => [m[1].trim(), m[2].trim()]);
      if (JSON.stringify(entries.map(([k]) => k)) !== JSON.stringify(dispositionFields)) fail(key, `Benchmark Evidence Disposition fields differ from template (${dispositionFields.join(' | ')})`);
      const status = entries[0]?.[1] ?? '';
      if (!dispositionStatuses.some((s) => status.toLowerCase().startsWith(s.toLowerCase()))) fail(key, 'Benchmark Evidence Disposition status is invalid');
    }
    const count = [...section(source, 'Measurements').matchAll(/^### /gm)].length;
    const triggers = note(proofs, 'Review triggers'), review = note(proofs, 'Decomposition review');
    if (kind === 'Leaf' && count > 6) {
      warnings.push(`${key}: ${count} Measurement proof subsections require decomposition review`);
      if (/^None\b/.test(review)) fail(key, 'more than six proof subsections without decomposition review');
    }
    if (triggers && !/^None\b/.test(triggers)) {
      warnings.push(`${key}: declared decomposition trigger: ${triggers}`);
      if (/^None\b/.test(review)) fail(key, 'declared decomposition trigger without review');
    }
    if (Buffer.byteLength(source) > 40000 || source.split('\n').length > 500) warnings.push(`${key}: large record (${Buffer.byteLength(source)} bytes); size is diagnostic`);
    if (kind === 'Synthesis') {
      const sourceLines = source.split('\n');
      for (const [i, line] of sourceLines.entries()) {
        // Metric names in a workload/child claim are legitimate composition inputs.
        // Primary evidence exposes metric columns, or numeric metric/value rows.
        const columns = line.startsWith('|') ? cells(line) : [];
        const metric = /\b(?:RefTime|ProofSize|Reads|Writes|Samples|median|p95|Recorded proof|Generated model SHA|Time sample|DB sample)\b/i;
        if ((/^\|\s*:?-/.test(sourceLines[i + 1] ?? '') && columns.some((c) => metric.test(c))) ||
            (metric.test(columns[0] ?? '') && columns.slice(1).some((c) => /\d/.test(c)))) fail(key, 'Synthesis contains primary benchmark table');
        if (/`(?:\.\/)?(?:scripts\/benchmarks\.sh|bin\/frame-omni-bencher)\b/.test(line)) fail(key, 'Synthesis contains primary benchmark command');
      }
      if (/^### /m.test(section(source, 'Measurements'))) fail(key, 'Synthesis contains independently measured subsections');
      if (/\|\s*(?:Candidate [A-Z]|[AB]\d?)\s*\|/m.test(section(source, 'Baseline and Candidates'))) fail(key, 'Synthesis contains primary benchmark candidate');
    }
  }
  const indexWrites = new Map();
  for (const [track, index] of indexes) {
    for (const line of index.source.split('\n')) {
      const m = line.match(/^\| (EXP-\d{4}) \|/);
      if (!m || cells(line)[2] !== 'Proposed') continue;
      const key = `${track}/${m[1]}`;
      if (!nodes.has(key)) nodes.set(key, { key, track, id: m[1], file: index.file, source: '', meta: new Map([['Status', 'Proposed']]), rel: new Map(), obligations: [] });
    }
    const table = index.source.match(/^\| ID \| Depends on \| Uses evidence from \|.*\n\|[^\n]+\n((?:\| EXP-\d{4} \|[^\n]+\n)+)/m);
    if (!table) continue;
    const header = cells(table[0].split('\n')[0]);
    for (const line of table[1].trimEnd().split('\n')) {
      const row = cells(line), key = `${track}/${row[0]}`, node = nodes.get(key);
      if (!node || node.source) { fail(key, 'index-only relations missing Proposed row or duplicate record'); continue; }
      node.rel = new Map(header.slice(1).map((name, i) => [name, row[i + 1] ?? '']));
    }
  }
  const globalIds = new Map();
  for (const [key, node] of nodes) {
    if (globalIds.has(node.id)) fail(key, `global experiment ID ${node.id} already owned by ${globalIds.get(node.id)}`);
    else globalIds.set(node.id, key);
  }
  function targets(node, value, check = true) {
    const found = new Set();
    const plain = (value ?? '').replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_, label, url) => {
      if (/^\.{1,2}\//.test(url)) {
        const resolved = path.resolve(path.dirname(node.file), url.split('#')[0]);
        if (check && !fs.existsSync(resolved)) fail(node.key, `broken link ${url}`);
        if (/\/EXP-\d{4}\.md$/.test(resolved)) found.add(`${path.basename(path.dirname(resolved))}/${path.basename(resolved, '.md')}`);
        else for (const id of label.match(/EXP-\d{4}/g) ?? []) found.add(`${node.track}/${id}`);
      }
      return '';
    });
    for (const m of plain.matchAll(/(?:([a-z][a-z0-9-]*)\/)?(EXP-\d{4})/g)) found.add(`${m[1] ?? node.track}/${m[2]}`);
    return found;
  }
  const hard = new Map(), decomposition = new Map(), flow = [], former = new Map();
  for (const [key, node] of nodes) {
    hard.set(key, targets(node, node.rel.get('Depends on')));
    decomposition.set(key, targets(node, node.rel.get('Decomposes into')));
    for (const [relation, value] of node.rel) {
      if (!value) fail(key, `empty ${relation}; use None`);
      for (const target of targets(node, value)) {
        if (!nodes.has(target)) fail(key, `${relation} references unknown ${target}`);
        if (relation === 'Uses evidence from') flow.push([target, key, 'uses']);
        if (relation === 'Produces input for') flow.push([key, target, 'produces']);
        if (['Confirms', 'Invalidates'].includes(relation)) flow.push([key, target, relation.toLowerCase()]);
      }
    }
    if (!node.source) continue;
    const parents = targets(node, node.meta.get('Parent question')), satisfies = targets(node, node.rel.get('Satisfies obligation'));
    for (const match of (node.rel.get('Satisfies obligation') ?? '').matchAll(/\]\(([^)]*EXP-\d{4}\.md)(?:#[^)]*)?\)\s+(O\d+)\b/g)) {
      const resolved = path.resolve(path.dirname(node.file), match[1]);
      const owner = nodes.get(`${path.basename(path.dirname(resolved))}/${path.basename(resolved, '.md')}`);
      if (!owner?.obligations.some((r) => r[0] === match[2] && targets(owner, r[4]).has(key))) fail(key, `Satisfies obligation names unknown or differently owned ${match[2]}`);
    }
    for (const parent of parents) {
      if (nodes.get(parent)?.meta.get('Record kind') !== 'Synthesis') fail(key, `Parent question ${parent} is not Synthesis`);
      if (!targets(nodes.get(parent) ?? node, nodes.get(parent)?.rel.get('Decomposes into')).has(key)) fail(key, `Parent question ${parent} is not reciprocal`);
      if (!satisfies.has(parent)) fail(key, `missing Satisfies obligation for ${parent}`);
    }
    for (const target of satisfies) if (!parents.has(target)) fail(key, `Satisfies obligation ${target} is not a parent`);
    if (node.meta.get('Record kind') === 'Leaf' && decomposition.get(key).size) fail(key, 'Leaf cannot decompose; use Synthesis');
    for (const child of decomposition.get(key)) {
      const n = nodes.get(child);
      if (!n?.source || !targets(n, n.meta.get('Parent question')).has(key)) fail(key, `Decomposes into ${child} is not reciprocal`);
    }
    for (const row of node.obligations) {
      const owners = row[4] === 'Self' ? new Set([key]) : targets(node, row[4]);
      if (owners.size !== 1 || [...owners].some((o) => !nodes.get(o)?.source)) fail(key, `${row[0]} requires one owning experiment`);
      if (node.meta.get('Record kind') === 'Leaf' && !owners.has(key)) fail(key, `${row[0]} Leaf claim must be self-owned`);
      if (node.meta.get('Record kind') === 'Synthesis') for (const owner of owners) {
        if (owner === key || !decomposition.get(key).has(owner)) fail(key, `${row[0]} owner must be a declared child`);
        const binding = new RegExp(`\\]\\([^)]*${node.id}\\.md(?:#[^)]*)?\\)\\s+${row[0]}\\b`);
        if (!binding.test(nodes.get(owner)?.rel.get('Satisfies obligation') ?? '')) fail(key, `${row[0]} child ${owner} does not satisfy obligation`);
      }
    }
    const priorIds = [...(node.meta.get('Former IDs') ?? '').matchAll(/(?:([a-z][a-z0-9-]*)\/)?(EXP-\d{4})(?:@([a-f0-9]{40}))?/g)].map((m) => `${m[1] ?? node.track}/${m[2]}${m[3] ? `@${m[3]}` : ''}`);
    for (const prior of priorIds) {
      if (former.has(prior) && former.get(prior) !== key) fail(key, `ambiguous Former ID ${prior}`);
      if (!prior.includes('@') && globalIds.has(prior.split('/')[1])) fail(key, `Former ID reused as canonical without baseline qualification: ${prior}`);
      if (!prior.includes('@') && [...former.keys()].some((id) => !id.includes('@') && id.split('/')[1] === prior.split('/')[1] && id !== prior)) fail(key, `ambiguous global Former ID ${prior}`);
      former.set(prior, key);
    }
    const consumers = targets(node, node.rel.get('Produces input for'));
    if (node.meta.get('Record kind') === 'Leaf' && consumers.size > 1) {
      warnings.push(`${key}: reused by ${consumers.size} downstream decisions; decomposition review required`);
      if (/^None\b/.test(note(section(node.source, 'Proof Obligations'), 'Decomposition review'))) fail(key, 'multiple consumers without decomposition review');
    }
  }
  function acyclic(graph, label) {
    const done = new Set(), active = new Set();
    function visit(key, chain) {
      if (active.has(key)) { fail(label, `cycle ${[...chain, key].join(' -> ')}`); return; }
      if (done.has(key)) return;
      active.add(key);
      for (const next of graph.get(key) ?? []) visit(next, [...chain, key]);
      active.delete(key); done.add(key);
    }
    for (const key of graph.keys()) visit(key, []);
  }
  acyclic(hard, 'hard dependency DAG'); acyclic(decomposition, 'decomposition graph');

  const manifests = walk(path.join(skillDir, 'migrations')).filter((p) => p.endsWith('.json')), mappings = new Map();
  for (const file of manifests) {
    const manifest = JSON.parse(fs.readFileSync(file, 'utf8'));
    const baselineFiles = new Map();
    for (const item of manifest.renumberings ?? []) {
      mappings.set(item.former, item.current);
      if (former.get(item.former) !== item.current) fail(file, `renumbering ${item.former} lacks Former IDs`);
      if (item.former.includes('@') && item.former.split('@')[1] !== manifest.baseline_commit) fail(file, `Former ID baseline qualifier mismatch: ${item.former}`);
    }
    for (const item of manifest.files ?? []) {
      const m = item.path.match(/\/tracks\/([^/]+)\/(EXP-\d{4})\.md$/), key = m ? `${m[1]}/${m[2]}` : null;
      const renamed = (manifest.renumberings ?? []).find((r) => r.former.split('@')[0] === key)?.current;
      if (key && sealed.has(item.status) && (!nodes.has(key) || renamed)) fail(file, `sealed ID renumbered or removed: ${key}`);
      if (key && renamed && !provisional.has(item.status)) fail(file, `non-provisional ID renumbered: ${key}`);
      try {
        const bytes = gitRead ? gitRead(manifest.baseline_commit, item.path) : execFileSync('git', ['show', `${manifest.baseline_commit}:${item.path}`], { cwd: root, maxBuffer: 32 * 1024 * 1024 });
        baselineFiles.set(item.path, bytes.toString());
        if (hash(bytes) !== item.sha256 || bytes.length !== item.bytes) fail(file, `baseline identity mismatch: ${item.path}`);
        if (key && new Map(fields(bytes.toString())).get('Status') !== item.status) fail(file, `baseline status mismatch: ${item.path}`);
        if (key && renamed && !gitRead) {
          const decisions = execFileSync('git', ['log', manifest.baseline_commit, '--format=%H', '-G', '^\\| Status \\| (Accepted|Rejected|Inconclusive|Superseded|Invalidated) \\|', '--', item.path], { cwd: root }).toString().trim();
          if (decisions) fail(file, `historically sealed ID renumbered: ${key}`);
        }
      } catch { fail(file, `unavailable Git baseline: ${item.path}`); }
      const current = renamed ? path.join(skillDir, 'tracks', `${renamed}.md`) : path.join(root, item.path);
      if (fs.existsSync(current) && (fs.statSync(current).mode & 0o777).toString(8).padStart(4, '0') !== item.mode) fail(file, `mode changed: ${item.path}`);
    }
    const coverage = new Map(), primaryRows = new Map();
    // Navigation may change; literal measurements, commands, hashes and conclusions may not.
    const evidenceLine = (line) => line.trim()
      .replace(/EXP-\d{4}(?:\s*[–—/-]\s*(?:EXP-)?\d{4})*(?:\.md(?:#[^\s)]*)?)?/g, 'EXP-NAV');
    for (const item of manifest.extractions ?? []) {
      const source = baselineFiles.get(item.source);
      if (source === undefined) { fail(file, `extraction lacks captured baseline: ${item.source}`); continue; }
      const lines = source.match(/[^\n]*\n|[^\n]+$/g) ?? [];
      const raw = lines.slice(item.start_line - 1, item.end_line).join('');
      if (hash(raw) !== item.sha256) fail(file, `extraction source identity mismatch: ${item.source}:${item.start_line}`);
      const owned = item.retained_source_lines ?? Array.from({ length: item.end_line - item.start_line + 1 }, (_, i) => item.start_line + i);
      if (!coverage.has(item.source)) coverage.set(item.source, new Set());
      for (const line of owned) coverage.get(item.source).add(line);
      if (!item.owner) {
        if (item.classification !== 'compact-provenance' || !item.reason) fail(file, 'unowned extraction requires explicit compact regression/composition provenance');
        if (/^\|[^\n]*\b(?:RefTime|ProofSize|Time sample|DB sample)\b[^\n]*\|\n\|\s*---/m.test(raw)) fail(file, 'primary benchmark table cannot be discarded as compact chronology');
        continue;
      }
      const node = nodes.get(item.owner);
      if (node?.meta.get('Record kind') !== 'Leaf') { fail(file, `primary extraction owner is not Leaf: ${item.owner}`); continue; }
      const marker = item.marker.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      const payloads = [...node.source.matchAll(new RegExp(`${marker}\\n([\\s\\S]*?)\\n<!-- extracted:end -->`, 'g'))];
      if (payloads.length !== 1) { fail(file, `extraction marker must occur once: ${item.marker}`); continue; }
      const payload = payloads[0][1];
      if (hash(payload) !== item.payload_sha256) fail(file, `relocated payload identity mismatch: ${item.marker}`);
      const retained = new Set(payload.split('\n').map(evidenceLine));
      for (const line of owned) {
        const originalLine = lines[line - 1]?.trim() ?? '';
        if (!originalLine || originalLine.startsWith('#')) continue;
        if (!retained.has(evidenceLine(originalLine))) fail(file, `extracted decision evidence changed or missing: ${item.source}:${line} -> ${item.owner}`);
        if (originalLine.startsWith('|') && !originalLine.startsWith('| ---') && !['shared-context', 'partitioned-context'].includes(item.classification)) {
          const key = `${item.source}:${line}`;
          // Table headers can be shared; primary rows belong to exactly one Leaf.
          const next = lines[line]?.trim() ?? '';
          if (!next.startsWith('| ---')) {
            if (primaryRows.has(key) && primaryRows.get(key) !== item.owner) fail(file, `primary table row duplicated across leaves: ${key}`);
            primaryRows.set(key, item.owner);
          }
        }
      }
    }
    for (const [id, scope] of Object.entries(manifest.coverage ?? {})) {
      const sourcePath = [...baselineFiles.keys()].find((p) => p.endsWith(`/EXP-${id.padStart(4, '0')}.md`));
      if (!sourcePath) { fail(file, `coverage missing baseline ${id}`); continue; }
      const lines = baselineFiles.get(sourcePath).split('\n'), [start, end] = scope.measurement_and_derived_lines;
      for (let n = start; n <= end; n++) if (lines[n - 1]?.trim() && !lines[n - 1].startsWith('#') && !coverage.get(sourcePath)?.has(n)) fail(file, `unclassified pre-fission evidence: ${sourcePath}:${n}`);
    }
  }
  for (const [prior, current] of former) {
    if (mappings.get(prior) === current) continue;
    const [identity, baseline] = prior.split('@');
    if (!baseline) { fail(current, `Former ID ${prior} lacks migration baseline`); continue; }
    const sourcePath = path.relative(root, path.join(skillDir, 'tracks', `${identity}.md`));
    try {
      const bytes = gitRead ? gitRead(baseline, sourcePath) : execFileSync('git', ['show', `${baseline}:${sourcePath}`], { cwd: root, maxBuffer: 32 * 1024 * 1024 });
      if (!provisional.has(new Map(fields(bytes.toString())).get('Status'))) fail(current, `non-provisional ID renumbered: ${identity}`);
      if (!gitRead) {
        const decisions = execFileSync('git', ['log', baseline, '--format=%H', '-G', '^\\| Status \\| (Accepted|Rejected|Inconclusive|Superseded|Invalidated) \\|', '--', sourcePath], { cwd: root }).toString().trim();
        if (decisions) fail(current, `historically sealed ID renumbered: ${identity}`);
      }
    } catch { fail(current, `unavailable Git baseline for Former ID ${prior}`); }
  }
  if (former.size) {
    let allFiles = repoFiles;
    if (!allFiles) {
      try { allFiles = execFileSync('git', ['ls-files', '-z', '--cached', '--others', '--exclude-standard'], { cwd: root, maxBuffer: 32 * 1024 * 1024 }).toString().split('\0').filter(Boolean).map((p) => path.join(root, p)); }
      catch { fail(root, 'cannot enumerate former-ID references'); allFiles = []; }
    }
    for (const file of new Set(allFiles)) {
      if (manifests.includes(file) || !fs.existsSync(file) || !fs.statSync(file).isFile()) continue;
      const bytes = fs.readFileSync(file);
      if (bytes.includes(0)) continue;
      for (const [prior] of former) {
        const [basePrior, qualifier] = prior.split('@'), [track, id] = basePrior.split('/');
        const otherTrack = file.match(/\/tracks\/([^/]+)\//)?.[1];
        const suffix = qualifier && nodes.has(basePrior) ? `@${qualifier}\\b` : '\\b';
        const pattern = new RegExp(otherTrack && otherTrack !== track ? `${track}/${id}${suffix}` : `(?:${track}/)?${id}${suffix}`);
        for (const [i, line] of bytes.toString('utf8').split('\n').entries()) {
          if (pattern.test(line) && !/^\| Former IDs \|/.test(line) && !/\bformer(?:[- ]ID| provisional| identity| navigation| IDs?)?\b/i.test(line)) fail(file, `live former-ID ${prior} at line ${i + 1}`);
        }
      }
    }
  }
  // Evidence reuse, including Uses-evidence edges declared by a consumer, is a review trigger.
  for (const [key, node] of nodes) {
    if (node.meta.get('Record kind') !== 'Leaf') continue;
    const consumers = new Set(flow.filter(([from, , type]) => from === key && ['uses', 'produces'].includes(type)).map(([, to]) => to));
    if (consumers.size > 1 && /^None\b/.test(note(section(node.source, 'Proof Obligations'), 'Decomposition review'))) fail(key, 'multiple evidence consumers without decomposition review');
  }
  // Record-body links are part of the atomic migration, not merely Relations syntax.
  for (const node of nodes.values()) for (const m of node.source.matchAll(/\[[^\]]+\]\((\.{1,2}\/[^)]+)\)/g)) {
    const [relative, anchor] = m[1].split('#'), target = path.resolve(path.dirname(node.file), relative);
    if (!fs.existsSync(target)) { fail(node.key, `broken body link ${m[1]}`); continue; }
    if (anchor && target.endsWith('.md')) {
      const slugs = [...fs.readFileSync(target, 'utf8').matchAll(/^#{1,6} (.+)$/gm)].map((h) => h[1].toLowerCase().replace(/[^\p{L}\p{N}_\- ]/gu, '').replaceAll(' ', '-'));
      if (!slugs.includes(anchor)) fail(node.key, `broken body anchor ${m[1]}`);
    }
  }
  for (const [track, index] of indexes) {
    const campaign = index.source.match(/<!-- experiment-graph-campaign: (.*?) -->/)?.[1];
    if (!campaign && !graphPattern.test(index.source)) continue;
    if (!campaign) { fail(index.file, 'graph requires experiment-graph-campaign scope'); continue; }
    const selected = new Set([...nodes.values()].filter((n) => n.track === track && n.meta.get('Architecture release / experiment campaign') === campaign).map((n) => n.key));
    if (!selected.size) { fail(index.file, 'graph campaign has no records'); continue; }
    const graph = renderGraph(nodes, selected, hard, decomposition, flow);
    if (!graphPattern.test(index.source)) fail(index.file, 'missing graph markers');
    else if (!errors.length && writeIndex) indexWrites.set(index.file, index.source.replace(graphPattern, graph));
    else if (index.source.match(graphPattern)?.[0] !== graph) fail(index.file, 'stale graph projection; use --write-index');
  }
  if (!errors.length) for (const [file, source] of indexWrites) fs.writeFileSync(file, source);
  return { errors: [...new Set(errors)], warnings: [...new Set(warnings)], recordCount: [...nodes.values()].filter((n) => n.source).length };
}

function renderGraph(nodes, selected, hard, decomposition, flow) {
  const name = (key) => key.replaceAll(/[^a-zA-Z0-9]/g, '_');
  const lines = ['<!-- experiment-dependencies:start -->', 'Scope: the declared campaign and directly related external inputs/consumers. Decomposition means constituent proofs; hard dependencies mean required input; dotted evidence flow means scoped reuse, not ordering.'];
  const hardEdges = [...hard].flatMap(([to, inputs]) => [...inputs].map((from) => [from, to, 'requires']));
  const children = [...decomposition].flatMap(([from, outputs]) => [...outputs].map((to) => [from, to, 'contains']));
  for (const [title, edges, dotted] of [['Decomposition', children, false], ['Hard dependencies', hardEdges, false], ['Evidence flow', flow, true]]) {
    const filtered = [...new Map(edges.filter(([a, b]) => selected.has(a) || selected.has(b)).map((e) => [e.join('|'), e])).values()].sort((a, b) => a.join('|').localeCompare(b.join('|')));
    const keys = new Set(filtered.flatMap(([a, b]) => [a, b]));
    lines.push('', `**${title}**`, '', '```mermaid', 'flowchart TD');
    for (const key of [...keys].sort()) {
      const n = nodes.get(key), label = selected.has(key) ? `${n?.meta.get('Record kind')} / ${n?.meta.get('Status')}` : 'external';
      lines.push(`  ${name(key)}["${key}: ${label}"]`);
    }
    for (const [a, b, kind] of filtered) lines.push(`  ${name(a)} ${dotted ? `-. ${kind} .->` : '-->'} ${name(b)}`);
    if (!filtered.length) lines.push('  none["No declared edges"]');
    lines.push('```');
  }
  lines.push('', 'Conditional residual experiments remain unallocated until a measured owner and changed assumption earn a distinct proof question.', '<!-- experiment-dependencies:end -->');
  return lines.join('\n');
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [skillDir, mode] = process.argv.slice(2);
  if (mode === '--self-test') {
    execFileSync(process.execPath, [path.join(skillDir, 'scripts/record-normalization.test.mjs'), skillDir], { stdio: 'inherit' });
  } else {
    const result = validate(path.resolve(skillDir), { writeIndex: mode === '--write-index' });
    for (const w of result.warnings) console.error(`warning: ${w}`);
    for (const e of result.errors) console.error(`error: ${e}`);
    if (result.errors.length) process.exitCode = 1;
    else console.log(`Experiment normalization passed: ${result.recordCount} records; obligations, identities and three graph projections valid`);
  }
}
