<!--
Domain: Automation task editor
Owns: Typed parameter controls for every current Actors Task variant.
Excludes: Adapter execution, asset discovery, quotes, balances, task semantics, and plan lowering.
Zone: Automation presentation helper; binds one authoring task through finite UI Kit controls.
-->
<script lang="ts">
  import { Plus, X } from '@lucide/svelte';

  import {
    ACTORS_AUTHORING_TASK_TYPES,
    type ActorAuthoringTask,
    createActorAuthoringTask,
  } from '$lib/automation/authoring';
  import type { ActorContractType } from '$lib/automation/contract-artifact';
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
    task: ActorAuthoringTask;
    actorType: ActorContractType;
    compact?: boolean;
  };

  let { task = $bindable(), actorType, compact = false }: Props = $props();

  const fieldGrid = $derived(compact ? 'grid gap-2' : 'grid grid-cols-2 gap-2');

  function selectTaskType(event: Event) {
    task = createActorAuthoringTask(
      (event.currentTarget as HTMLSelectElement)
        .value as ActorAuthoringTask['type'],
    );
  }

  function taskLabel(type: ActorAuthoringTask['type']) {
    return type.replace(/([a-z])([A-Z])/g, '$1 $2');
  }

  function selectInputLimit(event: Event) {
    if (task.type !== 'SwapOut') return;
    const type = (event.currentTarget as HTMLSelectElement).value;
    task = {
      ...task,
      inputLimit:
        type === 'Absolute'
          ? { type: 'Absolute', amount: '1' }
          : { type: 'LiveQuote' },
    };
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
    {#each ACTORS_AUTHORING_TASK_TYPES as type}
      <option value={type} disabled={type === 'Mint' && actorType !== 'System'}>
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
    <p class="text-[10px] leading-relaxed text-(--mono-muted)">
      Every non-zero leg must accept its allocation at execution time. One
      ineligible recipient fails the whole task atomically as a temporary error;
      rejected value never becomes retained remainder.
    </p>
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
  {:else if task.type === 'SwapIn'}
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
    <p class="text-[11px] leading-relaxed text-muted-foreground">
      System execution also checks a fresh EMA or direct-pool reserve reference.
      This local guard may reflect manipulated pool state and proves neither
      fair price nor transaction-order protection.
    </p>
  {:else if task.type === 'SwapOut'}
    <AutomationAssetEditor
      label="Asset out"
      bind:asset={task.assetOut}
      compact={true}
    />
    <AutomationAmountEditor
      label="Output amount mode"
      bind:amount={task.amountOut}
      {compact}
    />
    <AutomationAssetEditor
      label="Asset in"
      bind:asset={task.assetIn}
      compact={true}
    />
    <SelectField
      label="Input protection"
      value={task.inputLimit.type}
      onchange={selectInputLimit}
      selectClass="h-9 py-1.5 text-xs font-medium"
    >
      <option value="LiveQuote">Live market quote</option>
      <option value="Absolute">Absolute input ceiling</option>
    </SelectField>
    {#if task.inputLimit.type === 'Absolute'}
      <TextField
        label="Absolute input ceiling (base units)"
        inputmode="numeric"
        pattern="[0-9]*"
        bind:value={task.inputLimit.amount}
        inputClass="h-9 py-1.5 text-xs tabnum"
      />
      <p class="text-[11px] leading-relaxed text-muted-foreground">
        Execution will not spend above this declared maximum input.
      </p>
    {:else}
      <p class="text-[11px] leading-relaxed text-warning-foreground">
        Live-market mode may execute at any future market price, subject only to
        attempt-time quote-relative slippage and available balance.
      </p>
    {/if}
    <NumberInput
      label="Slippage tolerance (perbill)"
      min={0}
      max={1000000000}
      step={1}
      bind:value={task.slippageParts}
      class="h-9 py-1.5 text-xs tabnum"
    />
    <p class="text-[11px] leading-relaxed text-muted-foreground">
      System execution also checks a fresh EMA or direct-pool reserve reference.
      This local guard may reflect manipulated pool state and proves neither
      fair price nor transaction-order protection.
    </p>
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
    <AutomationAssetEditor label="Asset A" bind:asset={task.assetA} {compact} />
    <AutomationAssetEditor label="Asset B" bind:asset={task.assetB} {compact} />
    <AutomationAmountEditor bind:amount={task.lpAmount} {compact} />
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
    <AutomationAmountEditor bind:amount={task.maxAmountA} {compact} />
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
