<!--
Domain: Automation widget
Owns: System AAA actor snapshot presentation, automation health cards, and bounded automation read-model display.
Excludes: Runtime actor scheduling, system store ownership, adapter transport, and layout state.
Zone: Presentation widget; consumes system automation projections and UI Kit helpers.
-->
<script lang="ts">
  import type { AutomationActorSnapshot } from '$lib/automation/types';
  import { resolveChainSurfaceState } from '$lib/system/connection-surface';
  import { systemStore } from '$lib/system/index.svelte';
  import {
    BackButton,
    Badge,
    Button,
    Card,
    Notice,
    SectionCard,
    StatCard,
  } from '$lib/ui';
  import { fmt, toFloat } from '$lib/ui/format';

  let selectedActorId = $state<number | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let actors = $state<AutomationActorSnapshot[]>([]);
  let hasActorSnapshot = $state(false);

  const chainSurface = $derived(
    resolveChainSurfaceState(systemStore.connectionState, hasActorSnapshot),
  );
  const selectedActor = $derived(
    actors.find((actor) => actor.aaaId === selectedActorId) ?? null,
  );
  const actorHealth = $derived.by(() => ({
    live: actors.filter(
      (actor) =>
        actor.exists && !actor.paused && actor.runState !== 'suspended',
    ).length,
    suspended: actors.filter(
      (actor) =>
        actor.exists && !actor.paused && actor.runState === 'suspended',
    ).length,
    paused: actors.filter((actor) => actor.exists && actor.paused).length,
    missing: actors.filter((actor) => !actor.exists).length,
  }));

  $effect(() => {
    if (selectedActorId !== null && !selectedActor) {
      selectedActorId = null;
    }
  });

  $effect(() => {
    systemStore.snapshot?.blockNumber;
    const connectionStatus = systemStore.connectionState?.status;
    if (connectionStatus !== 'connected') {
      loading = false;
      error = null;
      return;
    }

    const adapter = systemStore.adapter;
    if (!adapter.getAutomationActors) {
      actors = [];
      hasActorSnapshot = false;
      loading = false;
      error = 'The connected adapter does not expose System AAA Actor state.';
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
        hasActorSnapshot = true;
        loading = false;
      })
      .catch((refreshError) => {
        if (cancelled) {
          return;
        }
        error =
          refreshError instanceof Error
            ? refreshError.message
            : 'System AAA Actor refresh failed';
        loading = false;
      });
    return () => {
      cancelled = true;
    };
  });
</script>

