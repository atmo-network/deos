<!--
Domain: Automation widget
Owns: System AAA actor snapshots plus typed linear plan authoring, validation, and exact artifact identity presentation.
Excludes: Runtime actor scheduling, submission authority, static weight models, simulation providers, and layout state.
Zone: Presentation widget; composes system projections, automation capabilities, and UI Kit helpers.
-->
<script lang="ts">
  import { Check, Copy, Plus } from '@lucide/svelte';
  import { onMount } from 'svelte';

  import AutomationStepEditor from '$lib/automation/AutomationStepEditor.svelte';
  import {
    type AaaAuthoringIssue,
    appendAaaStep,
    createAaaArtifactFromAuthoring,
    createAaaAuthoringProgram,
    createAaaAuthoringStep,
    moveAaaStep,
    removeAaaStep,
    validateAaaAuthoringProgram,
  } from '$lib/automation/authoring';
  import type { AaaPlanArtifact } from '$lib/automation/plan-artifact';
  import type {
    AutomationActorSnapshot,
    AutomationAuthoringContext,
  } from '$lib/automation/types';
  import { fromClientBoundedProjection } from '$lib/read-model';
  import { systemStore } from '$lib/system/index.svelte';
  import {
    Badge,
    Button,
    Card,
    DetailRow,
    Notice,
    NumberInput,
    SectionCard,
    SelectField,
  } from '$lib/ui';
  import { fmt, toFloat } from '$lib/ui/format';

  type AutomationView = 'actors' | 'compose';

  let rootEl = $state<HTMLDivElement | null>(null);
  let viewport = $state({ width: 0, height: 0 });
  let loading = $state(true);
  let error = $state<string | null>(null);
  let actors = $state<AutomationActorSnapshot[]>([]);
  let view = $state<AutomationView>('actors');
  let draft = $state(createAaaAuthoringProgram());
  let nextStepId = $state(2);
  let artifact = $state<AaaPlanArtifact | null>(null);
  let artifactContext = $state<AutomationAuthoringContext | null>(null);
  let boundDraftFingerprint = $state<string | null>(null);
  let artifactBusy = $state(false);
  let artifactMessage = $state<string | null>(null);
  let copiedPlanId = $state(false);

  const automationProvenance = fromClientBoundedProjection(
    true,
    'automationWidget <- AAA.ActorHot + AAA.ActorProgram + AAA.ContinuationState + System.Account',
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

  function draftFingerprint() {
    return JSON.stringify(draft);
  }

  const compactPane = $derived(viewport.width > 0 && viewport.width < 520);
  const densePane = $derived(viewport.width > 0 && viewport.width < 340);
  const validation = $derived.by(() => validateAaaAuthoringProgram(draft));
  const maxSteps = $derived(draft.aaaType === 'User' ? 3 : 10);
  const canAddStep = $derived(draft.steps.length < maxSteps);
  const rootIssues = $derived(
    validation.issues.filter((issue) => !issue.path.startsWith('steps[')),
  );

  function issuesForStep(index: number): AaaAuthoringIssue[] {
    return validation.issues.filter((issue) =>
      issue.path.startsWith(`steps[${index}]`),
    );
  }

  function selectAaaType(event: Event) {
    const aaaType = (event.currentTarget as HTMLSelectElement).value as
      | 'User'
      | 'System';
    draft.aaaType = aaaType;
    draft.fundingPolicy =
      aaaType === 'System' ? { type: 'RuntimePolicy' } : { type: 'OwnerOnly' };
  }

  function addStep() {
    if (!canAddStep) return;
    const key = `step-${nextStepId}`;
    nextStepId += 1;
    draft = appendAaaStep(draft, createAaaAuthoringStep(key));
  }

  function moveStep(index: number, direction: -1 | 1) {
    draft = moveAaaStep(draft, index, index + direction);
  }

  function deleteStep(key: string) {
    draft = removeAaaStep(draft, key);
  }

  async function bindArtifact() {
    artifactMessage = null;
    copiedPlanId = false;
    const currentValidation = validateAaaAuthoringProgram(draft);
    if (!currentValidation.valid) {
      artifactMessage = 'Resolve the visible validation findings first.';
      return;
    }
    const loadContext = systemStore.adapter.getAutomationAuthoringContext;
    if (!loadContext) {
      artifactMessage =
        'The current adapter cannot provide finalized runtime metadata.';
      return;
    }
    artifactBusy = true;
    try {
      const context = await loadContext.call(systemStore.adapter);
      const nextArtifact = createAaaArtifactFromAuthoring({
        program: draft,
        metadataBytes: context.metadataBytes,
        runtime: context.runtime,
      });
      boundDraftFingerprint = draftFingerprint();
      artifactContext = context;
      artifact = nextArtifact;
    } catch (bindError) {
      artifactMessage =
        bindError instanceof Error
          ? bindError.message
          : 'Exact artifact binding failed';
    } finally {
      artifactBusy = false;
    }
  }

  async function copyPlanId() {
    if (!artifact) return;
    await navigator.clipboard.writeText(artifact.planId);
    copiedPlanId = true;
  }

  function shortHash(value: string) {
    return `${value.slice(0, 10)}…${value.slice(-8)}`;
  }

  $effect(() => {
    const currentFingerprint = draftFingerprint();
    if (
      artifact !== null &&
      boundDraftFingerprint !== null &&
      currentFingerprint !== boundDraftFingerprint
    ) {
      artifact = null;
      artifactContext = null;
      boundDraftFingerprint = null;
      copiedPlanId = false;
      artifactMessage =
        'Draft changed. Rebind to finalized metadata for a new exact identity.';
    }
  });

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
        if (cancelled) return;
        actors = nextActors;
        loading = false;
      })
      .catch((refreshError) => {
        if (cancelled) return;
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
    if (!rootEl) return;
    const resizeObserver = new ResizeObserver(() => syncViewport());
    resizeObserver.observe(rootEl);
    return () => resizeObserver.disconnect();
  });
