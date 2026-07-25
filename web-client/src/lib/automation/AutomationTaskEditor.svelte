<!--
Domain: Automation task editor
Owns: Typed parameter controls for every current AAA Task variant.
Excludes: Adapter execution, asset discovery, quotes, balances, task semantics, and plan lowering.
Zone: Automation presentation helper; binds one authoring task through finite UI Kit controls.
-->
<script lang="ts">
  import { Plus, X } from '@lucide/svelte';

  import {
    AAA_AUTHORING_TASK_TYPES,
    type AaaAuthoringTask,
    createAaaAuthoringTask,
  } from '$lib/automation/authoring';
  import type { AaaPlanType } from '$lib/automation/plan-artifact';
  import {
    Button,
    IconButton,
    NumberInput,
    SelectField,
    TextField,
  } from '$lib/ui';

  import AutomationAmountEditor from './AutomationAmountEditor.svelte';
  import AutomationAssetEditor from './AutomationAssetEditor.svelte';

  type Props = {
    task: AaaAuthoringTask;
    aaaType: AaaPlanType;
    compact?: boolean;
  };

  let { task = $bindable(), aaaType, compact = false }: Props = $props();

  const fieldGrid = $derived(compact ? 'grid gap-2' : 'grid grid-cols-2 gap-2');

  function selectTaskType(event: Event) {
    task = createAaaAuthoringTask(
      (event.currentTarget as HTMLSelectElement)
        .value as AaaAuthoringTask['type'],
    );
  }

  function taskLabel(type: AaaAuthoringTask['type']) {
    return type.replace(/([a-z])([A-Z])/g, '$1 $2');
  }

  function addSplitLeg() {
    if (task.type !== 'SplitTransfer' || task.legs.length >= 8) return;
    task = {
      ...task,
      legs: [...task.legs, { to: '', shareParts: 0 }],
    };
  }

  function removeSplitLeg(index: number) {
    if (task.type !== 'SplitTransfer') return;
    task = {
      ...task,
      legs: task.legs.filter((_, candidate) => candidate !== index),
    };
  }
</script>

