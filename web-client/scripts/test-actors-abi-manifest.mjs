import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const manifest = JSON.parse(
  await readFile(
    new URL('../src/lib/automation/actors-abi-manifest.json', import.meta.url),
    'utf8',
  ),
);

function graphNode(path) {
  const typeName = path.split('::').at(-1);
  return manifest.types.find(
    (node) =>
      node.path[0] === 'pallet_deos_actors' &&
      node.path[1] === 'types' &&
      node.path.at(-1) === typeName,
  );
}

test('metadata exposes the current thirty-two-step execution-plan baseline', () => {
  const bound = manifest.pallet.constants.find(
    (constant) => constant.name === 'MaxContractSteps',
  );
  assert(bound, 'MaxContractSteps must remain public metadata');
  assert.equal(bound.value, '0x20000000');
});

test('metadata exposes the protocol-fixed retry-attempt bound', () => {
  const bound = manifest.pallet.constants.find(
    (constant) => constant.name === 'MaxRetryAttempts',
  );
  assert(bound, 'MaxRetryAttempts must remain public metadata');
  assert.equal(bound.value, '0x0a000000');
});

test('ActorClass carries the System custody locator independently of actor id', () => {
  const actorClass = graphNode('pallet_deos_actors::types::ActorClass');
  assert(
    actorClass,
    'ActorClass must remain reachable from public Actors metadata',
  );
  assert.equal(actorClass.def.tag, 'variant');
  const system = actorClass.def.value.find(
    (variant) => variant.name === 'System',
  );
  assert(system, 'ActorClass must retain the System variant');
  assert.deepEqual(
    system.fields.map((field) => [field.name, field.typeName]),
    [['sovereign_id', 'SystemSovereignId']],
  );
});

test('fresh System reattachment accepts a custody locator rather than an actor id', () => {
  const call = manifest.pallet.calls.find(
    (candidate) => candidate.name === 'create_system_actor_at_sovereign_id',
  );
  assert(call, 'fresh System custody reattachment must remain public metadata');
  assert.equal(call.index, 3);
  assert.equal(call.fields[0]?.name, 'sovereign_id');
  assert(
    !manifest.pallet.calls.some(
      (candidate) => candidate.name === 'reopen_system_actor',
    ),
  );
});

test('ActorContract owns the complete authored shape', () => {
  const actorContract = graphNode('pallet_deos_actors::types::ActorContract');
  assert(actorContract, 'ActorContract must remain public Actors metadata');
  assert.equal(actorContract.def.tag, 'composite');
  assert.deepEqual(
    actorContract.def.value.map((field) => field.name),
    [
      'trigger',
      'cooldown_blocks',
      'window',
      'steps',
      'funding',
      'completion',
      'auto_close_at_cycle_nonce',
    ],
  );
});

test('ActorCreated exposes ActorClass and InitialLifecycle without reconstruction', () => {
  const created = manifest.pallet.events.find(
    (event) => event.name === 'ActorCreated',
  );
  assert(created, 'ActorCreated must remain public metadata');
  const actorClass = created.fields.find(
    (field) => field.name === 'actor_class',
  );
  assert(actorClass, 'ActorCreated must expose ActorClass directly');
  const classType = manifest.types.find((node) => node.id === actorClass.type);
  assert.deepEqual(
    classType?.def.value.map((variant) => variant.name),
    ['User', 'System'],
    'ActorClass must distinguish User slots from System sovereign locators',
  );
  const lifecycle = created.fields.find(
    (field) => field.name === 'initial_lifecycle',
  );
  assert(lifecycle, 'ActorCreated must expose the initial lifecycle');
  const lifecycleType = manifest.types.find(
    (node) => node.id === lifecycle.type,
  );
  assert.deepEqual(
    lifecycleType?.def.value.map((variant) => variant.name),
    ['Dormant', 'Active'],
  );
  const ownerSlot = created.fields.find((field) => field.name === 'owner_slot');
  assert(!ownerSlot, 'ActorCreated must not reconstruct class from owner_slot');
  const actorType = created.fields.find((field) => field.name === 'actor_type');
  assert(
    !actorType,
    'ActorCreated must not carry a competing actor_type authority',
  );
});

test('CycleResult projection keeps terminal flow separate from factual counters', () => {
  const cycleResult = graphNode('pallet_deos_actors::types::CycleResult');
  assert(
    cycleResult,
    'CycleResult must remain reachable from public Actors metadata',
  );
  assert.equal(cycleResult.def.tag, 'variant');
  assert.deepEqual(
    cycleResult.def.value.map((variant) => variant.name),
    ['Completed', 'Failed', 'Cancelled'],
  );

  const summary = manifest.pallet.events.find(
    (event) => event.name === 'CycleSummary',
  );
  assert(summary, 'CycleSummary must remain a public Actors event');
  const fields = new Map(summary.fields.map((field) => [field.name, field]));
  assert.equal(fields.get('result')?.type, cycleResult.id);
  const outcomes = graphNode('pallet_deos_actors::types::OutcomeTotals');
  assert(outcomes, 'OutcomeTotals must remain reachable from CycleSummary');
  assert.equal(fields.get('outcomes')?.type, outcomes.id);
  const outcomeFields = new Set(outcomes.def.value.map((field) => field.name));
  for (const counter of [
    'executed_steps',
    'committed_effectful_tasks',
    'precondition_skips',
    'skipped_resolution',
    'skipped_funding_unavailable',
    'failed_steps',
  ]) {
    assert(
      outcomeFields.has(counter),
      `${counter} must remain factual summary data`,
    );
  }
});
