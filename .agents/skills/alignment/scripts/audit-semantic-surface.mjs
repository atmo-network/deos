#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(import.meta.url);
const projectRoot = resolve(dirname(scriptPath), '../../../..');
const SOURCE_ROOTS = [
  ...['governance', 'staking', 'actors', 'router', 'oracle'].map((domain) => ({
    path: `template/pallets/${domain}/src`,
    language: 'rust',
    domain,
    exclusions: ['benchmarking.rs', 'mock.rs', 'tests.rs', 'weights.rs', 'tests/'],
  })),
  {
    path: 'template/runtime/src',
    language: 'rust',
    domain: 'runtime',
    exclusions: ['benchmarking.rs', 'tests.rs', 'weights/', 'tests/'],
  },
  {
    path: 'web-client/src/lib',
    language: 'client',
    domain: 'client',
    exclusions: ['*.d.ts', '*.generated.ts', '*.spec.ts', '*.test.ts', '/__tests__/'],
  },
];
const LIMITATIONS = [
  'Discovery records typed result expressions exactly as authored; it does not claim symbol resolution or universal call-graph reachability.',
  'Typed witnesses are explicit reviewed compiler/test anchors, not inferred constructor counts.',
  'Patterns, classifiers, conversions, and constructors are not inferred from textual variant references.',
  'No closed-world semantic-duplication count is claimed.',
];

function usage() {
  console.log(`Usage: audit-semantic-surface.mjs [OPTIONS]\n\nFail-closed discovery guard and explicit typed-witness checker for DEOS Error Narrowness evidence.\n\nOptions:\n  --check PATH       Verify PATH against the current recursively discovered source closure\n  --run-witnesses    Execute every declared witness command after identity validation\n  --inventory        Print a candidate manifest with no typed witnesses\n  -h, --help         Show this help\n`);
}

function parseArgs(argv) {
  const options = { check: null, inventory: false, runWitnesses: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === '-h' || argument === '--help') {
      usage();
      process.exit(0);
    }
    if (argument === '--check') {
      if (!argv[index + 1]) throw new Error('--check requires a path');
      options.check = argv[index + 1];
      index += 1;
      continue;
    }
    if (argument === '--inventory') {
      options.inventory = true;
      continue;
    }
    if (argument === '--run-witnesses') {
      options.runWitnesses = true;
      continue;
    }
    throw new Error(`unknown argument: ${argument}`);
  }
  if (Number(Boolean(options.check)) + Number(options.inventory) !== 1) {
    throw new Error('select exactly one of --check PATH or --inventory');
  }
  if (options.inventory && options.runWitnesses) {
    throw new Error('--run-witnesses requires --check PATH');
  }
  return options;
}

function git(args) {
  return execFileSync('git', ['-C', projectRoot, ...args], { encoding: 'utf8' }).trim();
}

function sha256(value) {
  return createHash('sha256').update(value).digest('hex');
}

function stableJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function walk(directory) {
  const results = [];
  for (const name of readdirSync(directory).sort()) {
    const path = join(directory, name);
    if (statSync(path).isDirectory()) results.push(...walk(path));
    else results.push(relative(projectRoot, path).replaceAll('\\', '/'));
  }
  return results;
}

function sourceKind(path, language) {
  if (language === 'rust') return path.endsWith('.rs');
  return path.endsWith('.ts') || path.endsWith('.svelte');
}

function exclusionReason(path, root) {
  const relativePath = path.slice(root.path.length + 1);
  const basename = relativePath.split('/').at(-1);
  for (const rule of root.exclusions) {
    if (rule.endsWith('/') && (relativePath.startsWith(rule) || relativePath.includes(`/${rule}`))) return rule;
    if (rule.startsWith('*.') && basename.endsWith(rule.slice(1))) return rule;
    if (rule.startsWith('/') && relativePath.includes(rule)) return rule;
    if (basename === rule) return rule;
  }
  return null;
}

