<!--
Domain: AAA trigger authoring
Owns: Bounded source-set and admission-gate controls for one typed authoring trigger.
Excludes: Runtime admission, scheduler execution, conditions, graph control, and artifact encoding.
Zone: Automation presentation helper; edits the canonical trigger draft without inventing another trigger model.
-->
<script lang="ts">
  import { Plus, Trash2 } from '@lucide/svelte';

  import {
    type AaaAuthoringAsset,
    type AaaAuthoringTrigger,
    type AaaAuthoringTriggerSource,
    DEOS_AAA_AUTHORING_LIMITS,
  } from '$lib/automation/authoring';
  import {
    Badge,
    Button,
    IconButton,
    NumberInput,
    SelectField,
    TextArea,
  } from '$lib/ui';

  type Props = {
    trigger: AaaAuthoringTrigger;
    compact?: boolean;
  };

  let { trigger = $bindable(), compact = false }: Props = $props();
  let rememberedSources = $state<AaaAuthoringTriggerSource[]>(
    trigger.type === 'Immediate'
      ? trigger.sources
      : trigger.mode.type === 'WhenSignalled'
        ? trigger.mode.sources
        : [{ type: 'Manual' }],
  );

  const sources = $derived(
    trigger.type === 'Immediate'
      ? trigger.sources
      : trigger.mode.type === 'WhenSignalled'
        ? trigger.mode.sources
        : [],
  );
  const canAddSource = $derived(
    sources.length < DEOS_AAA_AUTHORING_LIMITS.maxTriggerSources,
  );
  const hasManual = $derived(
    sources.some((source) => source.type === 'Manual'),
  );

  function setSources(nextSources: AaaAuthoringTriggerSource[]) {
    rememberedSources = nextSources;
    if (trigger.type === 'Immediate') {
      trigger = { ...trigger, sources: nextSources };
      return;
    }
    if (trigger.mode.type === 'WhenSignalled') {
      trigger = {
        ...trigger,
        mode: { ...trigger.mode, sources: nextSources },
      };
    }
  }

  function selectAdmission(event: Event) {
    const type = (event.currentTarget as HTMLSelectElement).value as
      | 'Immediate'
      | 'Cadenced';
    if (type === trigger.type) return;
    const currentSources =
      trigger.type === 'Immediate'
        ? trigger.sources
        : trigger.mode.type === 'WhenSignalled'
          ? trigger.mode.sources
          : rememberedSources;
    trigger =
      type === 'Immediate'
        ? {
            type,
            sources: currentSources,
          }
        : {
            type,
            everyBlocks: 1,
            mode: { type: 'WhenSignalled', sources: currentSources },
          };
  }

  function selectCadenceMode(event: Event) {
    if (trigger.type !== 'Cadenced') return;
    const type = (event.currentTarget as HTMLSelectElement).value as
      | 'Always'
      | 'WhenSignalled';
    if (trigger.mode.type === 'WhenSignalled') {
      rememberedSources = trigger.mode.sources;
    }
    trigger = {
      ...trigger,
      mode: type === 'Always' ? { type } : { type, sources: rememberedSources },
    };
  }

  function addSource(type: 'Manual' | 'OnAddressEvent') {
    if (!canAddSource || (type === 'Manual' && hasManual)) return;
    const source: AaaAuthoringTriggerSource =
      type === 'Manual'
        ? { type }
        : {
            type,
            sourceFilter: { type: 'Any' },
            assetFilter: { type: 'Any' },
          };
    setSources([...sources, source]);
  }

  function replaceSource(index: number, source: AaaAuthoringTriggerSource) {
    setSources(
      sources.map((candidate, candidateIndex) =>
        candidateIndex === index ? source : candidate,
      ),
    );
  }

  function removeSource(index: number) {
    setSources(sources.filter((_, candidate) => candidate !== index));
  }

  function selectSourceFilter(index: number, event: Event) {
    const source = sources[index];
    if (source?.type !== 'OnAddressEvent') return;
    const type = (event.currentTarget as HTMLSelectElement).value as
      | 'Any'
      | 'OwnerOnly'
      | 'Whitelist';
    replaceSource(index, {
      ...source,
      sourceFilter: type === 'Whitelist' ? { type, accounts: [''] } : { type },
    });
  }

  function updateAccountWhitelist(index: number, value: string) {
    const source = sources[index];
    if (
      source?.type !== 'OnAddressEvent' ||
      source.sourceFilter.type !== 'Whitelist'
    )
      return;
    replaceSource(index, {
      ...source,
      sourceFilter: {
        type: 'Whitelist',
        accounts: value.split('\n').map((account) => account.trim()),
      },
    });
  }

  function selectAssetFilter(index: number, event: Event) {
    const source = sources[index];
    if (source?.type !== 'OnAddressEvent') return;
    const type = (event.currentTarget as HTMLSelectElement).value as
      | 'Any'
      | 'Whitelist';
    replaceSource(index, {
      ...source,
      assetFilter:
        type === 'Whitelist'
          ? { type, assets: [{ type: 'Native' }] }
          : { type },
    });
  }

  function addAsset(index: number) {
    const source = sources[index];
    if (
      source?.type !== 'OnAddressEvent' ||
      source.assetFilter.type !== 'Whitelist' ||
      source.assetFilter.assets.length >=
        DEOS_AAA_AUTHORING_LIMITS.maxWhitelistSize
    )
      return;
    replaceSource(index, {
      ...source,
      assetFilter: {
        ...source.assetFilter,
        assets: [...source.assetFilter.assets, { type: 'Local', id: 0 }],
      },
    });
  }

  function replaceAsset(
    sourceIndex: number,
    assetIndex: number,
    asset: AaaAuthoringAsset,
  ) {
    const source = sources[sourceIndex];
    if (
      source?.type !== 'OnAddressEvent' ||
      source.assetFilter.type !== 'Whitelist'
    )
      return;
    replaceSource(sourceIndex, {
      ...source,
      assetFilter: {
        ...source.assetFilter,
        assets: source.assetFilter.assets.map((candidate, candidateIndex) =>
          candidateIndex === assetIndex ? asset : candidate,
        ),
      },
    });
  }

  function selectAssetType(
    sourceIndex: number,
    assetIndex: number,
    event: Event,
  ) {
    const type = (event.currentTarget as HTMLSelectElement).value as
      | 'Native'
      | 'Local'
      | 'Foreign';
    replaceAsset(
      sourceIndex,
      assetIndex,
      type === 'Native' ? { type } : { type, id: 0 },
    );
  }

  function removeAsset(sourceIndex: number, assetIndex: number) {
    const source = sources[sourceIndex];
    if (
      source?.type !== 'OnAddressEvent' ||
      source.assetFilter.type !== 'Whitelist'
    )
      return;
    replaceSource(sourceIndex, {
      ...source,
      assetFilter: {
        ...source.assetFilter,
        assets: source.assetFilter.assets.filter(
          (_, candidate) => candidate !== assetIndex,
        ),
      },
    });
  }
