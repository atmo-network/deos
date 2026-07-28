<!--
Domain: Typed observation inspection
Owns: Bounded feed selection, authored freshness window, current scalar state, and explicit trust warnings.
Excludes: Runtime queries, history, plan authoring, and trading recommendations.
Zone: Observation domain UI; receives provider capabilities from its composition root.
-->
<script lang="ts">
  import { formatObservationFeed } from '$lib/observation/inspection';
  import type {
    ObservationFeedIdentity,
    ObservationInspection,
  } from '$lib/observation/types';
  import type { ReadModelValue } from '$lib/read-model';
  import {
    Badge,
    DetailRow,
    Notice,
    NumberInput,
    ReadModelBadge,
    SelectField,
  } from '$lib/ui';

  type Props = {
    refreshKey: number;
    compact?: boolean;
    actorOptions?: readonly { aaaId: number; label: string }[];
    loadFeeds:
      | (() => Promise<ReadModelValue<ObservationFeedIdentity[]>>)
      | null;
    loadInspection:
      | ((
          feed: ObservationFeedIdentity,
          maxAgeBlocks: number,
          aaaId?: number,
        ) => Promise<ReadModelValue<ObservationInspection>>)
      | null;
  };

  let {
    refreshKey,
    compact = false,
    actorOptions = [],
    loadFeeds,
    loadInspection,
  }: Props = $props();
  let feeds = $state<ReadModelValue<ObservationFeedIdentity[]> | null>(null);
  let inspection = $state<ReadModelValue<ObservationInspection> | null>(null);
  let selectedFeedIndex = $state(0);
  let selectedActorId = $state('');
  let maxAgeBlocks = $state(100);
  let loadingFeeds = $state(false);
  let loadingInspection = $state(false);
  let error = $state<string | null>(null);

  const selectedFeed = $derived(feeds?.value[selectedFeedIndex] ?? null);

  function aggregationLabel(value: ObservationInspection) {
    return value.aggregation.type === 'LastValue'
      ? 'Last value'
      : `EMA · half-life ${value.aggregation.halfLifeBlocks} blocks`;
  }

  function selectFeed(event: Event) {
    selectedFeedIndex = Number(
      (event.currentTarget as HTMLSelectElement).value,
    );
  }

  function selectActor(event: Event) {
    selectedActorId = (event.currentTarget as HTMLSelectElement).value;
  }

  $effect(() => {
    refreshKey;
    if (!loadFeeds) {
      feeds = null;
      inspection = null;
      error = 'The current adapter cannot inspect canonical observations.';
      return;
    }
    let cancelled = false;
    loadingFeeds = true;
    error = null;
    void loadFeeds()
      .then((nextFeeds) => {
        if (cancelled) return;
        feeds = nextFeeds;
        selectedFeedIndex = Math.min(
          selectedFeedIndex,
          Math.max(nextFeeds.value.length - 1, 0),
        );
        loadingFeeds = false;
      })
      .catch((loadError) => {
        if (cancelled) return;
        error =
          loadError instanceof Error
            ? loadError.message
            : 'Observation registry read failed';
        loadingFeeds = false;
      });
    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    const feed = selectedFeed;
    const age = maxAgeBlocks;
    const actorId = selectedActorId === '' ? undefined : Number(selectedActorId);
    refreshKey;
    if (!feed || !loadInspection || age <= 0 || age > 0xffff_ffff) {
      inspection = null;
      return;
    }
    let cancelled = false;
    loadingInspection = true;
    error = null;
    void loadInspection(feed, age, actorId)
      .then((nextInspection) => {
        if (cancelled) return;
        inspection = nextInspection;
        loadingInspection = false;
      })
      .catch((loadError) => {
        if (cancelled) return;
        error =
          loadError instanceof Error
            ? loadError.message
            : 'Observation read failed';
        loadingInspection = false;
      });
    return () => {
      cancelled = true;
    };
  });
</script>

