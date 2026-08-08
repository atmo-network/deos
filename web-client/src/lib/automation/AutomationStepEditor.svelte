<!--
Domain: Automation step editor
Owns: One stable ordered Step row, condition controls, task parameters, error policy, and linear move/remove actions.
Excludes: Program storage, artifact encoding, analysis, simulation, and runtime execution.
Zone: Automation presentation helper; composes typed authoring fields without successor selection.
-->
<script lang="ts">
  import { ArrowDown, ArrowUp, Plus, Trash2 } from '@lucide/svelte';

  import {
    type AaaAuthoringIssue,
    type AaaAuthoringStep,
    createAaaAuthoringCondition,
  } from '$lib/automation/authoring';
  import type { AaaPlanType } from '$lib/automation/plan-artifact';
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
    step: AaaAuthoringStep;
    index: number;
    total: number;
    aaaType: AaaPlanType;
    mutability: AutomationMutability;
    compact?: boolean;
    issues?: AaaAuthoringIssue[];
    onMove: (direction: -1 | 1) => void;
    onRemove: () => void;
  };

  let {
    step = $bindable(),
    index,
    total,
    aaaType,
    mutability,
    compact = false,
    issues = [],
    onMove,
    onRemove,
  }: Props = $props();

  function selectConditionMode(event: Event) {
    const type = (event.currentTarget as HTMLSelectElement).value as
      | 'Always'
      | 'All'
      | 'Any';
    step = {
      ...step,
      conditionSet:
        type === 'Always'
          ? { type }
          : step.conditionSet.type === 'Always'
            ? {
                type,
                conditions: [createAaaAuthoringCondition('BalanceAbove')],
              }
            : { type, conditions: step.conditionSet.conditions },
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

  function addCondition() {
    if (
      step.conditionSet.type === 'Always' ||
      step.conditionSet.conditions.length >= 4
    )
      return;
    step = {
      ...step,
      conditionSet: {
        ...step.conditionSet,
        conditions: [
          ...step.conditionSet.conditions,
          createAaaAuthoringCondition('BalanceAbove'),
        ],
      },
    };
  }

  function removeCondition(indexToRemove: number) {
    if (step.conditionSet.type === 'Always') return;
    const conditions = step.conditionSet.conditions.filter(
      (_, candidate) => candidate !== indexToRemove,
    );
    step = {
      ...step,
      conditionSet:
        conditions.length === 0
          ? { type: 'Always' }
          : { ...step.conditionSet, conditions },
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
          {step.conditionSet.type === 'Always'
            ? 'Always attempted when reached'
            : `${step.conditionSet.conditions.length} ${step.conditionSet.type === 'All' ? 'conjunctive' : 'disjunctive'} condition${step.conditionSet.conditions.length === 1 ? '' : 's'}`}
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
        label="Condition mode"
        value={step.conditionSet.type}
        onchange={selectConditionMode}
        selectClass="h-9 py-1.5 text-xs"
      >
        <option value="Always">Always</option>
        <option value="All">All conditions</option>
        <option value="Any">Any condition</option>
      </SelectField>
      {#if step.conditionSet.type !== 'Always'}
        <Button
          size="sm"
          variant="ghost"
          onclick={addCondition}
          disabled={step.conditionSet.conditions.length >= 4}
          class="inline-flex items-center gap-1"
        >
          <Plus size={12} /> Add condition
        </Button>
      {/if}
    </div>
    {#if step.conditionSet.type === 'Always'}
      <div
        class="rounded-xl border border-dashed border-(--mono-border) px-3 py-2 text-[10px] text-(--mono-muted)"
      >
        No predicate reads. The task remains eligible whenever the cursor
        reaches this row.
      </div>
    {:else}
      <div class="text-[10px] text-(--mono-muted)">
        Every atom is evaluated. {step.conditionSet.type === 'All'
          ? 'All must pass.'
          : 'At least one must pass; any atomic error fails the group.'}
        A false group skips only this task and advances.
      </div>
      {#each step.conditionSet.conditions as condition, conditionIndex}
        <AutomationConditionEditor
          bind:condition={step.conditionSet.conditions[conditionIndex]}
          {compact}
          onRemove={() => removeCondition(conditionIndex)}
        />
      {/each}
    {/if}
  </div>

  <div class="grid gap-2">
    <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">
      Typed task
    </div>
    <AutomationTaskEditor bind:task={step.task} {aaaType} {compact} />
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
