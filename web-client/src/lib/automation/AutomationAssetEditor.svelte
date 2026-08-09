<!--
Domain: Automation asset editor
Owns: Typed AssetKind field presentation for Actors authoring rows.
Excludes: Asset discovery, balances, runtime registry policy, and plan lowering.
Zone: Automation presentation helper; binds one authoring asset through UI Kit fields.
-->
<script lang="ts">
  import type { ActorAuthoringAsset } from '$lib/automation/authoring';
  import { NumberInput, SelectField } from '$lib/ui';

  type Props = {
    asset: ActorAuthoringAsset;
    label: string;
    compact?: boolean;
  };

  let { asset = $bindable(), label, compact = false }: Props = $props();

  function selectAssetType(event: Event) {
    const type = (event.currentTarget as HTMLSelectElement)
      .value as ActorAuthoringAsset['type'];
    asset = type === 'Native' ? { type } : { type, id: 0 };
  }
</script>

<div
  class={compact
    ? 'grid gap-2'
    : 'grid grid-cols-[minmax(0,1fr)_minmax(7rem,0.65fr)] gap-2'}
>
  <SelectField
    {label}
    value={asset.type}
    onchange={selectAssetType}
    selectClass="h-9 py-1.5 text-xs"
  >
    <option value="Native">Native</option>
    <option value="Local">Local</option>
    <option value="Foreign">Foreign</option>
  </SelectField>
  {#if asset.type === 'Native'}
    <div class="hidden" aria-hidden="true"></div>
  {:else}
    <NumberInput
      label="Asset ID"
      min={0}
      max={4294967295}
      step={1}
      bind:value={asset.id}
      class="h-9 py-1.5 text-xs tabnum"
    />
  {/if}
</div>
