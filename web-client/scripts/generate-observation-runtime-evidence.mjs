/*
Domain: Typed observation inspection validation
Owns: Deterministic browser evidence generated from runtime, metadata, descriptors, and Actors weights.
Excludes: Runtime builds, metadata export, descriptor generation, live chain access, and release identity.
Zone: Web-client validation/generation entrypoint for observation evidence.
*/
import { blake2AsHex } from '@polkadot/util-crypto';
import { createHash } from 'node:crypto';
import { readFile, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';
import prettier from 'prettier';

const webClientRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
);
const projectRoot = path.resolve(webClientRoot, '..');
const paths = {
  metadata: path.join(webClientRoot, '.papi/metadata/deos.scale'),
  descriptors: path.join(webClientRoot, '.papi/descriptors/package.json'),
  runtime: path.join(projectRoot, 'template/runtime/src/lib.rs'),
  runtimeConfigs: path.join(projectRoot, 'template/runtime/src/configs'),
  actorConfig: path.join(
    projectRoot,
    'template/runtime/src/configs/actor_config.rs',
  ),
  oracleConfig: path.join(
    projectRoot,
    'template/runtime/src/configs/oracle_config.rs',
  ),
  actorWeights: path.join(
    projectRoot,
    'template/runtime/src/weights/pallet_deos_actors.rs',
  ),
  databaseWeights: path.join(
    projectRoot,
    'template/runtime/src/weights/rocksdb_weights.rs',
  ),
  runtimeCode: path.join(
    projectRoot,
    'template/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm',
  ),
  output: path.join(
    webClientRoot,
    'src/lib/observation/runtime-evidence.generated.ts',
  ),
};

function fail(message) {
  throw new Error(message);
}

function requireMatch(source, expression, label) {
  const match = source.match(expression);
  if (match == null) fail(`Cannot derive ${label}`);
  return match[1];
}

function rustInteger(source, name) {
  const raw = requireMatch(
    source,
    new RegExp(`(?:pub\\s+)?const\\s+${name}[^=]*=\\s*([0-9_]+)`),
    name,
  );
  const value = Number(raw.replaceAll('_', ''));
  if (!Number.isSafeInteger(value)) fail(`${name} exceeds safe integer range`);
  return value;
}

function runtimeIdentity(source) {
  const text = (name) =>
    requireMatch(
      source,
      new RegExp(`${name}:\\s*alloc::borrow::Cow::Borrowed\\("([^"]+)"\\)`),
      name,
    );
  const version = (name) =>
    Number(
      requireMatch(
        source,
        new RegExp(`${name}:\\s*([0-9_]+)`),
        name,
      ).replaceAll('_', ''),
    );
  return {
    specName: text('spec_name'),
    implName: text('impl_name'),
    authoringVersion: version('authoring_version'),
    specVersion: version('spec_version'),
    implVersion: version('impl_version'),
    systemVersion: version('system_version'),
    transactionVersion: version('transaction_version'),
  };
}

function runtimeDatabaseWeights(source) {
  const nanos = 1_000;
  const body = requireMatch(
    source,
    /RuntimeDbWeight\s*=\s*RuntimeDbWeight\s*\{([\s\S]*?)\n\s*\};/,
    'RocksDbWeight body',
  );
  const field = (name) =>
    Number(
      requireMatch(body, new RegExp(`${name}:\\s*([0-9_]+)`), name).replaceAll(
        '_',
        '',
      ),
    ) * nanos;
  return { read: field('read'), write: field('write') };
}

function weightParts(source, method, databaseWeights) {
  const body = requireMatch(
    source,
    new RegExp(
      `fn\\s+${method}\\(\\)\\s*->\\s*Weight\\s*\\{([\\s\\S]*?)\\n\\s*\\}`,
    ),
    `${method} weight`,
  );
  const benchmarkRefTime = Number(
    requireMatch(
      body,
      /Weight::from_parts\(([0-9_]+),\s*0\)/,
      `${method} RefTime`,
    ).replaceAll('_', ''),
  );
  const reads = Number(
    body.match(/DbWeight::get\(\)\.reads\(([0-9_]+)\)/)?.[1] ?? 0,
  );
  const writes = Number(
    body.match(/DbWeight::get\(\)\.writes\(([0-9_]+)\)/)?.[1] ?? 0,
  );
  return {
    refTime:
      benchmarkRefTime +
      reads * databaseWeights.read +
      writes * databaseWeights.write,
    proofSize: Number(
      requireMatch(
        body,
        /saturating_add\(Weight::from_parts\(0,\s*([0-9_]+)\)\)/,
        `${method} ProofSize`,
      ).replaceAll('_', ''),
    ),
  };
}

