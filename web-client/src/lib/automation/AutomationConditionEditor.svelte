<!--
Domain: Automation condition editor
Owns: One typed AAA condition row and its bounded removal action.
Excludes: Predicate evaluation, chain reads, condition ordering semantics, and plan validation.
Zone: Automation presentation helper; composes authoring contracts and UI Kit fields.
-->
<script lang="ts">
  import { X } from '@lucide/svelte';

  import {
    AAA_AUTHORING_CONDITION_TYPES,
    type AaaAuthoringCondition,
    createAaaAuthoringCondition,
  } from '$lib/automation/authoring';
  import { IconButton, NumberInput, SelectField, TextField } from '$lib/ui';

  import AutomationAssetEditor from './AutomationAssetEditor.svelte';

  type Props = {
    condition: AaaAuthoringCondition;
    compact?: boolean;
    onRemove: () => void;
  };

  let { condition = $bindable(), compact = false, onRemove }: Props = $props();

  function selectConditionType(event: Event) {
    condition = createAaaAuthoringCondition(
      (event.currentTarget as HTMLSelectElement)
        .value as AaaAuthoringCondition['type'],
    );
  }

  function conditionLabel(type: AaaAuthoringCondition['type']) {
    return type.replace(/([a-z])([A-Z])/g, '$1 $2');
  }
</script>

<div class="grid gap-2 rounded-xl border border-(--mono-border) bg-white p-2.5">
  <div class="flex items-end gap-2">
    <SelectField
      label="Predicate"
      value={condition.type}
      onchange={selectConditionType}
      class="min-w-0 flex-1"
      selectClass="h-9 py-1.5 text-xs"
    >
      {#each AAA_AUTHORING_CONDITION_TYPES as type}
        <option value={type}>{conditionLabel(type)}</option>
      {/each}
    </SelectField>
    <IconButton
      label="Remove condition"
      onclick={onRemove}
      class="mb-0.5 shrink-0"
    >
      <X size={14} />
    </IconButton>
  </div>
  {#if condition.type === 'BalanceAbove' || condition.type === 'BalanceBelow' || condition.type === 'BalanceEquals' || condition.type === 'BalanceNotEquals'}
    <AutomationAssetEditor
      label="Observed asset"
      bind:asset={condition.asset}
      {compact}
    />
    <TextField
      label="Balance threshold (base units)"
      inputmode="numeric"
      pattern="[0-9]*"
      bind:value={condition.threshold}
      inputClass="h-9 py-1.5 text-xs tabnum"
    />
  {:else}
    <NumberInput
      label="Block threshold"
      min={0}
      max={4294967295}
      step={1}
      bind:value={condition.threshold}
      class="h-9 py-1.5 text-xs tabnum"
    />
  {/if}
</div>
