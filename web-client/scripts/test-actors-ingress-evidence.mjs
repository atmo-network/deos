/*
Domain: Certified AddressEvent ingress validation
Owns: Binds the generated certified-producer inventory evidence to the typed
boundary contract and to the web-client automation surface.
Excludes: Runtime builds, metadata export, live chain access, and release identity.
Zone: Web-client validation entrypoint for ingress evidence.
*/
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

import { DEOS_INGRESS_RUNTIME_EVIDENCE } from '../src/lib/automation/ingress-runtime-evidence.generated.ts';

const projectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
  '..',
);

const EXPECTED_PRODUCER_IDS = [
  'AddressEventIngressExtension::signed_transfer',
  'AddressEventIngressExtension::transfer_all',
  'TmctolAssetOps::transfer',
  'TmctolAssetOps::mint',
  'TmctolMintDistributionIngress',
  'DeosRouter::route_fee',
  'XCM asset deposit',
  'XCM deposit without origin',
  'TmctolFeeCollector',
];

async function run() {
  const { runtime, inventorySha256, certifiedProducers, boundary } =
    DEOS_INGRESS_RUNTIME_EVIDENCE;
  assert.equal(runtime.specName, 'deos-runtime');
  assert.equal(runtime.transactionVersion, 1);
  assert.match(inventorySha256, /^[0-9a-f]{64}$/);
  assert.equal(
    certifiedProducers.length,
    EXPECTED_PRODUCER_IDS.length,
    'certified producer inventory must match the frozen set',
  );
  const ids = certifiedProducers.map((producer) => producer.id);
  assert.deepEqual(
    [...ids].sort(),
    [...EXPECTED_PRODUCER_IDS].sort(),
    'producer ids must match the frozen inventory',
  );
  assert.equal(new Set(ids).size, ids.length, 'producer ids must be unique');
  for (const producer of certifiedProducers) {
    for (const field of [
      'id',
      'creditedSurface',
      'sourceProvenance',
      'preflightOwner',
      'notifyOwner',
      'rollbackOwner',
      'weightOwner',
    ]) {
      assert.ok(
        typeof producer[field] === 'string' && producer[field].length > 0,
        `${producer.id} must carry a nonempty ${field}`,
      );
    }
  }
  assert.equal(boundary.typedTrait, 'pallet_deos_actors::AddressEventIngress');
  assert.equal(boundary.adapter, 'RuntimeAddressEventIngress');
  assert.equal(boundary.extension, 'AddressEventIngressExtension');
  // The extension is the sole signed-movement producer: its candidate preparation
  // must route through the typed boundary inside the owning adapter file.
  const ingressAdapter = await readFile(
    path.join(
      projectRoot,
      'template/runtime/src/configs/address_event_ingress.rs',
    ),
    'utf8',
  );
  assert.match(
    ingressAdapter,
    /impl\s+pallet_deos_actors::AddressEventIngress<[^>]+>\s+for RuntimeAddressEventIngress/,
  );
  assert.match(ingressAdapter, /crate::Actors::notify_ingress\(/);
  assert.match(ingressAdapter, /crate::Actors::preflight_ingress\(/);
  process.stdout.write('certified ingress inventory evidence is valid\n');
}

run().catch((error) => {
  process.stderr.write(`ingress-evidence: ${error.message}\n`);
  process.exitCode = 1;
});
