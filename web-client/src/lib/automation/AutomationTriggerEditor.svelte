<!--
Domain: Actors trigger authoring
Owns: One scalar Manual, AddressEvent, ObservationChange, or Cadenced trigger control.
Excludes: Runtime admission, scheduler execution, conditions, graph control, and artifact encoding.
Zone: Automation presentation helper; edits the canonical trigger draft without inventing another trigger model.
-->
<script lang="ts">
  import { Plus, Trash2 } from '@lucide/svelte';

  import {
    type ActorAuthoringAsset,
    type ActorAuthoringObservationFeed,
    type ActorAuthoringTrigger,
    DEOS_ACTORS_AUTHORING_LIMITS,
  } from '$lib/automation/authoring';
  import {
    Badge,
    Button,
    IconButton,
    NumberInput,
    SelectField,
    TextArea,
  } from '$lib/ui';

  import AutomationAssetEditor from './AutomationAssetEditor.svelte';

  type Props = {
    trigger: ActorAuthoringTrigger;
    compact?: boolean;
  };

  let { trigger = $bindable(), compact = false }: Props = $props();

  function defaultObservationFeed(): ActorAuthoringObservationFeed {
    return {
      assetIn: { type: 'Native' },
      assetOut: { type: 'Local', id: 0 },
      method: 'PreExecutionSpot',
      aggregation: { type: 'Ema', halfLifeBlocks: 100 },
      scale: 12,
    };
  }

  function selectTriggerType(event: Event) {
    const type = (event.currentTarget as HTMLSelectElement)
      .value as ActorAuthoringTrigger['type'];
    if (type === trigger.type) return;
    switch (type) {
      case 'Manual':
        trigger = { type };
        return;
      case 'AddressEvent':
        trigger = {
          type,
          sourceFilter: { type: 'Any' },
          assetFilter: { type: 'Any' },
        };
        return;
      case 'ObservationChange':
        trigger = { type, feed: defaultObservationFeed() };
        return;
      case 'Cadenced':
        trigger = { type, everyTicks: 1 };
    }
  }

  function selectSourceFilter(event: Event) {
    if (trigger.type !== 'AddressEvent') return;
    const type = (event.currentTarget as HTMLSelectElement).value as
      | 'Any'
      | 'OwnerOnly'
      | 'Whitelist';
    trigger = {
      ...trigger,
      sourceFilter: type === 'Whitelist' ? { type, accounts: [''] } : { type },
    };
  }

  function updateAccountWhitelist(value: string) {
    if (
      trigger.type !== 'AddressEvent' ||
      trigger.sourceFilter.type !== 'Whitelist'
    )
      return;
    trigger = {
      ...trigger,
      sourceFilter: {
        type: 'Whitelist',
        accounts: value.split('\n').map((account) => account.trim()),
      },
    };
  }

  function selectAssetFilter(event: Event) {
    if (trigger.type !== 'AddressEvent') return;
    const type = (event.currentTarget as HTMLSelectElement).value as
      | 'Any'
      | 'Whitelist';
    trigger = {
      ...trigger,
      assetFilter:
        type === 'Whitelist'
          ? { type, assets: [{ type: 'Native' }] }
          : { type },
    };
  }

  function addAsset() {
    if (
      trigger.type !== 'AddressEvent' ||
      trigger.assetFilter.type !== 'Whitelist' ||
      trigger.assetFilter.assets.length >=
        DEOS_ACTORS_AUTHORING_LIMITS.maxWhitelistSize
    )
      return;
    trigger = {
      ...trigger,
      assetFilter: {
        ...trigger.assetFilter,
        assets: [...trigger.assetFilter.assets, { type: 'Local', id: 0 }],
      },
    };
  }

  function replaceAsset(index: number, asset: ActorAuthoringAsset) {
    if (
      trigger.type !== 'AddressEvent' ||
      trigger.assetFilter.type !== 'Whitelist'
    )
      return;
    trigger = {
      ...trigger,
      assetFilter: {
        ...trigger.assetFilter,
        assets: trigger.assetFilter.assets.map((candidate, candidateIndex) =>
          candidateIndex === index ? asset : candidate,
        ),
      },
    };
  }

  function selectAssetType(index: number, event: Event) {
    const type = (event.currentTarget as HTMLSelectElement).value as
      | 'Native'
      | 'Local'
      | 'Foreign';
    replaceAsset(index, type === 'Native' ? { type } : { type, id: 0 });
  }

  function removeAsset(index: number) {
    if (
      trigger.type !== 'AddressEvent' ||
      trigger.assetFilter.type !== 'Whitelist'
    )
      return;
    trigger = {
      ...trigger,
      assetFilter: {
        ...trigger.assetFilter,
        assets: trigger.assetFilter.assets.filter(
          (_, candidate) => candidate !== index,
        ),
      },
    };
  }

  function selectObservationAggregation(event: Event) {
    if (trigger.type !== 'ObservationChange') return;
    const type = (event.currentTarget as HTMLSelectElement).value;
    trigger = {
      ...trigger,
      feed: {
        ...trigger.feed,
        aggregation:
          type === 'Ema'
            ? { type, halfLifeBlocks: 100 }
            : { type: 'LastValue' },
      },
    };
  }
</script>

<section
  class="grid gap-3 rounded-2xl border border-(--mono-border) bg-white p-3"