<div class="grid gap-3 p-3 text-xs">
  <Notice variant="warn">
    Local-pool observations come from DEOS Router pre-execution reserves. They
    are bounded execution references, not fair-price, manipulation-resistance,
    MEV-protection, or ordering proofs.
  </Notice>

  {#if error}
    <Notice variant="warn">{error}</Notice>
  {/if}

  <div class={compact ? 'grid gap-2' : 'grid grid-cols-3 gap-2'}>
    <SelectField
      label="Canonical feed"
      value={String(selectedFeedIndex)}
      onchange={selectFeed}
      selectClass="h-9 py-1.5 text-xs"
      disabled={loadingFeeds || !feeds || feeds.value.length === 0}
    >
      {#if feeds?.value.length}
        {#each feeds.value as feed, index (formatObservationFeed(feed))}
          <option value={index}>{formatObservationFeed(feed)}</option>
        {/each}
      {:else}
        <option value="0">No registered feeds</option>
      {/if}
    </SelectField>
    <SelectField
      label="Selected actor delivery"
      value={selectedActorId}
      onchange={selectActor}
      selectClass="h-9 py-1.5 text-xs"
    >
      <option value="">No actor selected</option>
      {#each actorOptions as actor (actor.aaaId)}
        <option value={actor.aaaId}>{actor.label} · AAA {actor.aaaId}</option>
      {/each}
    </SelectField>
    <NumberInput
      label="Authored maximum age (blocks)"
      min={1}
      max={4294967295}
      step={1}
      bind:value={maxAgeBlocks}
      class="h-9 py-1.5 text-xs tabnum"
    />
  </div>

  {#if loadingFeeds || loadingInspection}
    <div class="text-(--mono-muted)">Reading finalized observation state…</div>
  {:else if inspection}
    {@const value = inspection.value}
    <div
      class="grid gap-2 rounded-xl border border-(--mono-border) bg-white p-3"
    >
      <div class="flex flex-wrap items-center justify-between gap-2">
        <div>
          <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">
            Current scalar observation
          </div>
          <div class="font-medium text-(--mono-text)">
            {value.formattedValue ?? 'No initialized value'}
          </div>
        </div>
        <div class="flex flex-wrap items-center gap-1.5">
          <Badge variant={value.status === 'Fresh' ? 'tmc' : 'info'}>
            {value.status}
          </Badge>
          <ReadModelBadge provenance={inspection.provenance} tone="subtle" />
        </div>
      </div>

      <div class={compact ? 'grid gap-1' : 'grid grid-cols-2 gap-x-4 gap-y-1'}>
        <DetailRow
          label="Feed identity"
          value={formatObservationFeed(value.feed)}
        />
        <DetailRow
          label="Raw scalar"
          value={value.value?.toString() ?? 'Unavailable'}
          valueClass="tabnum"
        />
        <DetailRow
          label="Scale"
          value={`10^${value.scale}`}
          valueClass="tabnum"
        />
        <DetailRow label="Aggregation" value={aggregationLabel(value)} />
        <DetailRow
          label="Producer"
          value={value.producer ?? 'Unavailable'}
          valueClass="font-mono break-all"
        />
        <DetailRow label="Provenance" value={value.provenance} />
        <DetailRow label="Lifecycle" value={value.lifecycle} />
        <DetailRow
          label="Updated at"
          value={value.updatedAt == null ? 'Never' : `Block ${value.updatedAt}`}
          valueClass="tabnum"
        />
        <DetailRow
          label="Revision"
          value={value.revision?.toString() ?? 'None'}
          valueClass="tabnum"
        />
        <DetailRow
          label="Current age"
          value={value.ageBlocks == null
            ? 'Unavailable'
            : `${value.ageBlocks} blocks`}
          valueClass="tabnum"
        />
        <DetailRow
          label="Authored age"
          value={`${value.authoredMaxAgeBlocks} blocks`}
          valueClass="tabnum"
        />
        <DetailRow
          label="Historical owner"
          value="Materialized provider (not loaded here)"
        />
      </div>

      {#if value.delivery}
        {@const delivery = value.delivery}
        <div class="grid gap-2 border-t border-(--mono-border) pt-2">
          <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">
            Reactive delivery
          </div>
          <div
            class={compact
              ? 'grid gap-1'
              : 'grid grid-cols-2 gap-x-4 gap-y-1'}
          >
            <DetailRow label="Delivery status" value={delivery.status} />
            <DetailRow
              label="Latest / fanout revision"
              value={`${delivery.latestRevision?.toString() ?? 'None'} / ${delivery.fanoutRevision?.toString() ?? 'None'}`}
              valueClass="tabnum"
            />
            <DetailRow
              label="Exact dirty age"
              value={delivery.dirtyAgeBlocks == null
                ? 'Clean'
                : `${delivery.dirtyAgeBlocks} blocks`}
              valueClass="tabnum"
            />
            <DetailRow
              label="Active-list position"
              value={delivery.activeList.selectedPosition == null
                ? 'Not active'
                : `${delivery.activeList.selectedPosition} of ${delivery.activeList.count} (zero-based)`}
              valueClass="tabnum"
            />
            <DetailRow
              label="Fair cursor"
              value={delivery.activeList.cursor == null
                ? 'None'
                : formatObservationFeed(delivery.activeList.cursor)}
            />
            <DetailRow
              label="Head / tail"
              value={`${delivery.activeList.head == null ? 'None' : formatObservationFeed(delivery.activeList.head)} / ${delivery.activeList.tail == null ? 'None' : formatObservationFeed(delivery.activeList.tail)}`}
            />
            <DetailRow
              label="Next subscriber page"
              value={delivery.nextSubscriberPage?.toString() ?? 'None'}
              valueClass="tabnum"
            />
            <DetailRow
              label="Occupied / remaining pages"
              value={`${delivery.occupiedPageCount} / ${delivery.estimatedRemainingFanoutPages}`}
              valueClass="tabnum"
            />
            <DetailRow
              label="Estimated fanout blocks"
              value={`${delivery.estimatedRemainingBlocks} under ${delivery.budget.maxPagesPerBlock} pages/block`}
              valueClass="tabnum"
            />
            <DetailRow
              label="Budget evidence"
              value={`${delivery.budget.runtimeIdentity} · ${delivery.budget.weightIdentity}`}
              valueClass="font-mono break-all"
            />
          </div>
          {#if delivery.selectedActor}
            {@const actor = delivery.selectedActor}
            <div class="grid gap-1 rounded-lg bg-(--mono-surface) p-2">
              <div
                class="text-[10px] uppercase tracking-wider text-(--mono-muted)"
              >
                Selected actor admission
              </div>
              <div
                class={compact
                  ? 'grid gap-1'
                  : 'grid grid-cols-2 gap-x-4 gap-y-1'}
              >
                <DetailRow
                  label="AAA id / lane"
                  value={`${actor.aaaId.toString()} / ${actor.queueLane ?? 'Unavailable'}`}
                  valueClass="tabnum"
                />
                <DetailRow
                  label="Queue-admission status"
                  value={actor.queueAdmissionStatus}
                />
                <DetailRow
                  label="Pending signal"
                  value={actor.pendingSignal == null
                    ? 'Unavailable'
                    : actor.pendingSignal
                      ? 'Yes'
                      : 'No'}
                />
                <DetailRow
                  label="Queue ticket"
                  value={actor.queueTicket?.toString() ?? 'None'}
                  valueClass="tabnum"
                />
                <DetailRow
                  label="Wakeup block"
                  value={actor.wakeup == null
                    ? 'None'
                    : actor.wakeup.block.toString()}
                  valueClass="tabnum"
                />
                <DetailRow
                  label="Wakeup page / slot"
                  value={actor.wakeup == null
                    ? 'None'
                    : `${actor.wakeup.pageId.toString()} / ${actor.wakeup.slot}`}
                  valueClass="tabnum"
                />
              </div>
            </div>
          {/if}
          {#if delivery.estimateAssumptions.length > 0}
            <Notice variant="muted">
              Estimate assumptions: {delivery.estimateAssumptions.join(' ')}
            </Notice>
          {/if}
        </div>
      {/if}

      <Notice variant="muted">
        Observation-change signals coalesce to latest-state reconsideration.
        Equal-value refreshes may advance the update block without a revision;
        intermediate revisions and per-revision execution are not promised.
      </Notice>
    </div>
  {/if}
</div>
