<!--
Domain: Automation step editor
Owns: One stable ordered Step row, condition controls, task parameters, error policy, and linear move/remove actions.
Excludes: Contract storage, artifact encoding, analysis, simulation, and runtime execution.
Zone: Automation presentation helper; composes typed authoring fields without successor selection.
-->
<script lang="ts">
  import { ArrowDown, ArrowUp, Plus, Trash2 } from '@lucide/svelte';

  import {
    type ActorAuthoringIssue,
    type ActorAuthoringStep,
    createActorAuthoringPredicate,
  } from '$lib/automation/authoring';
  import type { ActorContractType } from '$lib/automation/contract-artifact';
  import type { AutomationMutability } from '$lib/automation/types';
  import {
    Badge,
    Button,
    IconButton,
    Notice,
    NumberInput,
    SelectField,
  } from '$lib/ui';

  import AutomationConditionEditor from './AutomationConditionEditor.svelte';
  import AutomationTaskEditor from './AutomationTaskEditor.svelte';

  type Props = {
    step: ActorAuthoringStep;
    index: number;
    total: number;
    actorType: ActorContractType;
    mutability: AutomationMutability;
    compact?: boolean;
    issues?: ActorAuthoringIssue[];
    onMove: (direction: -1 | 1) => void;
    onRemove: () => void;
  };

  let {
    step = $bindable(),
    index,
    total,
    actorType,
    mutability,
    compact = false,
    issues = [],
    onMove,
    onRemove,
  }: Props = $props();

  function selectPreconditionMode(event: Event) {
    const type = (event.currentTarget as HTMLSelectElement).value as
      | 'Unconditional'
      | 'AnyOf';
    step = {
      ...step,
      preconditions:
        type === 'Unconditional'
          ? { type }
          : step.preconditions.type === 'Unconditional'
            ? {
                type,
                clauses: [
                  [
                    {
                      timing: 'Current',
                      predicate: createActorAuthoringPredicate('BalanceAbove'),
                    },
                  ],
                ],
              }
            : step.preconditions,
    };
  }

  function selectErrorPolicy(event: Event) {
    const type = (event.currentTarget as HTMLSelectElement).value as
      | 'AbortCycle'
      | 'ContinueNextStep'
      | 'RetryLater';
    step = {
      ...step,
      errorPolicy:
        type === 'RetryLater'
          ? {
              type,
              maxAttempts:
                step.errorPolicy.type === 'RetryLater'
                  ? step.errorPolicy.maxAttempts
                  : 3,
            }
          : { type },
    };
  }

  function predicateCount() {
    return step.preconditions.type === 'Unconditional'
      ? 0
      : step.preconditions.clauses.reduce(
          (total, clause) => total + clause.length,
          0,
        );
  }

  function addClause() {
    if (step.preconditions.type === 'Unconditional' || predicateCount() >= 4)
      return;
    step = {
      ...step,
      preconditions: {
        ...step.preconditions,
        clauses: [
          ...step.preconditions.clauses,
          [
            {
              timing: 'Current',
              predicate: createActorAuthoringPredicate('BalanceAbove'),
            },
          ],
        ],
      },
    };
  }

  function addPredicate(clauseIndex: number) {
    if (step.preconditions.type === 'Unconditional' || predicateCount() >= 4)
      return;
    step = {
      ...step,
      preconditions: {
        ...step.preconditions,
        clauses: step.preconditions.clauses.map((clause, candidate) =>
          candidate === clauseIndex
            ? [
                ...clause,
                {
                  timing: 'Current',
                  predicate: createActorAuthoringPredicate('BalanceAbove'),
                },
              ]
            : clause,
        ),
      },
    };
  }

  function setTiming(
    clauseIndex: number,
    predicateIndex: number,
    event: Event,
  ) {
    if (step.preconditions.type === 'Unconditional') return;
    const timing = (event.currentTarget as HTMLSelectElement).value as
      | 'Opening'
      | 'Current';
    step = {
      ...step,
      preconditions: {
        ...step.preconditions,
        clauses: step.preconditions.clauses.map((clause, candidateClause) =>
          candidateClause === clauseIndex
            ? clause.map((timed, candidatePredicate) =>
                candidatePredicate === predicateIndex
                  ? { ...timed, timing }
                  : timed,
              )
            : clause,
        ),
      },
    };
  }

  function removePredicate(clauseIndex: number, predicateIndex: number) {
    if (step.preconditions.type === 'Unconditional') return;
    const clauses = step.preconditions.clauses
      .map((clause, candidateClause) =>
        candidateClause === clauseIndex
          ? clause.filter((_, candidate) => candidate !== predicateIndex)
          : clause,
      )
      .filter((clause) => clause.length > 0);
    step = {
      ...step,
      preconditions:
        clauses.length === 0
          ? { type: 'Unconditional' }
          : { type: 'AnyOf', clauses },
    };
  }