function stripRustNoise(source) {
  let output = '';
  let index = 0;
  let blockDepth = 0;
  let state = 'code';
  while (index < source.length) {
    const current = source[index];
    const next = source[index + 1];
    if (state === 'line') {
      if (current === '\n') { state = 'code'; output += '\n'; } else output += ' ';
      index += 1;
      continue;
    }
    if (state === 'block') {
      if (current === '/' && next === '*') { blockDepth += 1; output += '  '; index += 2; continue; }
      if (current === '*' && next === '/') { blockDepth -= 1; output += '  '; index += 2; if (blockDepth === 0) state = 'code'; continue; }
      output += current === '\n' ? '\n' : ' ';
      index += 1;
      continue;
    }
    if (state === 'string' || state === 'char') {
      const terminator = state === 'string' ? '"' : "'";
      if (current === '\\') { output += '  '; index += 2; continue; }
      output += current === '\n' ? '\n' : ' ';
      index += 1;
      if (current === terminator) state = 'code';
      continue;
    }
    if (current === '/' && next === '/') { state = 'line'; output += '  '; index += 2; continue; }
    if (current === '/' && next === '*') { state = 'block'; blockDepth = 1; output += '  '; index += 2; continue; }
    if (current === '"') { state = 'string'; output += ' '; index += 1; continue; }
    if (current === "'" && /[^A-Za-z0-9_]/.test(next ?? '')) { state = 'char'; output += ' '; index += 1; continue; }
    output += current;
    index += 1;
  }
  if (state === 'block' || state === 'string' || state === 'char') throw new Error('unterminated Rust comment or literal');
  return output;
}

function matchingBrace(source, opening) {
  let depth = 0;
  for (let index = opening; index < source.length; index += 1) {
    if (source[index] === '{') depth += 1;
    if (source[index] === '}') {
      depth -= 1;
      if (depth === 0) return index;
    }
  }
  throw new Error('unbalanced Rust braces');
}