<div class="grid gap-3">
  <SelectField
    label="Task"
    value={task.type}
    onchange={selectTaskType}
    selectClass="h-9 py-1.5 text-xs font-medium"
  >
    {#each AAA_AUTHORING_TASK_TYPES as type}
      <option value={type} disabled={type === 'Mint' && aaaType !== 'System'}>
        {taskLabel(type)}
      </option>
    {/each}
  </SelectField>

  {#if task.type === 'Transfer'}
    <TextField
      label="Recipient"
      placeholder="SS58 account"
      bind:value={task.to}
      inputClass="h-9 py-1.5 font-mono text-[11px]"
    />
    <AutomationAssetEditor label="Asset" bind:asset={task.asset} {compact} />
    <AutomationAmountEditor bind:amount={task.amount} {compact} />
  {:else if task.type === 'SplitTransfer'}
    <AutomationAssetEditor label="Asset" bind:asset={task.asset} {compact} />
    <AutomationAmountEditor bind:amount={task.amount} {compact} />
    <div class="grid gap-2">
      <div class="flex items-center justify-between gap-2">
        <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">
          Transfer legs · {task.legs.length}/8
        </div>
        <Button
          size="sm"
          variant="ghost"
          onclick={addSplitLeg}
          disabled={task.legs.length >= 8}
          class="inline-flex items-center gap-1"
        >
          <Plus size={12} /> Add leg
        </Button>
      </div>
      {#each task.legs as leg, legIndex}
        <div
          class={compact
            ? 'grid gap-2 rounded-xl bg-(--mono-bg) p-2'
            : 'grid grid-cols-[minmax(0,1fr)_minmax(8rem,0.45fr)_2rem] items-end gap-2 rounded-xl bg-(--mono-bg) p-2'}
        >
          <TextField
            label={`Recipient ${legIndex + 1}`}
            placeholder="SS58 account"
            bind:value={leg.to}
            inputClass="h-9 py-1.5 font-mono text-[11px]"
          />
          <NumberInput
            label="Share (perbill)"
            min={0}
            max={1000000000}
            step={1}
            bind:value={leg.shareParts}
            class="h-9 py-1.5 text-xs tabnum"
          />
          <IconButton
            label={`Remove transfer leg ${legIndex + 1}`}
            onclick={() => removeSplitLeg(legIndex)}
            disabled={task.legs.length <= 2}
            class={compact ? 'justify-self-end' : 'mb-0.5'}
          >
            <X size={14} />
          </IconButton>
        </div>
      {/each}
    </div>
  {:else if task.type === 'SwapExactIn'}
    <div class={fieldGrid}>
      <AutomationAssetEditor
        label="Asset in"
        bind:asset={task.assetIn}
        compact={true}
      />
      <AutomationAssetEditor
        label="Asset out"
        bind:asset={task.assetOut}
        compact={true}
      />
    </div>
    <AutomationAmountEditor
      label="Input amount mode"
      bind:amount={task.amountIn}
      {compact}
    />
    <NumberInput
      label="Slippage tolerance (perbill)"
      min={0}
      max={1000000000}
      step={1}
      bind:value={task.slippageParts}
      class="h-9 py-1.5 text-xs tabnum"
    />
  {:else if task.type === 'SwapExactOut'}
    <div class={fieldGrid}>
      <AutomationAssetEditor
        label="Asset in"
        bind:asset={task.assetIn}
        compact={true}
      />
      <AutomationAssetEditor
        label="Asset out"
        bind:asset={task.assetOut}
        compact={true}
      />
    </div>
    <AutomationAmountEditor
      label="Output amount mode"
      bind:amount={task.amountOut}
      {compact}
    />
    <TextField
      label="Maximum input (base units)"
      inputmode="numeric"
      pattern="[0-9]*"
      bind:value={task.maxAmountIn}
      inputClass="h-9 py-1.5 text-xs tabnum"
    />
    <NumberInput
      label="Slippage tolerance (perbill)"
      min={0}
      max={1000000000}
      step={1}
      bind:value={task.slippageParts}
      class="h-9 py-1.5 text-xs tabnum"
    />
  {:else if task.type === 'AddLiquidity'}
    <div class={fieldGrid}>
      <AutomationAssetEditor
        label="Asset A"
        bind:asset={task.assetA}
        compact={true}
      />
      <AutomationAssetEditor
        label="Asset B"
        bind:asset={task.assetB}
        compact={true}
      />
    </div>
    <div class={fieldGrid}>
      <AutomationAmountEditor
        label="Amount A mode"
        bind:amount={task.amountA}
        compact={true}
      />
      <AutomationAmountEditor
        label="Amount B mode"
        bind:amount={task.amountB}
        compact={true}
      />
    </div>
    <TextField
      label="Minimum LP output (base units)"
      inputmode="numeric"
      pattern="[0-9]*"
      bind:value={task.minLpOut}
      inputClass="h-9 py-1.5 text-xs tabnum"
    />
  {:else if task.type === 'RemoveLiquidity'}
    <AutomationAssetEditor
      label="LP asset"
      bind:asset={task.lpAsset}
      {compact}
    />
    <AutomationAmountEditor bind:amount={task.amount} {compact} />
    <div class={fieldGrid}>
      <TextField
        label="Minimum asset A output"
        inputmode="numeric"
        pattern="[0-9]*"
        bind:value={task.minAmountA}
        inputClass="h-9 py-1.5 text-xs tabnum"
      />
      <TextField
        label="Minimum asset B output"
        inputmode="numeric"
        pattern="[0-9]*"
        bind:value={task.minAmountB}
        inputClass="h-9 py-1.5 text-xs tabnum"
      />
    </div>
  {:else if task.type === 'Burn' || task.type === 'Mint' || task.type === 'Stake'}
    <AutomationAssetEditor label="Asset" bind:asset={task.asset} {compact} />
    <AutomationAmountEditor bind:amount={task.amount} {compact} />
  {:else if task.type === 'DonateLiquidity'}
    <div class={fieldGrid}>
      <AutomationAssetEditor
        label="Asset A"
        bind:asset={task.assetA}
        compact={true}
      />
      <AutomationAssetEditor
        label="Asset B"
        bind:asset={task.assetB}
        compact={true}
      />
    </div>
    <AutomationAmountEditor bind:amount={task.amount} {compact} />
    <NumberInput
      label="Maximum ratio error (perbill)"
      min={0}
      max={1000000000}
      step={1}
      bind:value={task.maxRatioErrorParts}
      class="h-9 py-1.5 text-xs tabnum"
    />
  {:else if task.type === 'Unstake'}
    <AutomationAssetEditor
      label="Position asset"
      bind:asset={task.asset}
      {compact}
    />
    <AutomationAmountEditor
      label="Share amount mode"
      bind:amount={task.shares}
      {compact}
    />
  {/if}
</div>