</script>

<section
  class="grid gap-3 rounded-2xl border border-(--mono-border) bg-white p-3"
>
  <header class="grid gap-1">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <div class="text-xs font-semibold text-(--mono-text)">
        Trigger admission
      </div>
      <Badge variant="xyk">OR-only · max 4 sources</Badge>
    </div>
    <p class="text-[10px] text-(--mono-muted)">
      Sources mark readiness. Admission decides whether readiness enters the
      scheduler now or at a fixed cadence.
    </p>
  </header>

  <div class={compact ? 'grid gap-2' : 'grid grid-cols-2 gap-2'}>
    <SelectField
      label="Admission"
      value={trigger.type}
      onchange={selectAdmission}
      selectClass="h-9 py-1.5 text-xs"
    >
      <option value="Immediate">Immediate</option>
      <option value="Cadenced">Cadenced</option>
    </SelectField>
    {#if trigger.type === 'Cadenced'}
      <NumberInput
        label="Every blocks"
        min={1}
        max={4294967295}
        step={1}
        bind:value={trigger.everyBlocks}
        class="h-9 py-1.5 text-xs tabnum"
      />
      <SelectField
        label="Cadence mode"
        value={trigger.mode.type}
        onchange={selectCadenceMode}
        selectClass="h-9 py-1.5 text-xs"
      >
        <option value="Always">Always</option>
        <option value="WhenSignalled">When signalled</option>
      </SelectField>
    {/if}
  </div>

  {#if trigger.type === 'Immediate' || trigger.mode.type === 'WhenSignalled'}
    <div class="grid gap-2">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">
          Readiness sources · {sources.length}/{DEOS_AAA_AUTHORING_LIMITS.maxTriggerSources}
        </div>
        <div class="flex flex-wrap gap-1">
          <Button
            size="sm"
            variant="secondary"
            disabled={!canAddSource || hasManual}
            onclick={() => addSource('Manual')}
          >
            <Plus size={12} /> Manual
          </Button>
          <Button
            size="sm"
            variant="secondary"
            disabled={!canAddSource}
            onclick={() => addSource('OnAddressEvent')}
          >
            <Plus size={12} /> Address event
          </Button>
        </div>
      </div>

      {#each sources as source, sourceIndex}
        <article class="grid gap-2 rounded-xl bg-(--mono-bg) p-2.5">
          <div class="flex items-center justify-between gap-2">
            <div class="text-xs font-medium text-(--mono-text)">
              {source.type === 'Manual' ? 'Manual' : 'Address event'}
            </div>
            <IconButton
              label={`Remove source ${sourceIndex + 1}`}
              onclick={() => removeSource(sourceIndex)}
            >
              <Trash2 size={13} />
            </IconButton>
          </div>

          {#if source.type === 'OnAddressEvent'}
            <div class={compact ? 'grid gap-2' : 'grid grid-cols-2 gap-2'}>
              <SelectField
                label="Sender filter"
                value={source.sourceFilter.type}
                onchange={(event) => selectSourceFilter(sourceIndex, event)}
                selectClass="h-9 py-1.5 text-xs"
              >
                <option value="Any">Any sender</option>
                <option value="OwnerOnly">Owner only</option>
                <option value="Whitelist">Whitelist</option>
              </SelectField>
              <SelectField
                label="Asset filter"
                value={source.assetFilter.type}
                onchange={(event) => selectAssetFilter(sourceIndex, event)}
                selectClass="h-9 py-1.5 text-xs"
              >
                <option value="Any">Any asset</option>
                <option value="Whitelist">Whitelist</option>
              </SelectField>
            </div>

            {#if source.sourceFilter.type === 'Whitelist'}
              <TextArea
                label="Sender whitelist"
                helper="One SS58 or 32-byte hex account per line. Canonical order is applied during lowering."
                value={source.sourceFilter.accounts.join('\n')}
                oninput={(event) =>
                  updateAccountWhitelist(
                    sourceIndex,
                    (event.currentTarget as HTMLTextAreaElement).value,
                  )}
                rows={3}
                textareaClass="font-mono text-xs"
              />
            {/if}

            {#if source.assetFilter.type === 'Whitelist'}
              <div class="grid gap-1.5">
                <div class="flex items-center justify-between gap-2">
                  <div class="text-[10px] text-(--mono-muted)">
                    Asset whitelist
                  </div>
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={source.assetFilter.assets.length >=
                      DEOS_AAA_AUTHORING_LIMITS.maxWhitelistSize}
                    onclick={() => addAsset(sourceIndex)}
                  >
                    <Plus size={12} /> Asset
                  </Button>
                </div>
                {#each source.assetFilter.assets as asset, assetIndex}
                  <div
                    class="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] gap-1.5"
                  >
                    <SelectField
                      label={`Asset ${assetIndex + 1}`}
                      value={asset.type}
                      onchange={(event) =>
                        selectAssetType(sourceIndex, assetIndex, event)}
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
                        onclick={() => removeAsset(sourceIndex, assetIndex)}
                      >
                        <Trash2 size={13} />
                      </IconButton>
                    </div>
                  </div>
                {/each}
              </div>
            {/if}
          {/if}
        </article>
      {/each}
    </div>
  {:else}
    <p class="rounded-xl bg-(--mono-bg) p-2.5 text-[10px] text-(--mono-muted)">
      Always admits once per fixed actor cadence and has no readiness source
      set.
    </p>
  {/if}
</section>
