<!--
Domain: Automation widget
Owns: System AAA actor snapshot presentation, automation health cards, and bounded automation read-model display.
Excludes: Runtime actor scheduling, system store ownership, adapter transport, and layout state.
Zone: Presentation widget; consumes system automation projections and UI Kit helpers.
-->
<script lang="ts">
  import { onMount } from 'svelte';

  import type { AutomationActorSnapshot } from '$lib/automation/types';
  import { fromClientBoundedProjection } from '$lib/read-model';
  import { systemStore } from '$lib/system/index.svelte';
  import { Badge, Card, DetailRow, Notice } from '$lib/ui';
  import { fmt, toFloat } from '$lib/ui/format';

  let rootEl = $state<HTMLDivElement | null>(null);
  let viewport = $state({ width: 0, height: 0 });
  let loading = $state(true);
  let error = $state<string | null>(null);
  let actors = $state<AutomationActorSnapshot[]>([]);

  const automationProvenance = fromClientBoundedProjection(
    true,
    'automationWidget <- AAA.ActorHot + AAA.ActorProgram + System.Account',
  ).provenance;

  function syncViewport() {
    if (!rootEl) {
      viewport = { width: 0, height: 0 };
      return;
    }
    viewport = {
      width: rootEl.clientWidth,
      height: rootEl.clientHeight,
    };
  }

  const compactPane = $derived(viewport.width > 0 && viewport.width < 430);
  const densePane = $derived(viewport.width > 0 && viewport.width < 340);

  $effect(() => {
    systemStore.snapshot?.blockNumber;
    const adapter = systemStore.adapter;
    if (!adapter.getAutomationActors) {
      actors = [];
      loading = false;
      error = 'Automation surface not available in the current adapter';
      return;
    }
    loading = true;
    error = null;
    let cancelled = false;
    void Promise.resolve(adapter.getAutomationActors())
      .then((nextActors) => {
        if (cancelled) {
          return;
        }
        actors = nextActors;
        loading = false;
      })
      .catch((refreshError) => {
        if (cancelled) {
          return;
        }
        error =
          refreshError instanceof Error
            ? refreshError.message
            : 'Actor refresh failed';
        loading = false;
      });
    return () => {
      cancelled = true;
    };
  });

  onMount(() => {
    syncViewport();
    if (!rootEl) {
      return;
    }
    const resizeObserver = new ResizeObserver(() => syncViewport());
    resizeObserver.observe(rootEl);
    return () => resizeObserver.disconnect();
  });
</script>

<Card class="min-h-full flex flex-col">
  <div bind:this={rootEl} class="h-full flex flex-col min-h-0">
    <div class="grid gap-3 p-3 text-xs">
      {#if loading}
        <div class="text-(--mono-muted)">Loading automation…</div>
      {:else if error}
        <Notice variant="warn">{error}</Notice>
      {:else}
        {#each actors as actor}
          <div
            class={[
              'rounded-xl border bg-white',
              densePane ? 'grid gap-2 p-2' : 'grid gap-2 p-3',
            ]}
          >
            <div
              class={[
                densePane
                  ? 'grid gap-1'
                  : 'flex flex-wrap items-start justify-between gap-2',
              ]}
            >
              <div>
                <div class="font-medium text-(--mono-text)">{actor.label}</div>
                <div class="text-[10px] text-(--mono-muted)">{actor.role}</div>
              </div>
              <Badge
                variant={actor.exists
                  ? actor.paused
                    ? 'info'
                    : 'tmc'
                  : 'info'}
              >
                {#if !actor.exists}
                  missing
                {:else if actor.paused}
                  paused
                {:else}
                  live
                {/if}
              </Badge>
            </div>
            {#if compactPane}
              <div
                class="grid gap-1 rounded-xl border bg-(--mono-bg) px-2.5 py-2 text-[10px] text-(--mono-muted)"
              >
                <DetailRow
                  label="Trigger"
                  value={actor.triggerLabel}
                  valueClass="text-(--mono-text)"
                />
                <DetailRow
                  label="Cycle"
                  value={actor.lastCycleBlock?.toString() ?? '—'}
                  valueClass="tabnum text-(--mono-text)"
                />
                <DetailRow
                  label="Balance"
                  value={`${fmt(toFloat(actor.nativeBalance))} ${systemStore.snapshot?.nativeAsset.symbol ?? 'NTVE'}`}
                  valueClass="tabnum text-(--mono-text)"
                />
              </div>
            {:else}
              <div class="grid gap-1 text-[10px] text-(--mono-muted)">
                <DetailRow
                  label="Trigger"
                  value={actor.triggerLabel}
                  valueClass="text-(--mono-text)"
                />
                <DetailRow
                  label="Last cycle"
                  value={actor.lastCycleBlock?.toString() ?? '—'}
                  valueClass="tabnum text-(--mono-text)"
                />
                <DetailRow
                  label="Native balance"
                  value={`${fmt(toFloat(actor.nativeBalance))} ${systemStore.snapshot?.nativeAsset.symbol ?? 'NTVE'}`}
                  valueClass="tabnum text-(--mono-text)"
                />
              </div>
            {/if}
          </div>
        {/each}
      {/if}
    </div>
  </div>
</Card>