<Card class="h-full min-h-full flex flex-col">
  <div
    class="automation-container grid h-full min-h-0 gap-3 p-3 text-xs [container-type:size]"
  >
    {#if error && systemStore.connectionState?.status === 'connected' && !hasActorSnapshot}
      <SectionCard title="Automation unavailable">
        <Notice variant="warn">{error}</Notice>
      </SectionCard>
    {:else}
      {#if chainSurface.status === 'stale' || chainSurface.status === 'preview'}
        <Notice variant="warn" class="grid gap-0.5">
          <strong>{chainSurface.title}</strong>
          <span>{chainSurface.detail}</span>
        </Notice>
      {/if}
      {#if error}
        <Notice variant="warn">Actor refresh failed: {error}</Notice>
      {/if}
      {#if selectedActor}
        <section
          class="actor-detail grid w-full max-w-3xl justify-self-center content-start gap-3 rounded-xl bg-(--mono-bg) p-3"
        >
          <div class="flex min-w-0 items-center gap-2">
            <BackButton
              onclick={() => (selectedActorId = null)}
              label="Back to actors"
            />
            <div class="min-w-0 flex-1">
              <div class="truncate text-base font-semibold text-(--mono-text)">
                {selectedActor.label}
              </div>
              <div class="truncate text-2xs text-(--mono-muted)">
                {selectedActor.role}
              </div>
            </div>
            <Badge
              variant={selectedActor.exists
                ? selectedActor.paused
                  ? 'info'
                  : selectedActor.runState === 'suspended'
                    ? 'xyk'
                    : 'tmc'
                : 'info'}
            >
              {selectedActor.exists
                ? selectedActor.paused
                  ? 'paused'
                  : selectedActor.runState === 'suspended'
                    ? 'suspended'
                    : 'live'
                : 'missing'}
            </Badge>
          </div>
          <div
            class="evidence-grid grid grid-cols-[repeat(auto-fit,minmax(min(100%,calc(var(--widget-em)*17.0667)),1fr))] gap-2"
          >
            <StatCard label="Trigger" value={selectedActor.triggerLabel} />
            <StatCard
              label="Logical run"
              value={`#${selectedActor.cycleNonce}`}
            />
            <StatCard
              label="Continuation"
              value={selectedActor.continuation
                ? `Attempt ${selectedActor.continuation.attempt} · step ${selectedActor.continuation.cursor + 1} · block ${selectedActor.continuation.lastAttemptBlock}`
                : 'None'}
            />
            <StatCard
              label="Last cycle"
              value={selectedActor.lastCycleBlock?.toString() ?? '—'}
            />
            <StatCard
              label="Native balance"
              value={`${fmt(toFloat(selectedActor.nativeBalance))} ${systemStore.snapshot?.nativeAsset.symbol ?? 'NTVE'}`}
            />
          </div>
        </section>
      {:else}
        <SectionCard title="Actor health" class="automation-health">
          {#if loading}
            <Notice>Loading automation…</Notice>
          {:else if error}
            <Notice variant="warn">{error}</Notice>
          {:else}
            <div
              class="grid grid-cols-[repeat(auto-fit,minmax(min(100%,7rem),1fr))] gap-2"
            >
              <StatCard
                label="Live"
                value={hasActorSnapshot ? actorHealth.live.toString() : '—'}
                toneClass="text-(--mono-green)"
              />
              <StatCard
                label="Suspended"
                value={hasActorSnapshot
                  ? actorHealth.suspended.toString()
                  : '—'}
                toneClass="text-(--mono-orange)"
              />
              <StatCard
                label="Paused"
                value={hasActorSnapshot ? actorHealth.paused.toString() : '—'}
                toneClass="text-(--mono-orange)"
              />
              <StatCard
                label="Missing"
                value={hasActorSnapshot ? actorHealth.missing.toString() : '—'}
                toneClass="text-(--mono-muted)"
              />
            </div>
          {/if}
        </SectionCard>

        {#if !loading && !error && hasActorSnapshot}
          {#if actors.length === 0}
            <Notice>No System AAA Actors exposed</Notice>
          {:else}
            <div
              class="actor-grid grid grid-cols-[repeat(auto-fit,minmax(min(100%,calc(var(--widget-em)*17.0667)),1fr))] gap-2"
            >
              {#each actors as actor}
                <article
                  class="actor-row flex min-w-0 items-center gap-3 rounded-xl bg-(--mono-bg) px-3 py-2"
                >
                  <div class="min-w-0 flex-1">
                    <div class="truncate font-medium text-(--mono-text)">
                      {actor.label}
                    </div>
                    <div
                      class="actor-role truncate text-2xs text-(--mono-muted)"
                    >
                      {actor.role}
                    </div>
                  </div>
                  <Badge
                    variant={actor.exists
                      ? actor.paused
                        ? 'info'
                        : actor.runState === 'suspended'
                          ? 'xyk'
                          : 'tmc'
                      : 'info'}
                  >
                    {actor.exists
                      ? actor.paused
                        ? 'paused'
                        : actor.runState === 'suspended'
                          ? 'suspended'
                          : 'live'
                      : 'missing'}
                  </Badge>
                  <Button
                    size="sm"
                    variant="ghost"
                    class="shrink-0 text-(--mono-purple)"
                    onclick={() => (selectedActorId = actor.aaaId)}
                  >
                    View
                  </Button>
                </article>
              {/each}
            </div>
          {/if}
        {/if}
      {/if}
    {/if}
  </div>
</Card>

<style>
  @container (max-height: 176px) {
    :global(.automation-health) {
      display: none;
    }
    .actor-grid {
      align-self: center;
    }
    .actor-role {
      display: none;
    }
    .actor-detail {
      align-self: center;
      padding: calc(var(--spacing) * 2);
    }
    .evidence-grid {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }
</style>