>
  <header class="grid gap-1">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <div class="text-xs font-semibold text-(--mono-text)">Trigger</div>
      <Badge variant="xyk">One trigger · one pipeline</Badge>
    </div>
    <p class="text-[10px] text-(--mono-muted)">
      Select one readiness source. Independent event sources require separate
      Actors; composite conditions belong in step preconditions.
    </p>
  </header>

  <div class={compact ? 'grid gap-2' : 'grid grid-cols-2 gap-2'}>
    <SelectField
      label="Trigger type"
      value={trigger.type}
      onchange={selectTriggerType}
      selectClass="h-9 py-1.5 text-xs"
    >
      <option value="Manual">Manual</option>
      <option value="AddressEvent">Address event</option>
      <option value="ObservationChange">Observation change</option>
      <option value="Cadenced">Cadenced</option>
    </SelectField>
    {#if trigger.type === 'Cadenced'}
      <NumberInput
        label="Every 500 ms ticks"
        min={1}
        max={DEOS_ACTORS_AUTHORING_LIMITS.maxCadenceTicks}
        step={1}
        bind:value={trigger.everyTicks}
        class="h-9 py-1.5 text-xs tabnum"
      />
    {/if}
  </div>

  {#if trigger.type === 'Manual'}
    <p class="rounded-xl bg-(--mono-bg) p-2.5 text-[10px] text-(--mono-muted)">
      Readiness is latched only by the authorized manual trigger call.
    </p>
  {:else if trigger.type === 'AddressEvent'}
    <div class="grid gap-2 rounded-xl bg-(--mono-bg) p-2.5">
      <div class={compact ? 'grid gap-2' : 'grid grid-cols-2 gap-2'}>
        <SelectField
          label="Sender filter"
          value={trigger.sourceFilter.type}
          onchange={selectSourceFilter}
          selectClass="h-9 py-1.5 text-xs"
        >
          <option value="Any">Any sender</option>
          <option value="OwnerOnly">Owner only</option>
          <option value="Whitelist">Whitelist</option>
        </SelectField>
        <SelectField
          label="Asset filter"
          value={trigger.assetFilter.type}
          onchange={selectAssetFilter}
          selectClass="h-9 py-1.5 text-xs"
        >
          <option value="Any">Any asset</option>
          <option value="Whitelist">Whitelist</option>
        </SelectField>
      </div>

      {#if trigger.sourceFilter.type === 'Whitelist'}
        <TextArea
          label="Sender whitelist"
          helper="One SS58 or 32-byte hex account per line. Canonical order is applied during lowering."
          value={trigger.sourceFilter.accounts.join('\n')}
          oninput={(event) =>
            updateAccountWhitelist(
              (event.currentTarget as HTMLTextAreaElement).value,
            )}
          rows={3}
          textareaClass="font-mono text-xs"
        />
      {/if}

      {#if trigger.assetFilter.type === 'Whitelist'}
        <div class="grid gap-1.5">
          <div class="flex items-center justify-between gap-2">
            <div class="text-[10px] text-(--mono-muted)">Asset whitelist</div>
            <Button
              size="sm"
              variant="ghost"
              disabled={trigger.assetFilter.assets.length >=
                DEOS_ACTORS_AUTHORING_LIMITS.maxWhitelistSize}
              onclick={addAsset}
            >
              <Plus size={12} /> Asset
            </Button>
          </div>
          {#each trigger.assetFilter.assets as asset, assetIndex}
            <div
              class="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] gap-1.5"
            >
              <SelectField
                label={`Asset ${assetIndex + 1}`}
                value={asset.type}
                onchange={(event) => selectAssetType(assetIndex, event)}
                selectClass="h-9 py-1.5 text-xs"
              >
                <option value="Native">Native</option>
                <option value="Local">Local</option>
                <option value="Foreign">Foreign</option>
              </SelectField>
              {#if asset.type !== 'Native'}
                <NumberInput
                  label="Asset ID"
                  min={0}
                  max={4294967295}
                  step={1}
                  bind:value={asset.id}
                  class="h-9 py-1.5 text-xs tabnum"
                />
              {:else}
                <div></div>
              {/if}
              <div class="self-end pb-0.5">
                <IconButton
                  label={`Remove asset ${assetIndex + 1}`}
                  onclick={() => removeAsset(assetIndex)}
                >
                  <Trash2 size={13} />
                </IconButton>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {:else if trigger.type === 'ObservationChange'}
    <div class="grid gap-2 rounded-xl bg-(--mono-bg) p-2.5">
      <p class="text-[10px] text-(--mono-muted)">
        Latest-state reconsideration only. Thresholds belong to plan conditions;
        this trigger carries no amount or revision payload.
      </p>
      <div class="grid gap-2 sm:grid-cols-2">
        <AutomationAssetEditor
          label="Input asset"
          bind:asset={trigger.feed.assetIn}
          {compact}
        />
        <AutomationAssetEditor
          label="Output asset"
          bind:asset={trigger.feed.assetOut}
          {compact}
        />
      </div>
      <div class="grid gap-2 sm:grid-cols-3">
        <SelectField
          label="Aggregation"
          value={trigger.feed.aggregation.type}
          onchange={selectObservationAggregation}
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
          bind:value={trigger.feed.scale}
          class="h-9 py-1.5 text-xs tabnum"
        />
        {#if trigger.feed.aggregation.type === 'Ema'}
          <NumberInput
            label="EMA half-life"
            min={1}
            max={4294967295}
            step={1}
            bind:value={trigger.feed.aggregation.halfLifeBlocks}
            class="h-9 py-1.5 text-xs tabnum"
          />
        {/if}
      </div>
    </div>
  {:else}
    <p class="rounded-xl bg-(--mono-bg) p-2.5 text-[10px] text-(--mono-muted)">
      Consensus time is quantized into 500 ms ticks. Delayed periods coalesce
      into one opportunity without catch-up execution.
    </p>
  {/if}
</section>
