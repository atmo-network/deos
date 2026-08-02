/*
Domain: Certified AddressEvent ingress validation
Owns: Deterministic browser evidence generated from the certified-producer inventory,
the typed AddressEventIngress boundary, and the runtime identity.
Excludes: Runtime builds, metadata export, descriptor generation, live chain access,
and release identity.
Zone: Web-client validation/generation entrypoint for ingress evidence.
*/
import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
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
  runtime: path.join(projectRoot, 'template/runtime/src/lib.rs'),
  ingressAdapter: path.join(
    projectRoot,
    'template/runtime/src/configs/address_event_ingress.rs',
  ),
  runtimeConfigs: path.join(projectRoot, 'template/runtime/src/configs'),
  output: path.join(
    webClientRoot,
    'src/lib/automation/ingress-runtime-evidence.generated.ts',
  ),
};

// Every runtime file that may invoke one of the provenance-specific certified
// ingress helpers. The inventory must name an owner for each; the generator
// rejects any helper call site outside this set.
const INGRESS_HELPER_FILES = new Set([
  'address_event_ingress.rs',
  'aaa_config.rs',
  'axial_router_config.rs',
  'tmc_config.rs',
  'xcm_config.rs',
]);

function fail(message) {
  throw new Error(message);
}

function requireMatch(source, expression, label) {
  const match = source.match(expression);
  if (match == null) fail(`Cannot derive ${label}`);
  return match[1];
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

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function certifiedProducers(source) {
  const body = requireMatch(
    source,
    /AAA_ADDRESS_EVENT_PRODUCER_INVENTORY[^=]*=\s*&\[([\s\S]*?)\n\];/,
    'AAA address-event producer inventory',
  );
  const entries = [
    ...body.matchAll(/AddressEventProducer\s*\{([\s\S]*?)\n\s*\},/g),
  ];
  if (entries.length === 0) {
    fail('AAA address-event producer inventory must be nonempty');
  }
  const field = (entry, name) => {
    const match = entry[1].match(new RegExp(`${name}:\\s*"([^"]+)"`));
    if (match == null || match[1].length === 0) {
      fail(`AAA producer entry missing nonempty ${name}`);
    }
    return match[1];
  };
  const producers = entries.map((entry) => ({
    id: field(entry, 'id'),
    creditedSurface: field(entry, 'credited_surface'),
    sourceProvenance: field(entry, 'source_provenance'),
    preflightOwner: field(entry, 'preflight_owner'),
    notifyOwner: field(entry, 'notify_owner'),
    rollbackOwner: field(entry, 'rollback_owner'),
    weightOwner: field(entry, 'weight_owner'),
  }));
  const ids = producers.map((producer) => producer.id);
  if (new Set(ids).size !== ids.length) {
    fail('AAA producer inventory ids must be unique');
  }
  return producers;
}

async function rustSources(root) {
  const { readdir } = await import('node:fs/promises');
  const sources = [];
  const walk = async (dir) => {
    for (const entry of await readdir(dir, { withFileTypes: true })) {
      const entryPath = path.join(dir, entry.name);
      if (entry.isDirectory()) await walk(entryPath);
      else if (entry.isFile() && entry.name.endsWith('.rs')) {
        sources.push({
          path: entryPath,
          source: await readFile(entryPath, 'utf8'),
        });
      }
    }
  };
  await walk(root);
  return sources;
}

function verifyBoundary(ingressSource, runtimeConfigSources, producers) {
  // The typed pallet boundary functions may only be invoked from the owning
  // runtime adapter file; any other movement path would bypass the certified
  // inventory authority.
  const typedCalls = runtimeConfigSources.filter(({ source: candidate }) =>
    /(?:crate::)?AAA::(?:preflight_ingress|notify_ingress)\(/.test(candidate),
  );
  const ownerFiles = new Set(
    typedCalls.map(({ path: candidatePath }) => path.basename(candidatePath)),
  );
  if (ownerFiles.size !== 1 || !ownerFiles.has('address_event_ingress.rs')) {
    fail(
      'Every typed ingress boundary call must live in the owning address_event_ingress.rs adapter',
    );
  }
  // The provenance-specific helper boundary may only be reached from files the
  // inventory names; a new crediting path must register before acceptance.
  const helperPattern =
    /RuntimeAddressEventIngress::(?:preflight_internal_inbound|on_internal_inbound|preflight_xcm_inbound|on_xcm_inbound|on_inbound_without_source)\(/;
  const helperCallFiles = new Set(
    runtimeConfigSources
      .filter(({ source: candidate }) => helperPattern.test(candidate))
      .map(({ path: candidatePath }) => path.basename(candidatePath)),
  );
  for (const file of helperCallFiles) {
    if (!INGRESS_HELPER_FILES.has(file)) {
      fail(`Ingress helper call in unregistered file: ${file}`);
    }
  }
  if (helperCallFiles.size === 0) {
    fail('No certified ingress helper call site found');
  }
  // The old resolved core must not be reached from runtime configuration: every
  // movement goes through the typed boundary.
  if (
    runtimeConfigSources.some(({ source: candidate }) =>
      /(?:crate::)?AAA::notify_address_event\(/.test(candidate),
    )
  ) {
    fail('Runtime configuration bypasses the typed ingress boundary');
  }
  if (
    !/impl\s+pallet_aaa::AddressEventIngress<[^>]+> for RuntimeAddressEventIngress/.test(
      ingressSource,
    )
  ) {
    fail('Runtime adapter must implement the typed AddressEventIngress trait');
  }
  void producers;
}

async function generatedSource() {
  const [runtime, ingressAdapter, runtimeConfigSources] = await Promise.all([
    readFile(paths.runtime, 'utf8'),
    readFile(paths.ingressAdapter, 'utf8'),
    rustSources(paths.runtimeConfigs),
  ]);
  const producers = certifiedProducers(ingressAdapter);
  verifyBoundary(ingressAdapter, runtimeConfigSources, producers);
  const evidence = {
    runtime: runtimeIdentity(runtime),
    inventorySha256: sha256(ingressAdapter),
    certifiedProducers: producers,
    boundary: {
      typedTrait: 'pallet_aaa::AddressEventIngress',
      adapter: 'RuntimeAddressEventIngress',
      extension: 'AddressEventIngressExtension',
      helperFiles: [...INGRESS_HELPER_FILES].sort(),
    },
  };
  const prettierConfig = (await prettier.resolveConfig(paths.output)) ?? {};
  return await prettier.format(
    `/*\nDomain: Certified AddressEvent ingress\nOwns: Generated expected certified-producer inventory and typed-boundary evidence.\nExcludes: Live chain evidence, observation state, and estimate projection.\nZone: Generated ingress domain evidence; regenerate through the owning script.\n*/\nexport const DEOS_INGRESS_RUNTIME_EVIDENCE = ${JSON.stringify(evidence, null, 2)} as const;\n`,
    { ...prettierConfig, filepath: paths.output },
  );
}

async function main() {
  const check = process.argv.slice(2);
  if (check.length === 1 && ['--help', '-h'].includes(check[0])) {
    process.stdout.write(
      'Usage: generate-ingress-runtime-evidence.mjs [--check]\n\nGenerate the certified-ingress evidence projection, or verify freshness.\n',
    );
    return;
  }
  if (check.length > 1 || (check.length === 1 && check[0] !== '--check')) {
    fail('Usage: generate-ingress-runtime-evidence.mjs [--check]');
  }
  const current = await readFile(paths.output, 'utf8').catch(() => '');
  const expected = await generatedSource();
  if (check[0] === '--check') {
    if (current !== expected) {
      fail('Generated ingress runtime evidence is stale');
    }
    process.stdout.write('ingress runtime evidence is current\n');
    return;
  }
  await writeFile(paths.output, expected);
  process.stdout.write(`${path.relative(projectRoot, paths.output)}\n`);
}

main().catch((error) => {
  process.stderr.write(`ingress-runtime-evidence: ${error.message}\n`);
  process.exitCode = 1;
});
