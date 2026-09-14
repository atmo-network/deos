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
  };

  let {
    amount = $bindable(),
    label = 'Amount mode',
    compact = false,
  }: Props = $props();

  function selectAmountType(event: Event) {
    const type = (event.currentTarget as HTMLSelectElement)
      .value as ActorAuthoringAmount['type'];
    amount =
      type === 'Fixed' ? { type, value: '0' } : { type, parts: 500_000_000 };
  }

  const observation = $derived.by(() => {
    switch (amount.type) {
      case 'Fixed':
        return 'Artifact value; live capacity still applies';
      case 'Percent':
        return 'Re-observed at each step attempt';
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
    </SelectField>
    {#if amount.type === 'Fixed'}
      <TextField
        label="Base units"
        inputmode="numeric"
        pattern="[0-9]*"
        bind:value={amount.value}
        inputClass="h-9 py-1.5 text-xs tabnum"
      />
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
