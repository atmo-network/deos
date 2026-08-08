<!--
Domain: Automation amount editor
Owns: Typed AmountResolution field presentation and observation-window copy.
Excludes: Balance queries, fee reserve policy, amount forecasting, and runtime lowering.
Zone: Automation presentation helper; binds one authoring amount through UI Kit fields.
-->
<script lang="ts">
  import type { AaaAuthoringAmount } from '$lib/automation/authoring';
  import { NumberInput, SelectField, TextField } from '$lib/ui';

  type Props = {
    amount: AaaAuthoringAmount;
    label?: string;
    compact?: boolean;
  };

  let {
    amount = $bindable(),
    label = 'Amount mode',
    compact = false,
  }: Props = $props();

  function selectAmountType(event: Event) {
    const type = (event.currentTarget as HTMLSelectElement)
      .value as AaaAuthoringAmount['type'];
    amount =
      type === 'Fixed'
        ? { type, value: '0' }
        : type === 'AllAvailable'
          ? { type }
          : { type, parts: 500_000_000 };
  }

  const observation = $derived.by(() => {
    switch (amount.type) {
      case 'Fixed':
        return 'Artifact value; live capacity still applies';
      case 'PercentageOfCurrent':
      case 'AllAvailable':
        return 'Re-observed at each step attempt';
      case 'PercentageAtOpening':
        return 'Frozen at logical-cycle start';
      case 'PercentageOfLastFunding':
        return 'Frozen from the latest accepted funding';
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
      <option value="PercentageOfCurrent">% current</option>
      <option value="PercentageAtOpening">% at opening</option>
      <option value="PercentageOfLastFunding">% last funding</option>
      <option value="AllAvailable">All available</option>
    </SelectField>
    {#if amount.type === 'Fixed'}
      <TextField
        label="Base units"
        inputmode="numeric"
        pattern="[0-9]*"
        bind:value={amount.value}
        inputClass="h-9 py-1.5 text-xs tabnum"
      />
    {:else if amount.type !== 'AllAvailable'}
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