function maximumBlockWeight(source) {
  const match = source.match(
    /MAXIMUM_BLOCK_WEIGHT:\s*Weight\s*=\s*Weight::from_parts\(([0-9_]+),\s*([0-9_]+)\)/,
  );
  if (match == null) fail('Cannot derive MAXIMUM_BLOCK_WEIGHT');
  return {
    refTime: Number(match[1].replaceAll('_', '')),
    proofSize: Number(match[2].replaceAll('_', '')),
  };
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

async function rustSources(root) {
  const sources = [];
  for (const entry of await readdir(root, { withFileTypes: true })) {
    const entryPath = path.join(root, entry.name);
    if (entry.isDirectory()) sources.push(...(await rustSources(entryPath)));
    else if (entry.isFile() && entry.name.endsWith('.rs')) {
      sources.push({
        path: entryPath,
        source: await readFile(entryPath, 'utf8'),
      });
    }
  }
  return sources;
}

function certifiedObservationPublishers(source, runtimeConfigSources) {
  const body = requireMatch(
    source,
    /ACTORS_OBSERVATION_PUBLISHER_INVENTORY[^=]*=\s*&\[([^\]]*)\]/,
    'Actors observation publisher inventory',
  );
  const publishers = [...body.matchAll(/"([^"]+)"/g)].map((match) => match[1]);
  if (
    publishers.length === 0 ||
    new Set(publishers).size !== publishers.length
  ) {
    fail('Actors observation publisher inventory must be nonempty and unique');
  }
  const ingressCalls = runtimeConfigSources.filter(({ source: candidate }) =>
    /ObservationTransitionIngress<[^>]+>>::note_observation_transition\(/.test(
      candidate,
    ),
  );
  if (
    ingressCalls.length !== 1 ||
    path.basename(ingressCalls[0].path) !== 'oracle_config.rs'
  ) {
    fail(
      'Every runtime observation ingress call must have one Oracle-owned inventory entry',
    );
  }
  if (
    runtimeConfigSources.some(({ source: candidate }) =>
      /(?:crate::)?Actors::note_observation_changed\(/.test(candidate),
    )
  ) {
    fail(
      'Runtime configuration bypasses the typed observation ingress boundary',
    );
  }
  return publishers;
}

async function generatedSource(runtimeCodeHashFallback = null) {
  const [
    metadata,
    descriptorBytes,
    runtimeCode,
    runtime,
    actorConfig,
    oracleConfig,
    actorWeights,
    databaseWeightSource,
    runtimeConfigSources,
  ] = await Promise.all([
    readFile(paths.metadata),
    readFile(paths.descriptors),
    readFile(paths.runtimeCode).catch(() => null),
    readFile(paths.runtime, 'utf8'),
    readFile(paths.actorConfig, 'utf8'),
    readFile(paths.oracleConfig, 'utf8'),
    readFile(paths.actorWeights, 'utf8'),
    readFile(paths.databaseWeights, 'utf8'),
    rustSources(paths.runtimeConfigs),
  ]);
  const runtimeCodeHash =
    runtimeCode === null
      ? runtimeCodeHashFallback
      : blake2AsHex(runtimeCode, 256);
  if (runtimeCodeHash === null) {
    fail('Compact runtime Wasm is missing and no generated identity exists');
  }
  const descriptorPackage = JSON.parse(descriptorBytes.toString('utf8'));
  if (
    typeof descriptorPackage.version !== 'string' ||
    descriptorPackage.version.length === 0
  ) {
    fail('Descriptor package version is missing');
  }
  const databaseWeights = runtimeDatabaseWeights(databaseWeightSource);
  const maximumBlock = maximumBlockWeight(runtime);
  const fanoutWeightLimit = {
    refTime: Math.floor(maximumBlock.refTime / 5),
    proofSize: Math.floor(maximumBlock.proofSize / 5),
  };
  const fanoutBaseWeight = weightParts(
    actorWeights,
    'observation_fanout_base',
    databaseWeights,
  );
  const serviceUnitWeight = weightParts(
    actorWeights,
    'observation_fanout_page',
    databaseWeights,
  );
  const configuredUnits = rustInteger(
    actorConfig,
    'ActorMaxObservationFanoutPagesPerBlock',
  );
  const maxServiceUnitsPerBlock = Math.min(
    configuredUnits,
    Math.floor(
      (fanoutWeightLimit.refTime - fanoutBaseWeight.refTime) /
        serviceUnitWeight.refTime,
    ),
    Math.floor(
      (fanoutWeightLimit.proofSize - fanoutBaseWeight.proofSize) /
        serviceUnitWeight.proofSize,
    ),
  );
  if (maxServiceUnitsPerBlock < 1) {
    fail('Derived fanout budget admits no production service unit');
  }
  const evidence = {
    runtime: runtimeIdentity(runtime),
    runtimeCodeHash,
    metadataHash: blake2AsHex(metadata, 256),
    metadataSha256: sha256(metadata),
    descriptorIdentity: descriptorPackage.version,
    weightIdentity: `sha256:${sha256(Buffer.from(actorWeights))}`,
    certifiedPublishers: certifiedObservationPublishers(
      oracleConfig,
      runtimeConfigSources,
    ),
    fanout: {
      configuredServiceUnitsPerBlock: configuredUnits,
      fanoutWeightLimit,
      fanoutBaseWeight,
      serviceUnitWeight,
      maxServiceUnitsPerBlock,
      maxActiveDirtyFeeds: rustInteger(actorConfig, 'ActorMaxActiveActors'),
      maxSubscriberPagesPerFeed: Math.ceil(
        rustInteger(actorConfig, 'ActorMaxActiveActors') /
          rustInteger(actorConfig, 'ActorQueuePageSize'),
      ),
    },
  };
  const prettierConfig = (await prettier.resolveConfig(paths.output)) ?? {};
  return await prettier.format(
    `/*\nDomain: Typed observation inspection\nOwns: Generated expected runtime, metadata, descriptor, weight, and fanout evidence.\nExcludes: Live chain evidence, observation state, and estimate projection.\nZone: Generated observation domain evidence; regenerate through the owning script.\n*/\nexport const DEOS_OBSERVATION_RUNTIME_EVIDENCE = ${JSON.stringify(evidence, null, 2)} as const;\n`,
    { ...prettierConfig, filepath: paths.output },
  );
}

async function main() {
  const check = process.argv.slice(2);
  if (check.length === 1 && ['--help', '-h'].includes(check[0])) {
    process.stdout.write(
      'Usage: generate-observation-runtime-evidence.mjs [--check]\n\nGenerate the observation inspector evidence projection, or verify freshness.\n',
    );
    return;
  }
  if (check.length > 1 || (check.length === 1 && check[0] !== '--check')) {
    fail('Usage: generate-observation-runtime-evidence.mjs [--check]');
  }
  const current = await readFile(paths.output, 'utf8').catch(() => '');
  const currentRuntimeCodeHash =
    current.match(/runtimeCodeHash:\s*\n?\s*['"](0x[0-9a-f]{64})['"]/)?.[1] ??
    null;
  const expected = await generatedSource(
    check[0] === '--check' ? currentRuntimeCodeHash : null,
  );
  if (check[0] === '--check') {
    if (current !== expected)
      fail('Generated observation runtime evidence is stale');
    process.stdout.write('observation runtime evidence is current\n');
    return;
  }
  await writeFile(paths.output, expected);
  process.stdout.write(`${path.relative(projectRoot, paths.output)}\n`);
}

main().catch((error) => {
  process.stderr.write(`observation-runtime-evidence: ${error.message}\n`);
  process.exitCode = 1;
});
