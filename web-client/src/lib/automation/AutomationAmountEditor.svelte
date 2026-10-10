<!--
Domain: Automation amount editor
Owns: Typed AmountResolution field presentation and observation-window copy.
Excludes: Balance queries, fee reserve policy, amount forecasting, and runtime lowering.
Zone: Automation presentation helper; binds one authoring amount through UI Kit fields.
-->
<script lang="ts">
  import type { ActorAuthoringAmount } from '$lib/automation/authoring';
  import { NumberInput, SelectField, TextField } from '$lib/ui';

  type Props = {
    amount: ActorAuthoringAmount;
    label?: string;
    compact?: boolean;
    allowCycleBalance?: boolean;
  };

  let {
    amount = $bindable(),
    label = 'Amount mode',
    compact = false,
    allowCycleBalance = false,
  }: Props = $props();

  function selectAmountType(event: Event) {
    const type = (event.currentTarget as HTMLSelectElement)
      .value as ActorAuthoringAmount['type'];
    amount =
      type === 'Fixed'
        ? { type, value: '0' }
        : type === 'CaptureCycleBalance'
          ? { type, percentParts: 1_000_000_000, shareParts: 500_000_000 }
          : { type, parts: 500_000_000 };
  }

  const observation = $derived.by(() => {
    switch (amount.type) {
      case 'Fixed':
        return 'Artifact value; live capacity still applies';
      case 'Percent':
        return 'Re-observed at each step attempt';
      case 'CaptureCycleBalance':
        return 'Capture once at this Transfer. Later Steps use this volume; new ingress waits for the next cycle. User Steps still need their own action fees.';
      case 'CycleBalancePercent':
        return 'Share of the preceding capture of this asset. A skipped capture skips this Transfer.';
    }
  });
</script>

<div class="grid gap-1">
  <div class={compact ? 'grid gap-2' : 'grid grid-cols-2 gap-2'}>
    <SelectField
      {label}
      value={amount.type}
      onchange={selectAmountType}
      selectClass="h-9 py-1.5 text-xs"
    >
      <option value="Fixed">Fixed</option>
      <option value="Percent">% current</option>
      {#if allowCycleBalance}
        <option value="CaptureCycleBalance">Capture distribution</option>
        <option value="CycleBalancePercent">% captured volume</option>
      {/if}
    </SelectField>
    {#if amount.type === 'Fixed'}
      <TextField
        label="Base units"
        inputmode="numeric"
        pattern="[0-9]*"
        bind:value={amount.value}
        inputClass="h-9 py-1.5 text-xs tabnum"
      />
    {:else if amount.type === 'CaptureCycleBalance'}
      <div class="grid gap-2">
        <NumberInput
          label="Volume (% current, perbill)"
          min={1}
          max={1000000000}
          step={1}
          bind:value={amount.percentParts}
          class="h-9 py-1.5 text-xs tabnum"
        />
        <NumberInput
          label="This transfer's share (perbill)"
          min={1}
          max={1000000000}
          step={1}
          bind:value={amount.shareParts}
          class="h-9 py-1.5 text-xs tabnum"
        />
      </div>
    {:else}
      <NumberInput
        label="Perbill parts"
        min={0}
        max={1000000000}
        step={1}
        bind:value={amount.parts}
        class="h-9 py-1.5 text-xs tabnum"
      />
    {/if}
  </div>
  <div class="text-[10px] leading-relaxed text-(--mono-muted)">
    {observation}
  </div>
</div>
