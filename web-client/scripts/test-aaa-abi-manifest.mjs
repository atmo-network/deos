import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const manifest = JSON.parse(
  await readFile(
    new URL('../src/lib/automation/aaa-abi-manifest.json', import.meta.url),
    'utf8',
  ),
);

function graphNode(path) {
  return manifest.types.find((node) => node.path.join('::') === path);
}

test('metadata exposes the current eight-step execution-plan baseline', () => {
  const bound = manifest.pallet.constants.find(
    (constant) => constant.name === 'MaxExecutionPlanSteps',
  );
  assert(bound, 'MaxExecutionPlanSteps must remain public metadata');
  assert.equal(bound.value, '0x08000000');
});

test('metadata exposes the protocol-fixed retry-attempt bound', () => {
  const bound = manifest.pallet.constants.find(
    (constant) => constant.name === 'MaxRetryAttempts',
  );
  assert(bound, 'MaxRetryAttempts must remain public metadata');
  assert.equal(bound.value, '0x0a000000');
});

test('ActorClass carries the System custody locator independently of actor id', () => {
  const actorClass = graphNode('pallet_aaa::types::ActorClass');
  assert(
    actorClass,
    'ActorClass must remain reachable from public AAA metadata',
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
    (candidate) => candidate.name === 'create_system_aaa_at_sovereign_id',
  );
  assert(call, 'fresh System custody reattachment must remain public metadata');
  assert.equal(call.index, 3);
  assert.equal(call.fields[0]?.name, 'sovereign_id');
  assert(
    !manifest.pallet.calls.some(
      (candidate) => candidate.name === 'reopen_system_aaa',
    ),
  );
});

test('ProgramInput delegates one canonical ActiveProgramInput shape', () => {
  const programInput = graphNode('pallet_aaa::types::ProgramInput');
  const activeProgram = graphNode('pallet_aaa::types::ActiveProgramInput');
  assert(programInput, 'ProgramInput must remain public AAA metadata');
  assert(activeProgram, 'ActiveProgramInput must remain public AAA metadata');
  const active = programInput.def.value.find(
    (variant) => variant.name === 'Active',
  );
  assert.equal(active?.fields.length, 1);
  assert.equal(active?.fields[0]?.type, activeProgram.id);
  assert.deepEqual(
    activeProgram.def.value.map((field) => field.name),
    [
      'schedule',
      'schedule_window',
      'execution_plan',
      'completion_policy',
      'funding_source_policy',
      'auto_close_at_cycle_nonce',
    ],
  );
});

test('AaaCreated exposes ActorClass and InitialLifecycle without reconstruction', () => {
  const created = manifest.pallet.events.find(
    (event) => event.name === 'AaaCreated',
  );
  assert(created, 'AaaCreated must remain public metadata');
  const actorClass = created.fields.find(
    (field) => field.name === 'actor_class',
  );
  assert(actorClass, 'AaaCreated must expose ActorClass directly');
  const classType = manifest.types.find((node) => node.id === actorClass.type);
  assert.deepEqual(
    classType?.def.value.map((variant) => variant.name),
    ['User', 'System'],
    'ActorClass must distinguish User slots from System sovereign locators',
  );
  const lifecycle = created.fields.find(
    (field) => field.name === 'initial_lifecycle',
  );
  assert(lifecycle, 'AaaCreated must expose the initial lifecycle');
  const lifecycleType = manifest.types.find(
    (node) => node.id === lifecycle.type,
  );
  assert.deepEqual(
    lifecycleType?.def.value.map((variant) => variant.name),
    ['Dormant', 'Active'],
  );
  const ownerSlot = created.fields.find((field) => field.name === 'owner_slot');
  assert(!ownerSlot, 'AaaCreated must not reconstruct class from owner_slot');
  const aaaType = created.fields.find((field) => field.name === 'aaa_type');
  assert(!aaaType, 'AaaCreated must not carry a competing aaa_type authority');
});

test('CycleResult projection keeps terminal flow separate from factual counters', () => {
  const cycleResult = graphNode('pallet_aaa::types::CycleResult');
  assert(
    cycleResult,
    'CycleResult must remain reachable from public AAA metadata',
  );
  assert.equal(cycleResult.def.tag, 'variant');
  assert.deepEqual(
    cycleResult.def.value.map((variant) => variant.name),
    ['Completed', 'Failed', 'Cancelled'],
  );

  const summary = manifest.pallet.events.find(
    (event) => event.name === 'CycleSummary',
  );
  assert(summary, 'CycleSummary must remain a public AAA event');
  const fields = new Map(summary.fields.map((field) => [field.name, field]));
  assert.equal(fields.get('result')?.type, cycleResult.id);
  const outcomes = graphNode('pallet_aaa::types::OutcomeTotals');
  assert(outcomes, 'OutcomeTotals must remain reachable from CycleSummary');
  assert.equal(fields.get('outcomes')?.type, outcomes.id);
  const outcomeFields = new Set(outcomes.def.value.map((field) => field.name));
  for (const counter of [
    'executed_steps',
    'committed_effectful_tasks',
    'skipped_conditions',
    'skipped_resolution',
    'skipped_funding_unavailable',
    'failed_steps',
  ]) {
    assert(outcomeFields.has(counter), `${counter} must remain factual summary data`);
  }
});