</script>

<Card class="min-h-full flex flex-col">
  <div bind:this={rootEl} class="h-full min-h-0">
    <header
      class={densePane
        ? 'grid gap-2 border-b border-(--mono-border) p-2'
        : 'flex flex-wrap items-center justify-between gap-2 border-b border-(--mono-border) p-3'}
    >
      <div>
        <div class="text-sm font-semibold text-(--mono-text)">Automation</div>
        <div class="text-[10px] text-(--mono-muted)">
          Live actors and verifiable straight-line plans
        </div>
      </div>
      <div
        class="grid grid-cols-2 gap-1 rounded-xl bg-(--mono-bg) p-1"
        aria-label="Automation view"
      >
        <Button
          size="sm"
          variant={view === 'actors' ? 'primary' : 'ghost'}
          aria-pressed={view === 'actors'}
          onclick={() => (view = 'actors')}
        >
          Actors
        </Button>
        <Button
          size="sm"
          variant={view === 'compose' ? 'primary' : 'ghost'}
          aria-pressed={view === 'compose'}
          onclick={() => (view = 'compose')}
        >
          Compose
        </Button>
      </div>
    </header>

    {#if view === 'actors'}
      <div class="grid gap-3 p-3 text-xs">
        {#if loading}
          <div class="text-(--mono-muted)">Loading automation…</div>
        {:else if error}
          <Notice variant="warn">{error}</Notice>
        {:else}
          {#each actors as actor (actor.aaaId)}
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
                  <div class="font-medium text-(--mono-text)">
                    {actor.label}
                  </div>
                  <div class="text-[10px] text-(--mono-muted)">
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
                  {#if !actor.exists}
                    missing
                  {:else if actor.paused}
                    paused
                  {:else if actor.runState === 'suspended'}
                    suspended
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
                    label="Run"
                    value={actor.continuation
                      ? `#${actor.cycleNonce} · try ${actor.continuation.attempt} · step ${actor.continuation.cursor + 1}`
                      : `#${actor.cycleNonce}`}
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
                    label="Logical run"
                    value={`#${actor.cycleNonce}`}
                    valueClass="tabnum text-(--mono-text)"
                  />
                  <DetailRow
                    label="Continuation"
                    value={actor.continuation
                      ? `Attempt ${actor.continuation.attempt} · step ${actor.continuation.cursor + 1} · block ${actor.continuation.lastAttemptBlock}`
                      : 'None'}
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
    {:else}
      <div class="grid gap-3 p-3 text-xs">
        <Notice variant="muted">
          Authoring produces an inspectable artifact only. It never submits,
          schedules, or changes an actor.
        </Notice>

        <SectionCard
          title="Plan context"
          subtitle="Choose the actor class; this surface keeps trigger and funding policy explicit but narrow."
        >
          <div class={compactPane ? 'grid gap-2' : 'grid grid-cols-3 gap-2'}>
            <SelectField
              label="AAA class"
              value={draft.aaaType}
              onchange={selectAaaType}
              selectClass="h-9 py-1.5 text-xs"
            >
              <option value="User">User AAA</option>
              <option value="System">System AAA</option>
            </SelectField>
            <SelectField
              label="Mutability"
              bind:value={draft.mutability}
              selectClass="h-9 py-1.5 text-xs"
            >
              <option value="Mutable">Mutable</option>
              <option value="Immutable">Immutable</option>
            </SelectField>
            <NumberInput
              label="Cooldown (blocks)"
              min={0}
              max={4294967295}
              step={1}
              bind:value={draft.cooldownBlocks}
              class="h-9 py-1.5 text-xs tabnum"
            />
          </div>
          <div
            class={compactPane
              ? 'grid gap-1 rounded-xl bg-(--mono-bg) p-2.5 text-[10px]'
              : 'grid grid-cols-2 gap-x-4 gap-y-1 rounded-xl bg-(--mono-bg) p-2.5 text-[10px]'}
          >
            <DetailRow
              label="Trigger"
              value="Manual"
              valueClass="text-(--mono-text)"
            />
            <DetailRow
              label="Funding"
              value={draft.fundingPolicy.type}
              valueClass="text-(--mono-text)"
            />
          </div>
        </SectionCard>

        <div class="flex flex-wrap items-end justify-between gap-2">
          <div>
            <div
              class="text-[10px] uppercase tracking-wider text-(--mono-muted)"
            >
              Ordered execution plan
            </div>
            <div class="text-xs text-(--mono-text)">
              {draft.steps.length}/{maxSteps} steps · fixed cursor order
            </div>
          </div>
          <Button
            size="sm"
            variant="secondary"
            onclick={addStep}
            disabled={!canAddStep}
            class="inline-flex items-center gap-1"
          >
            <Plus size={13} /> Add step
          </Button>
        </div>

        <div class="grid gap-2">
          {#each draft.steps as step, index (step.key)}
            <AutomationStepEditor
              bind:step={draft.steps[index]}
              {index}
              total={draft.steps.length}
              aaaType={draft.aaaType}
              mutability={draft.mutability}
              compact={compactPane}
              issues={issuesForStep(index)}
              onMove={(direction) => moveStep(index, direction)}
              onRemove={() => deleteStep(step.key)}
            />
          {/each}
        </div>

        {#if rootIssues.length > 0}
          <Notice variant="warn">
            <div class="grid gap-1">
              {#each rootIssues as issue}
                <div>{issue.message}</div>
              {/each}
            </div>
          </Notice>
        {/if}

        <SectionCard
          title="Exact artifact"
          subtitle="Validate locally, then bind the draft to metadata and runtime identity at one finalized block."
        >
          <div
            class={compactPane
              ? 'grid gap-2'
              : 'flex flex-wrap items-center justify-between gap-2'}
          >
            <div class="flex items-center gap-2">
              <Badge variant={validation.valid ? 'tmc' : 'info'}>
                {validation.valid
                  ? 'Structurally valid'
                  : `${validation.issues.length} finding${validation.issues.length === 1 ? '' : 's'}`}
              </Badge>
              <span class="text-[10px] text-(--mono-muted)">
                Canonical SCALE remains unbound until requested.
              </span>
            </div>
            <Button
              variant="primary"
              size="sm"
              onclick={bindArtifact}
              disabled={!validation.valid || artifactBusy}
            >
              {artifactBusy ? 'Binding…' : 'Bind exact artifact'}
            </Button>
          </div>

          {#if artifactMessage}
            <Notice variant="warn">{artifactMessage}</Notice>
          {/if}

          {#if artifact && artifactContext}
            <div class="grid gap-2 rounded-xl bg-(--mono-bg) p-2.5">
              <div class="flex flex-wrap items-center justify-between gap-2">
                <div>
                  <div
                    class="text-[10px] uppercase tracking-wider text-(--mono-muted)"
                  >
                    Plan ID
                  </div>
                  <code class="break-all text-[10px] text-(--mono-text)">
                    {artifact.planId}
                  </code>
                </div>
                <Button
                  size="sm"
                  variant="ghost"
                  onclick={copyPlanId}
                  class="inline-flex shrink-0 items-center gap-1"
                >
                  {#if copiedPlanId}
                    <Check size={12} /> Copied
                  {:else}
                    <Copy size={12} /> Copy
                  {/if}
                </Button>
              </div>
              <div
                class={compactPane
                  ? 'grid gap-1 text-[10px]'
                  : 'grid grid-cols-2 gap-x-4 gap-y-1 text-[10px]'}
              >
                <DetailRow
                  label="Metadata"
                  value={shortHash(artifact.metadataHash)}
                  valueClass="font-mono text-(--mono-text)"
                />
                <DetailRow
                  label="Genesis"
                  value={shortHash(artifact.genesisHash)}
                  valueClass="font-mono text-(--mono-text)"
                />
                <DetailRow
                  label="Runtime"
                  value={`spec ${artifact.specVersion} · tx ${artifact.transactionVersion}`}
                  valueClass="tabnum text-(--mono-text)"
                />
                <DetailRow
                  label="Finalized pin"
                  value={`#${artifactContext.finalizedBlock.number} · ${shortHash(artifactContext.finalizedBlock.hash)}`}
                  valueClass="font-mono text-(--mono-text)"
                />
                <DetailRow
                  label="Program SCALE"
                  value={`${(artifact.programScale.length - 2) / 2} bytes`}
                  valueClass="tabnum text-(--mono-text)"
                />
                <DetailRow
                  label="Format"
                  value={`${artifact.format}@${artifact.formatVersion}`}
                  valueClass="font-mono text-(--mono-text)"
                />
              </div>
            </div>
          {/if}
        </SectionCard>

        <SectionCard
          title="Evidence lanes"
          subtitle="Different engines answer different questions; absence never inherits another lane's truth."
        >
          <div class={compactPane ? 'grid gap-2' : 'grid grid-cols-3 gap-2'}>
            <div class="grid gap-1 rounded-xl border bg-white p-2.5">
              <div class="flex items-center justify-between gap-2">
                <span class="font-medium text-(--mono-text)">Forecast</span>
                <Badge variant="info">Not run</Badge>
              </div>
              <div class="text-[10px] leading-relaxed text-(--mono-muted)">
                Requires the pinned production weight/fee model and reports
                RefTime, ProofSize, and fee bounds separately.
              </div>
            </div>
            <div class="grid gap-1 rounded-xl border bg-white p-2.5">
              <div class="flex items-center justify-between gap-2">
                <span class="font-medium text-(--mono-text)">Adapter-local</span
                >
                <Badge variant="info">Not run</Badge>
              </div>
              <div class="text-[10px] leading-relaxed text-(--mono-muted)">
                Requires an explicit local state model. It remains a projection,
                never matching-runtime truth.
              </div>
            </div>
            <div class="grid gap-1 rounded-xl border bg-white p-2.5">
              <div class="flex items-center justify-between gap-2">
                <span class="font-medium text-(--mono-text)">Matching Wasm</span
                >
                <Badge variant="info">Not run</Badge>
              </div>
              <div class="text-[10px] leading-relaxed text-(--mono-muted)">
                Requires an existing actor and finalized runtime/state pin; the
                runtime API remains the authoritative execution check.
              </div>
            </div>
          </div>
        </SectionCard>
      </div>
    {/if}
  </div>
</Card>
