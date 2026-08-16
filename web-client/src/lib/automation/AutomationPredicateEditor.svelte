<!--
Domain: Automation predicate editor
Owns: One typed Actors predicate row and its bounded removal action.
Excludes: Predicate evaluation, chain reads, predicate ordering semantics, and contract validation.
Zone: Automation presentation helper; composes authoring contracts and UI Kit fields.
-->
<script lang="ts">
  import { X } from '@lucide/svelte';

  import {
    ACTORS_AUTHORING_CONDITION_TYPES,
    type ActorAuthoringPredicate,
    createActorAuthoringPredicate,
  } from '$lib/automation/authoring';
  import { IconButton, NumberInput, SelectField, TextField } from '$lib/ui';

  import AutomationAssetEditor from './AutomationAssetEditor.svelte';

  type Props = {
    predicate: ActorAuthoringPredicate;
    compact?: boolean;
    onRemove: () => void;
  };

  let { predicate = $bindable(), compact = false, onRemove }: Props = $props();

  function selectPredicateType(event: Event) {
    predicate = createActorAuthoringPredicate(
      (event.currentTarget as HTMLSelectElement)
        .value as ActorAuthoringPredicate['type'],
    );
  }

  function predicateLabel(type: ActorAuthoringPredicate['type']) {
    return type.replace(/([a-z])([A-Z])/g, '$1 $2');
  }

  function selectAggregation(event: Event) {
    if (!predicate.type.startsWith('Observation')) return;
    const observation = predicate as Extract<
      ActorAuthoringPredicate,
      { feed: object }
    >;
    const type = (event.currentTarget as HTMLSelectElement).value;
    observation.feed.aggregation =
      type === 'Ema' ? { type, halfLifeBlocks: 100 } : { type: 'LastValue' };
    predicate = { ...observation, feed: { ...observation.feed } };
  }
</script>

<div class="grid gap-2 rounded-xl border border-(--mono-border) bg-white p-2.5">
  <div class="flex items-end gap-2">
    <SelectField
      label="Predicate"
      value={predicate.type}
      onchange={selectPredicateType}
      class="min-w-0 flex-1"
      selectClass="h-9 py-1.5 text-xs"
    >
      {#each ACTORS_AUTHORING_CONDITION_TYPES as type}
        <option value={type}>{predicateLabel(type)}</option>
      {/each}
    </SelectField>
    <IconButton
      label="Remove predicate"
      onclick={onRemove}
      class="mb-0.5 shrink-0"
    >
      <X size={14} />
    </IconButton>
  </div>
  {#if predicate.type === 'BalanceAbove' || predicate.type === 'BalanceBelow' || predicate.type === 'BalanceEquals' || predicate.type === 'BalanceNotEquals'}
    <AutomationAssetEditor
      label="Observed asset"
      bind:asset={predicate.asset}
      {compact}
    />
    <TextField
      label="Balance threshold (base units)"
      inputmode="numeric"
      pattern="[0-9]*"
      bind:value={predicate.threshold}
      inputClass="h-9 py-1.5 text-xs tabnum"
    />
  {:else if predicate.type === 'BlockNumberAbove' || predicate.type === 'BlockNumberBelow'}
    <NumberInput
      label="Block threshold"
      min={0}
      max={4294967295}
      step={1}
      bind:value={predicate.threshold}
      class="h-9 py-1.5 text-xs tabnum"
    />
  {:else}
    {@const observationPredicate = predicate as Extract<
      ActorAuthoringPredicate,
      { feed: object }
    >}
    <p
      class="text-[11px] font-semibold uppercase tracking-wide text-(--muted-foreground)"
    >
      Observation feed identity
    </p>
    <div class="grid gap-2 sm:grid-cols-2">
      <AutomationAssetEditor
        label="Input asset"
        bind:asset={observationPredicate.feed.assetIn}
        {compact}
      />
      <AutomationAssetEditor
        label="Output asset"
        bind:asset={observationPredicate.feed.assetOut}
        {compact}
      />
    </div>
    <div class="grid gap-2 sm:grid-cols-3">
      <SelectField
        label="Aggregation"
        value={observationPredicate.feed.aggregation.type}
        onchange={selectAggregation}
        selectClass="h-9 py-1.5 text-xs"
      >
        <option value="LastValue">Last value</option>
        <option value="Ema">EMA</option>
      </SelectField>
      <NumberInput
        label="Scale"
        min={0}
        max={255}
        step={1}
        bind:value={observationPredicate.feed.scale}
        class="h-9 py-1.5 text-xs tabnum"
      />
      <NumberInput
        label="Maximum age (blocks)"
        min={1}
        max={4294967295}
        step={1}
        bind:value={observationPredicate.maxAgeBlocks}
        class="h-9 py-1.5 text-xs tabnum"
      />
    </div>
    {#if observationPredicate.feed.aggregation.type === 'Ema'}
      <NumberInput
        label="EMA half-life (blocks)"
        min={1}
        max={4294967295}
        step={1}
        bind:value={observationPredicate.feed.aggregation.halfLifeBlocks}
        class="h-9 py-1.5 text-xs tabnum"
      />
    {/if}
    <TextField
      label="Observation threshold (raw scalar)"
      inputmode="numeric"
      pattern="[0-9]*"
      bind:value={observationPredicate.threshold}
      inputClass="h-9 py-1.5 text-xs tabnum"
    />
    <p class="text-[11px] leading-relaxed text-(--muted-foreground)">
      Only a fresh typed observation compares true; unavailable, uninitialized,
      and stale observations evaluate false.
    </p>
  {/if}
</div>
