/*
Domain: Actors normative surface drift gate validation
Owns: Proves the drift gate detects missing/extra variants, fields, and stale
shapes on synthetic spec/manifest fixtures, and passes on the aligned
specification surface.
Excludes: Runtime metadata generation, index pinning, release identity.
Zone: Web-client validation entrypoint.
*/
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const scriptSource = await readFile(
  new URL('../scripts/check-actors-normative-drift.mjs', import.meta.url),
  'utf8',
);

test('drift gate script exists and parses the canonical spec markers', () => {
  assert.match(scriptSource, /## 8\. Events and Ordering/);
  assert.match(scriptSource, /## 9\. ABI, Errors, Storage, and Upgrades/);
  assert.match(scriptSource, /### 9\.2 Errors/);
  assert.match(scriptSource, /'ContractInput'/);
  assert.match(scriptSource, /'Task'/);
  assert.match(scriptSource, /'Predicate'/);
  assert.match(scriptSource, /'AmountResolution'/);
  assert.match(scriptSource, /specCalls\(\)/);
  assert.match(scriptSource, /'Step'/);
  assert.match(scriptSource, /'Precondition'/);
  assert.match(
    scriptSource,
    /pallet_deos_actors::types::\$\{metadataEnumName\}/,
  );
  assert.match(scriptSource, /plural Preconditions compatibility type remains/);
  assert.match(scriptSource, /'ExecutionPlanOf'/);
  assert.match(scriptSource, /'MaxExecutionPlanSteps'/);
  assert.match(scriptSource, /duplicate specification variants/);
  assert.match(scriptSource, /duplicate metadata variants/);
  assert.match(scriptSource, /section reference: missing Section/);
  assert.match(scriptSource, /terminology: stale term/);
  assert.match(scriptSource, /entry\.path\?\.includes\('Preconditions'\)/);
  assert.doesNotMatch(scriptSource, /entry\.id === 238/);
  assert.doesNotMatch(scriptSource, /specCleanupExclusions/);
});

test('drift gate declares the required runtime constants surface', () => {
  assert.match(scriptSource, /MaxContractSteps/);
  assert.match(scriptSource, /MaxOwnerSlots/);
  assert.match(scriptSource, /MaxRetryAttempts/);
  assert.match(scriptSource, /MinUserBalance/);
  assert.match(scriptSource, /MinWindowLength/);
  assert.match(scriptSource, /MaxExecutionDelayBlocks/);
});

test('drift gate passes on the aligned surface and fails closed on drift', async () => {
  // The current metadata-derived public surface must match the accepted
  // specification candidate before implementation begins.
  const { spawn } = await import('node:child_process');
  const { mkdtemp, mkdir, writeFile, rm } = await import('node:fs/promises');
  const { tmpdir } = await import('node:os');
  const { join } = await import('node:path');
  const run = (args, cwd) =>
    new Promise((resolve) => {
      const child = spawn('node', args, {
        cwd,
        stdio: ['ignore', 'pipe', 'pipe'],
      });
      let output = '';
      child.stdout.on('data', (chunk) => (output += chunk));
      child.stderr.on('data', (chunk) => (output += chunk));
      child.on('close', (code) => resolve({ code, output }));
    });
  const aligned = await run(
    ['scripts/check-actors-normative-drift.mjs', '--spec-only'],
    new URL('..', import.meta.url),
  );
  assert.equal(aligned.code, 0, `aligned surface must pass: ${aligned.output}`);
  assert.match(aligned.output, /passed/);

  // A manifest missing a required constant must fail closed with exit code 1.
  // The mutated manifest lives in a temp sandbox so the live manifest is never
  // written mid-run (the release gate regenerates it concurrently).
  const sandbox = await mkdtemp(join(tmpdir(), 'actor-drift-'));
  try {
    const scriptsSrc = await readFile(
      new URL('../scripts/check-actors-normative-drift.mjs', import.meta.url),
      'utf8',
    );
    const rawManifestSource = await readFile(
      new URL(
        '../src/lib/automation/actors-abi-manifest.json',
        import.meta.url,
      ),
      'utf8',
    );
    const alignedManifest = JSON.parse(rawManifestSource);
    alignedManifest.pallet.events = alignedManifest.pallet.events.filter(
      (entry) => entry.name !== 'CycleDeferred',
    );
    const manifestSource = JSON.stringify(alignedManifest);
    const mutated = JSON.parse(manifestSource);
    mutated.pallet.constants = mutated.pallet.constants.filter(
      (entry) => entry.name !== 'MinUserBalance',
    );
    const webClient = join(sandbox, 'web-client');
    await mkdir(join(webClient, 'scripts'), { recursive: true });
    await mkdir(join(webClient, 'src/lib/automation'), { recursive: true });
    await mkdir(join(sandbox, 'template/pallets/actors/docs'), {
      recursive: true,
    });
    await mkdir(join(sandbox, '.agents/skills/alignment/rules'), {
      recursive: true,
    });
    await writeFile(
      join(webClient, 'scripts/check-actors-normative-drift.mjs'),
      scriptsSrc,
    );
    await writeFile(
      join(webClient, 'src/lib/automation/actors-abi-manifest.json'),
      JSON.stringify(mutated),
    );
    const semanticSource = await readFile(
      new URL(
        '../src/lib/automation/actors-semantic-manifest.json',
        import.meta.url,
      ),
      'utf8',
    );
    await writeFile(
      join(webClient, 'src/lib/automation/actors-semantic-manifest.json'),
      semanticSource,
    );
    const specSource = await readFile(
      new URL(
        '../../template/pallets/actors/docs/specification.en.md',
        import.meta.url,
      ),
      'utf8',
    );
    await writeFile(
      join(sandbox, 'template/pallets/actors/docs/specification.en.md'),
      specSource,
    );
    const ruleInventory = await readFile(
      new URL(
        '../../.agents/skills/alignment/rules/actors-identity-rules.json',
        import.meta.url,
      ),
      'utf8',
    );
    await writeFile(
      join(
        sandbox,
        '.agents/skills/alignment/rules/actors-identity-rules.json',
      ),
      ruleInventory,
    );
    const drifted = await run(
      ['scripts/check-actors-normative-drift.mjs'],
      webClient,
    );
    assert.equal(drifted.code, 1);
    assert.match(drifted.output, /MinUserBalance/);

    const manifestPath = join(
      webClient,
      'src/lib/automation/actors-abi-manifest.json',
    );
    const sandboxSpecPath = join(
      sandbox,
      'template/pallets/actors/docs/specification.en.md',
    );

    const missingCallManifest = JSON.parse(manifestSource);
    missingCallManifest.pallet.calls = missingCallManifest.pallet.calls.filter(
      (entry) => entry.name !== 'cancel_continuation',
    );
    await writeFile(manifestPath, JSON.stringify(missingCallManifest));
    await writeFile(sandboxSpecPath, specSource);
    const missingCall = await run(
      ['scripts/check-actors-normative-drift.mjs'],
      webClient,
    );
    assert.equal(missingCall.code, 1);
    assert.match(missingCall.output, /calls: missing: cancel_continuation/);

    await writeFile(manifestPath, manifestSource);
    await writeFile(
      sandboxSpecPath,
      specSource.replace(
        'struct Schedule<Sources> { trigger: TriggerPolicy<Sources>, cooldown_blocks: u32 }',
        'struct Schedule<Sources> { cooldown_blocks: u32, trigger: TriggerPolicy<Sources> }',
      ),
    );
    const structFieldDrift = await run(
      ['scripts/check-actors-normative-drift.mjs'],
      webClient,
    );
    assert.equal(structFieldDrift.code, 1);
    assert.match(structFieldDrift.output, /Schedule fields: ordered drift/);

    await writeFile(manifestPath, manifestSource);
    await writeFile(
      sandboxSpecPath,
      specSource.replace('Section 4.4.', 'Section 99.9.'),
    );
    const staleReference = await run(
      ['scripts/check-actors-normative-drift.mjs'],
      webClient,
    );
    assert.equal(staleReference.code, 1);
    assert.match(
      staleReference.output,
      /section reference: missing Section 99.9/,
    );

    await writeFile(
      sandboxSpecPath,
      `${specSource}\nStale implementation name: MaxSweepPerBlock.\n`,
    );
    const staleTerminology = await run(
      ['scripts/check-actors-normative-drift.mjs'],
      webClient,
    );
    assert.equal(staleTerminology.code, 1);
    assert.match(staleTerminology.output, /terminology: stale term/);

    const duplicateMetadataError = JSON.parse(manifestSource);
    duplicateMetadataError.pallet.errors.push({
      ...duplicateMetadataError.pallet.errors[0],
    });
    await writeFile(manifestPath, JSON.stringify(duplicateMetadataError));
    await writeFile(sandboxSpecPath, specSource);
    const duplicateMetadata = await run(
      ['scripts/check-actors-normative-drift.mjs'],
      webClient,
    );
    assert.equal(duplicateMetadata.code, 1);
    assert.match(duplicateMetadata.output, /duplicate metadata variants/);

    await writeFile(manifestPath, manifestSource);
    await writeFile(
      sandboxSpecPath,
      specSource.replace(
        '  ActorIdOverflow,',
        '  ActorIdOverflow, ActorIdOverflow,',
      ),
    );
    const duplicateSpec = await run(
      ['scripts/check-actors-normative-drift.mjs'],
      webClient,
    );
    assert.equal(duplicateSpec.code, 1);
    assert.match(duplicateSpec.output, /duplicate specification variants/);

    await writeFile(
      sandboxSpecPath,
      specSource.replace(
        'ActorCreated { actor_id: ActorId, owner: AccountId,',
        'ActorCreated { owner: AccountId, actor_id: ActorId,',
      ),
    );
    const fieldDrift = await run(
      ['scripts/check-actors-normative-drift.mjs'],
      webClient,
    );
    assert.equal(fieldDrift.code, 1);
    assert.match(fieldDrift.output, /ActorCreated fields: ordered drift/);

    await writeFile(
      sandboxSpecPath,
      specSource.replace(
        'Transfer { to: AccountId, asset: AssetId,',
        'Transfer { asset: AccountId, to: AssetId,',
      ),
    );
    const typeFieldDrift = await run(
      ['scripts/check-actors-normative-drift.mjs'],
      webClient,
    );
    assert.equal(typeFieldDrift.code, 1);
    assert.match(typeFieldDrift.output, /Task.Transfer fields: ordered drift/);

    await writeFile(
      sandboxSpecPath,
      specSource.replace(
        'ActorCreated { actor_id: ActorId, owner: AccountId, actor_class: ActorClass, mutability: Mutability, sovereign_account: AccountId, initial_lifecycle: InitialLifecycle }\nActorActivated { actor_id: ActorId }',
        'ActorActivated { actor_id: ActorId }\nActorCreated { actor_id: ActorId, owner: AccountId, actor_class: ActorClass, mutability: Mutability, sovereign_account: AccountId, initial_lifecycle: InitialLifecycle }',
      ),
    );
    const orderDrift = await run(
      ['scripts/check-actors-normative-drift.mjs'],
      webClient,
    );
    assert.equal(orderDrift.code, 1);
    assert.match(orderDrift.output, /events: ordered drift/);

    const renumberedPrecondition = JSON.parse(manifestSource);
    const preconditionType = renumberedPrecondition.types.find(
      (entry) =>
        entry.path?.join('::') === 'pallet_deos_actors::types::Precondition',
    );
    preconditionType.id = 999999;
    await writeFile(manifestPath, JSON.stringify(renumberedPrecondition));
    await writeFile(sandboxSpecPath, specSource);
    const renumbered = await run(
      ['scripts/check-actors-normative-drift.mjs'],
      webClient,
    );
    assert.equal(
      renumbered.code,
      0,
      `Precondition numeric id must not own identity: ${renumbered.output}`,
    );
  } finally {
    await rm(sandbox, { recursive: true, force: true });
  }
});
