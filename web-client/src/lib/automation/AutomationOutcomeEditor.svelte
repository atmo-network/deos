<!--
Domain: Actors outcome-policy authoring
Owns: Explicit bounded AND/OR outcome conditions and canonical draft ordering.
Excludes: Runtime admission, observation history, pricing, signing, and submission.
-->
<script lang="ts">
  import { Plus, Trash2 } from '@lucide/svelte';

  import {
    ACTOR_BEHAVIOR_METRICS,
    type ActorBehaviorLimits,
    type ActorBehaviorMetric,
    type ActorCloseWhen,
  } from '$lib/automation/behavior-policy';
  import { Button, IconButton, NumberInput, SelectField } from '$lib/ui';

  type Props = {
    policy: ActorCloseWhen | null;
    limits: ActorBehaviorLimits | null;
    compact?: boolean;
  };
  let { policy = $bindable(), limits, compact = false }: Props = $props();
  const labels: Record<ActorBehaviorMetric, string> = {
    PreconditionPassTotal: 'Precondition passes · total',
    PreconditionPassStreak: 'Precondition passes · consecutive',
    PreconditionRejectTotal: 'Precondition rejections · total',
    PreconditionRejectStreak: 'Precondition rejections · consecutive',
    ActionSuccessTotal: 'Successful Actions · total',
    ActionSuccessStreak: 'Successful Actions · consecutive',
    ActionFailureTotal: 'Failed Actions · total',
    ActionFailureStreak: 'Failed Actions · consecutive',
    CycleCompletedTotal: 'Normally completed Cycles',
    CycleCleanTotal: 'Clean Cycles',
  };
  function update(clauses: ActorCloseWhen['clauses']) {
    const ordered = clauses.map((clause) =>
      [...clause].sort(
        (left, right) =>
          ACTOR_BEHAVIOR_METRICS.indexOf(left.metric) -
          ACTOR_BEHAVIOR_METRICS.indexOf(right.metric),
      ),
    );
    ordered.sort((left, right) => {
      for (
        let index = 0;
        index < Math.min(left.length, right.length);
        index += 1
      ) {
        const metric =
          ACTOR_BEHAVIOR_METRICS.indexOf(left[index].metric) -
          ACTOR_BEHAVIOR_METRICS.indexOf(right[index].metric);
        const threshold = left[index].threshold - right[index].threshold;
        if (metric || threshold) return metric || threshold;
      }
      return left.length - right.length;
    });
    policy = ordered.length === 0 ? null : { clauses: ordered };
  }
  function selectMode(event: Event) {
    policy =
      (event.currentTarget as HTMLSelectElement).value === 'Absent'
        ? null
        : { clauses: [[{ metric: 'ActionSuccessTotal', threshold: 1 }]] };
  }
  function setMetric(clauseIndex: number, atomIndex: number, event: Event) {
    if (policy === null) return;
    const clauses = structuredClone(policy.clauses);
    clauses[clauseIndex][atomIndex].metric = (
      event.currentTarget as HTMLSelectElement
    ).value as ActorBehaviorMetric;
    update(clauses);
  }
  function setThreshold(clauseIndex: number, atomIndex: number, event: Event) {
    if (policy === null) return;
    const clauses = structuredClone(policy.clauses);
    clauses[clauseIndex][atomIndex].threshold = Number(
      (event.currentTarget as HTMLInputElement).value,
    );
    update(clauses);
  }
  function addAtom(clauseIndex: number) {
    if (policy === null || limits === null) return;
    const clauses = structuredClone(policy.clauses);
    if (clauses[clauseIndex].length >= limits.maxAtomsPerClause) return;
    const metric = ACTOR_BEHAVIOR_METRICS.find(
      (metric) => !clauses[clauseIndex].some((atom) => atom.metric === metric),
    );
    if (metric === undefined) return;
    clauses[clauseIndex].push({ metric, threshold: 1 });
    update(clauses);
  }
  function removeAtom(clauseIndex: number, atomIndex: number) {
    if (policy === null) return;
    const clauses = structuredClone(policy.clauses);
    clauses[clauseIndex].splice(atomIndex, 1);
    update(clauses.filter((clause) => clause.length > 0));
  }
  function addClause() {
    if (
      policy === null ||
      limits === null ||
      policy.clauses.length >= limits.maxClauses
    )
      return;
    update([
      ...policy.clauses,
      [{ metric: 'ActionSuccessTotal', threshold: 1 }],
    ]);
  }
</script>

<div class="grid gap-3">
  <SelectField
    label="Outcome condition"
    value={policy === null ? 'Absent' : 'Present'}
    onchange={selectMode}
    selectClass="h-9 py-1.5 text-xs"
  >
    <option value="Absent">No outcome rule</option>
    <option value="Present" disabled={limits === null}
      >Close when outcomes reach a threshold</option
    >
  </SelectField>
  {#if limits === null}
    <p class="text-[11px] text-(--mono-muted)">
      Bind matching runtime metadata to configure an outcome condition.
    </p>
  {/if}
  {#if policy !== null}
    <p class="text-[11px] text-(--mono-muted)">
      Any group may close the Actor. Every condition within that group must be
      met. The permitted Step result commits before closure.
    </p>
    {#each policy.clauses as clause, clauseIndex}
      <div class="grid gap-2 rounded-xl border border-(--mono-border) p-3">
        <div class="text-xs font-medium text-(--mono-text)">
          {clauseIndex === 0 ? 'When' : 'Or when'}
        </div>
        {#each clause as atom, atomIndex}
          <div
            class={compact
              ? 'grid gap-2'
              : 'grid grid-cols-[minmax(0,1fr)_7rem_auto] items-end gap-2'}
          >
            <SelectField
              label={atomIndex === 0 ? 'Metric' : 'And metric'}
              value={atom.metric}
              onchange={(event) => setMetric(clauseIndex, atomIndex, event)}
              selectClass="h-9 py-1.5 text-xs"
            >
              {#each ACTOR_BEHAVIOR_METRICS as metric}
                <option
                  value={metric}
                  disabled={metric !== atom.metric &&
                    clause.some((other) => other.metric === metric)}
                  >{labels[metric]}</option
                >
              {/each}
            </SelectField>
            <NumberInput
              label="At least"
              min={1}
              max={4294967295}
              step={1}
              value={atom.threshold}
              oninput={(event) => setThreshold(clauseIndex, atomIndex, event)}
              class="h-9 py-1.5 text-xs tabnum"
            />
            <IconButton
              label="Remove outcome condition"
              onclick={() => removeAtom(clauseIndex, atomIndex)}
              ><Trash2 size={14} /></IconButton
            >
          </div>
        {/each}
        <Button
          variant="secondary"
          size="sm"
          disabled={limits === null ||
            clause.length >= limits.maxAtomsPerClause ||
            clause.length >= ACTOR_BEHAVIOR_METRICS.length}
          onclick={() => addAtom(clauseIndex)}
          ><Plus size={12} /> And condition</Button
        >
      </div>
    {/each}
    <Button
      variant="secondary"
      size="sm"
      disabled={limits === null || policy.clauses.length >= limits.maxClauses}
      onclick={addClause}><Plus size={12} /> Or group</Button
    >
    <p class="text-[11px] text-(--mono-muted)">
      Counts belong to this Contract episode. Skips leave Action streaks
      unchanged. A clean Cycle must reach plan end with economic success and no
      failed or skipped economic work.
    </p>
  {/if}
</div>