function functionRecords(source) {
  const records = [];
  const pattern = /\b(pub(?:\([^)]*\))?\s+)?fn\s+([a-zA-Z_][A-Za-z0-9_]*)\s*(?:<[^;{}]*>)?\s*\(/g;
  for (const match of source.matchAll(pattern)) {
    const opening = source.indexOf('(', match.index + match[0].length - 1);
    let depth = 0;
    let closing = -1;
    for (let index = opening; index < source.length; index += 1) {
      if (source[index] === '(') depth += 1;
      if (source[index] === ')' && --depth === 0) { closing = index; break; }
    }
    if (closing < 0) throw new Error(`unbalanced function parameters for ${match[2]}`);
    const tail = source.slice(closing + 1);
    const terminatorMatch = tail.match(/^[\s\S]*?(?=\{|;)/);
    if (!terminatorMatch) continue;
    const terminator = closing + 1 + terminatorMatch[0].length;
    const bodyOpening = source[terminator] === '{' ? terminator : -1;
    const end = bodyOpening >= 0 ? matchingBrace(source, bodyOpening) : terminator;
    const returnMatch = source.slice(closing + 1, terminator).match(/->\s*([\s\S]*?)\s*$/);
    records.push({
      name: match[2],
      visibility: match[1]?.replace(/\s+/g, '') ?? '',
      start: match.index,
      end,
      bodyOpening,
      resultType: (returnMatch?.[1] ?? '()').replace(/\s+/g, ' ').trim(),
    });
  }
  return records;
}

function enclosingOwner(source, record) {
  const owners = [];
  const pattern = /\b(pub\s+trait\s+[A-Z][A-Za-z0-9_]*[^;{]*|impl(?:\s*<[^{};]*>)?\s+[^{};]+)\{/g;
  for (const match of source.matchAll(pattern)) {
    const opening = source.indexOf('{', match.index);
    const closing = matchingBrace(source, opening);
    if (opening < record.start && record.start < closing) {
      owners.push({ opening, text: match[1].replace(/\s+/g, ' ').trim() });
    }
  }
  return owners.sort((left, right) => right.opening - left.opening)[0]?.text ?? '';
}

function isResultExpression(resultType) {
  return /\b(?:Result|DispatchResult|DispatchResultWithPostInfo)\b/.test(resultType);
}

function rustBoundaries(path, source, domain) {
  const clean = stripRustNoise(source);
  const boundaries = [];
  for (const record of functionRecords(clean)) {
    if (!isResultExpression(record.resultType)) continue;
    const owner = enclosingOwner(clean, record);
    const inPublicTrait = owner.startsWith('pub trait ');
    const inRuntimeTraitImpl = domain === 'runtime' && owner.startsWith('impl ') && owner.includes(' for ');
    if (!record.visibility && !inPublicTrait && !inRuntimeTraitImpl) continue;
    const symbol = owner ? `${owner}::${record.name}` : record.name;
    boundaries.push({
      id: `${path}#${symbol}`,
      domain,
      language: 'rust',
      resultTypeExpression: record.resultType,
    });
  }
  return boundaries;
}

function clientBoundaries(path, source) {
  const boundaries = [];
  const functionPattern = /\bexport\s+(?:async\s+)?function\s+([A-Za-z_][A-Za-z0-9_]*)\s*\([^)]*\)\s*(?::\s*([^\n{]+))?/g;
  for (const match of source.matchAll(functionPattern)) {
    boundaries.push({
      id: `${path}#${match[1]}`,
      domain: 'client',
      language: 'client',
      resultTypeExpression: (match[2] ?? 'inferred').replace(/\s+/g, ' ').trim(),
    });
  }
  const arrowPattern = /\bexport\s+const\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*([^=\n]+))?=\s*(?:async\s*)?\([^)]*\)\s*=>/g;
  for (const match of source.matchAll(arrowPattern)) {
    boundaries.push({
      id: `${path}#${match[1]}`,
      domain: 'client',
      language: 'client',
      resultTypeExpression: (match[2] ?? 'inferred').replace(/\s+/g, ' ').trim(),
    });
  }
  return boundaries;
}

function discover() {
  const includedFiles = [];
  const excludedFiles = [];
  const boundaries = [];
  for (const root of SOURCE_ROOTS) {
    const absoluteRoot = join(projectRoot, root.path);
    if (!existsSync(absoluteRoot)) throw new Error(`source root is missing: ${root.path}`);
    for (const path of walk(absoluteRoot).filter((candidate) => sourceKind(candidate, root.language))) {
      const reason = exclusionReason(path, root);
      if (reason) {
        excludedFiles.push({ path, reason: `root rule ${reason}` });
        continue;
      }
      const source = readFileSync(join(projectRoot, path), 'utf8');
      includedFiles.push({ path, sha256: sha256(source) });
      boundaries.push(...(root.language === 'rust'
        ? rustBoundaries(path, source, root.domain)
        : clientBoundaries(path, source)));
    }
  }
  includedFiles.sort((left, right) => left.path.localeCompare(right.path));
  excludedFiles.sort((left, right) => left.path.localeCompare(right.path));
  boundaries.sort((left, right) => left.id.localeCompare(right.id));
  const duplicateBoundary = boundaries.find((boundary, index) => boundaries[index - 1]?.id === boundary.id);
  if (duplicateBoundary) throw new Error(`duplicate discovered boundary: ${duplicateBoundary.id}`);
  const resultTypeExpressions = [...new Set(boundaries.map(({ resultTypeExpression }) => resultTypeExpression))].sort();
  return {
    sourceRoots: SOURCE_ROOTS,
    includedFiles,
    excludedFiles,
    boundaries,
    resultTypeExpressions,
    identities: {
      pathManifest: sha256(stableJson({ included: includedFiles.map(({ path }) => path), excluded: excludedFiles })),
      sourceClosure: sha256(includedFiles.map(({ path, sha256: hash }) => `${path}\0${hash}`).join('\n')),
      boundaries: sha256(stableJson(boundaries)),
      resultTypeExpressions: sha256(resultTypeExpressions.join('\0')),
    },
  };
}

function validateWitnesses(witnesses, discovery) {
  if (!Array.isArray(witnesses) || witnesses.length === 0) throw new Error('manifest must retain explicit typed witnesses');
  const ids = new Set();
  const commandIds = new Set();
  const discoveredBoundaries = new Set(discovery.boundaries.map(({ id }) => id));
  for (const witness of witnesses) {
    if (!witness.id || ids.has(witness.id)) throw new Error(`invalid or duplicate witness id: ${witness.id ?? '<missing>'}`);
    ids.add(witness.id);
    if (!['typed-signature', 'public-root-execution', 'exhaustive-classification', 'conversion-edge'].includes(witness.proofClass)) {
      throw new Error(`unknown proof class for ${witness.id}: ${witness.proofClass}`);
    }
    if (!Array.isArray(witness.boundaries) || witness.boundaries.length === 0) throw new Error(`${witness.id} has no discovered boundary`);
    for (const boundary of witness.boundaries) {
      if (!discoveredBoundaries.has(boundary)) throw new Error(`${witness.id} references undiscovered boundary: ${boundary}`);
    }
    if (!Array.isArray(witness.anchors) || witness.anchors.length === 0) throw new Error(`${witness.id} has no typed-source anchor`);
    const executableAnchorIds = new Set();
    for (const anchor of witness.anchors) {
      const absolute = join(projectRoot, anchor.path);
      if (!existsSync(absolute)) throw new Error(`${witness.id} anchor path is missing: ${anchor.path}`);
      const source = readFileSync(absolute, 'utf8');
      if (!source.includes(anchor.contains)) throw new Error(`${witness.id} anchor drifted: ${anchor.path} lacks ${JSON.stringify(anchor.contains)}`);
      if (!/^[a-f0-9]{64}$/.test(anchor.sha256 ?? '') || sha256(source) !== anchor.sha256) {
        throw new Error(`${witness.id} anchor file identity drifted: ${anchor.path}`);
      }
      if (!['source', 'executable'].includes(anchor.role)) throw new Error(`${witness.id} anchor has invalid role: ${anchor.path}`);
      if (anchor.role === 'executable') executableAnchorIds.add(`${anchor.path}#${anchor.contains}`);
    }
    if (!Array.isArray(witness.validationCommands) || witness.validationCommands.length === 0) {
      throw new Error(`${witness.id} has no compiler/test validation commands`);
    }
    const coveredExecutableAnchors = new Set();
    for (const command of witness.validationCommands) {
      if (!command.id || commandIds.has(command.id)) throw new Error(`invalid or duplicate witness command id: ${command.id ?? '<missing>'}`);
      commandIds.add(command.id);
      if (!command.program || !Array.isArray(command.args) || typeof command.cwd !== 'string') {
        throw new Error(`${command.id} is not an explicit executable command`);
      }
      if (!Array.isArray(command.executableAnchors) || command.executableAnchors.length === 0) {
        throw new Error(`${command.id} cites no executable anchor`);
      }
      for (const anchorId of command.executableAnchors) {
        if (!executableAnchorIds.has(anchorId)) throw new Error(`${command.id} cites an undeclared executable anchor: ${anchorId}`);
        coveredExecutableAnchors.add(anchorId);
      }
    }
    for (const anchorId of executableAnchorIds) {
      if (!coveredExecutableAnchors.has(anchorId)) throw new Error(`${witness.id} executable anchor has no command: ${anchorId}`);
    }
  }
}

function runWitnesses(witnesses) {
  for (const witness of witnesses) {
    for (const command of witness.validationCommands) {
      console.log(`Running Error Narrowness witness ${command.id}`);
      execFileSync(command.program, command.args, {
        cwd: resolve(projectRoot, command.cwd),
        env: { ...process.env, ...(command.env ?? {}) },
        stdio: 'inherit',
      });
    }
  }
}

function report(discovery, typedWitnesses) {
  return {
    schema: 'deos-error-narrowness-evidence/v2',
    authority: {
      validatorSha256: sha256(readFileSync(scriptPath)),
      candidateCommit: git(['rev-parse', 'HEAD^{commit}']),
      dirtyWorktree: git(['status', '--porcelain', '--untracked-files=all']).length > 0,
    },
    definitions: {
      discoveryGuard: 'Recursive checked source closure plus exact typed result expressions; any source, path, public-result boundary, or exported client function drift requires reviewed manifest refresh.',
      typedWitness: 'An explicit discovered-boundary binding to exact source/test file hashes and commands covering every cited executable anchor; proof scope is only the named witness.',
    },
    limitations: LIMITATIONS,
    discovery: {
      sourceRoots: discovery.sourceRoots,
      counts: {
        includedFiles: discovery.includedFiles.length,
        excludedFiles: discovery.excludedFiles.length,
        checkedBoundarySignatures: discovery.boundaries.length,
        resultTypeExpressions: discovery.resultTypeExpressions.length,
      },
      identities: discovery.identities,
    },
    typedWitnesses,
    witnessIdentity: sha256(stableJson(typedWitnesses)),
    closure: {
      universalVariantReachabilityProven: false,
      errorNarrownessCanCloseFromThisArtifactAlone: false,
      residual: 'Universal root-to-constructor reachability is intentionally unresolved; this artifact proves only its explicit typed witnesses and fails closed on unreviewed source or boundary drift.',
    },
  };
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const discovery = discover();
  if (options.inventory) {
    process.stdout.write(stableJson(report(discovery, [])));
    return;
  }
  const manifestPath = resolve(projectRoot, options.check);
  const expected = JSON.parse(readFileSync(manifestPath, 'utf8'));
  if (expected.schema !== 'deos-error-narrowness-evidence/v2') throw new Error(`unsupported manifest schema: ${expected.schema}`);
  validateWitnesses(expected.typedWitnesses, discovery);
  const actual = report(discovery, expected.typedWitnesses);
  if (stableJson(expected) !== stableJson(actual)) {
    throw new Error(`error-narrowness evidence drifted: ${relative(projectRoot, manifestPath)}`);
  }
  if (options.runWitnesses) runWitnesses(expected.typedWitnesses);
  console.log(`Error Narrowness evidence matches ${relative(projectRoot, manifestPath)} (${discovery.identities.sourceClosure})`);
}

try {
  main();
} catch (error) {
  console.error(`Error Narrowness audit failed: ${error.message}`);
  process.exit(1);
}