</script>

<section
  aria-labelledby={`automation-step-${step.key}`}
  class="relative grid gap-3 overflow-hidden rounded-2xl border border-(--mono-border) bg-white p-3 pl-4 shadow-[0_2px_8px_rgba(44,50,30,0.04)]"
>
  <div class="absolute inset-y-0 left-0 w-1 bg-(--mono-purple)"></div>
  <header
    class={compact
      ? 'grid gap-2'
      : 'flex flex-wrap items-start justify-between gap-2'}
  >
    <div class="flex min-w-0 items-center gap-2">
      <div
        class="grid size-8 shrink-0 place-items-center rounded-full bg-(--mono-purple) text-xs font-semibold text-white tabnum"
        aria-hidden="true"
      >
        {String(index + 1).padStart(2, '0')}
      </div>
      <div class="min-w-0">
        <div
          id={`automation-step-${step.key}`}
          class="truncate text-sm font-semibold text-(--mono-text)"
        >
          {step.task.type.replace(/([a-z])([A-Z])/g, '$1 $2')}
        </div>
        <div class="text-[10px] text-(--mono-muted)">
          {step.preconditions.type === 'Unconditional'
            ? 'Unconditional when reached'
            : `${step.preconditions.clauses.length} clause${step.preconditions.clauses.length === 1 ? '' : 's'} · ${predicateCount()} timed predicate${predicateCount() === 1 ? '' : 's'}`}
        </div>
      </div>
    </div>
    <div class="flex items-center gap-0.5 self-start">
      <IconButton
        label={`Move step ${index + 1} up`}
        onclick={() => onMove(-1)}
        disabled={index === 0}
      >
        <ArrowUp size={14} />
      </IconButton>
      <IconButton
        label={`Move step ${index + 1} down`}
        onclick={() => onMove(1)}
        disabled={index === total - 1}
      >
        <ArrowDown size={14} />
      </IconButton>
      <IconButton
        label={`Remove step ${index + 1}`}
        onclick={onRemove}
        disabled={total === 1}
        class="ml-1"
      >
        <Trash2 size={14} />
      </IconButton>
    </div>
  </header>

  <div class="grid gap-2 rounded-xl bg-(--mono-bg) p-2.5">
    <div class="flex flex-wrap items-end justify-between gap-2">
      <SelectField
        label="Preconditions"
        value={step.preconditions.type}
        onchange={selectPreconditionMode}
        selectClass="h-9 py-1.5 text-xs"
      >
        <option value="Unconditional">Unconditional</option>
        <option value="AnyOf">Bounded DNF</option>
      </SelectField>
      {#if step.preconditions.type === 'AnyOf'}
        <Button
          size="sm"
          variant="ghost"
          onclick={addClause}
          disabled={predicateCount() >= 4 ||
            step.preconditions.clauses.length >= 4}
          class="inline-flex items-center gap-1"
        >
          <Plus size={12} /> Add OR clause
        </Button>
      {/if}
    </div>
    {#if step.preconditions.type === 'Unconditional'}
      <div
        class="rounded-xl border border-dashed border-(--mono-border) px-3 py-2 text-[10px] text-(--mono-muted)"
      >
        No predicate reads. The task remains eligible whenever the cursor
        reaches this row.
      </div>
    {:else}
      <div class="text-[10px] text-(--mono-muted)">
        Clauses compose with OR; predicates inside each clause compose with AND.
        Every predicate is visited. False skips only this task and advances.
      </div>
      {#each step.preconditions.clauses as clause, clauseIndex}
        <div
          class="grid gap-2 rounded-xl border border-(--mono-border) bg-white p-2"
        >
          <div
            class="flex items-center justify-between gap-2 text-[10px] text-(--mono-muted)"
          >
            <span>OR clause {clauseIndex + 1} · all predicates must pass</span>
            <Button
              size="sm"
              variant="ghost"
              onclick={() => addPredicate(clauseIndex)}
              disabled={predicateCount() >= 4 || clause.length >= 4}
            >
              <Plus size={12} /> Add AND predicate
            </Button>
          </div>
          {#each clause as timed, predicateIndex}
            <div class="grid gap-2">
              <SelectField
                label="Observation timing"
                value={timed.timing}
                onchange={(event) =>
                  setTiming(clauseIndex, predicateIndex, event)}
                selectClass="h-9 py-1.5 text-xs"
              >
                <option value="Opening">Opening — frozen for cycle</option>
                <option value="Current"
                  >Current — immediately before step</option
                >
              </SelectField>
              <AutomationConditionEditor
                bind:condition={
                  step.preconditions.clauses[clauseIndex][predicateIndex]
                    .predicate
                }
                {compact}
                onRemove={() => removePredicate(clauseIndex, predicateIndex)}
              />
            </div>
          {/each}
        </div>
      {/each}
    {/if}
  </div>

  <div class="grid gap-2">
    <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">
      Typed task
    </div>
    <AutomationTaskEditor bind:task={step.task} {actorType} {compact} />
  </div>

  <div class="grid gap-1 border-t border-(--mono-border) pt-3">
    <SelectField
      label="On failure"
      helper={step.errorPolicy.type === 'RetryLater'
        ? 'Temporary task failure or unavailable funding retries this row up to the declared unsuccessful-attempt limit.'
        : step.errorPolicy.type === 'ContinueNextStep'
          ? 'Task failure rolls back and advances to the next row.'
          : 'Task failure rolls back and terminates this logical cycle; unavailable funding still skips forward.'}
      value={step.errorPolicy.type}
      onchange={selectErrorPolicy}
      selectClass="h-9 py-1.5 text-xs"
    >
      <option value="AbortCycle">Abort on task failure</option>
      <option value="ContinueNextStep">Continue after task failure</option>
      <option value="RetryLater" disabled={mutability === 'Immutable'}>
        Retry temporary failure
      </option>
    </SelectField>
    {#if step.errorPolicy.type === 'RetryLater'}
      <NumberInput
        label="Maximum unsuccessful attempts"
        helper="Includes the initial unsuccessful attempt. A value of 1 closes immediately without suspension."
        min={1}
        max={4294967295}
        step={1}
        bind:value={step.errorPolicy.maxAttempts}
        class="h-9 py-1.5 text-xs"
      />
    {/if}
    <details class="group text-[11px] leading-relaxed text-(--mono-muted)">
      <summary
        class="w-fit cursor-pointer font-medium text-(--mono-fg) outline-none marker:text-(--mono-muted) focus-visible:ring-1 focus-visible:ring-(--mono-accent)"
      >
        Outcome semantics
      </summary>
      <ul class="mt-1.5 grid gap-1 border-l border-(--mono-border) pl-3">
        <li>
          <span class="text-(--mono-fg)">Condition false:</span> Skip this task and
          advance; the failure policy does not run.
        </li>
        <li>
          <span class="text-(--mono-fg)">Resolution skipped:</span> Record a non-failing
          skip and advance.
        </li>
        <li>
          <span class="text-(--mono-fg)">Funding unavailable:</span> Abort and Continue
          advance; Mutable Retry suspends, while Immutable Retry terminates.
        </li>
        <li>
          <span class="text-(--mono-fg)">Temporary task failure:</span> Continue advances,
          Abort terminates, and Mutable Retry suspends.
        </li>
        <li>
          <span class="text-(--mono-fg)">Permanent task failure:</span> Continue advances;
          Abort and Retry terminate.
        </li>
      </ul>
    </details>
    {#if step.task.type === 'StopCycle' && step.errorPolicy.type === 'ContinueNextStep'}
      <Notice variant="warn">
        False conditions skip this stop normally. An atomic condition or User
        fee-collection failure can also advance to the next row and may still
        end as an ordinary successful run. Use Abort on task failure unless that
        fall-through is deliberate.
      </Notice>
    {/if}
  </div>

  {#if issues.length > 0}
    <Notice variant="warn">
      <div class="grid gap-1">
        {#each issues as issue}
          <div>{issue.message}</div>
        {/each}
      </div>
    </Notice>
  {/if}

  <footer
    class="flex items-center justify-between gap-2 rounded-xl border border-(--mono-border) bg-(--mono-bg) px-2.5 py-2 text-[10px]"
  >
    <span class="text-(--mono-muted)">Only fixed cursor progression</span>
    <Badge variant="xyk">
      {step.task.type === 'StopCycle'
        ? 'Then complete current cycle'
        : index + 1 < total
          ? `Then step ${index + 2}`
          : 'Then complete'}
    </Badge>
  </footer>
</section>
