<!--
Domain: Workspace frame
Owns: Top-level responsive layout shell, reserved edge lanes, sidebar placement, and center tile host composition.
Excludes: Widget internals, domain store behavior, and transport/provider wiring.
Zone: Layout composition root under web-client; may import layout primitives and special lane widgets.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import type { Component } from 'svelte';
  import { cubicOut } from 'svelte/easing';
  import { fly } from 'svelte/transition';

  import type { TileNode } from '$lib/layout/types';

  import AppFooter from './AppFooter.svelte';
  import AppHeader from './AppHeader.svelte';
  import SidebarPanelSkeleton from './SidebarPanelSkeleton.svelte';
  import TileContainer from './TileContainer.svelte';
  import { layoutStore } from './index.svelte';
  import { MOBILE_LAYOUT_BREAKPOINT } from './types';

  type SidebarPanelComponent = Component<{
    id?: string;
    mobile: boolean;
    onclose: () => void;
  }>;

  type Props = {
    root: TileNode;
  };

  const SIDEBAR_PANEL_ID = 'deos-sidebar-panel';
  const SIDEBAR_STACK_BREAKPOINT = 900;
  const SIDEBAR_WIDTH = '22rem';
  const SIDEBAR_TRANSITION_MS = 180;
  const SIDEBAR_DESKTOP_OFFSET = 24;
  const SIDEBAR_STACK_OFFSET = 16;
  const DESKTOP_SIDEBAR_GAP = '0.75rem';

  let { root }: Props = $props();
  let frameEl = $state<HTMLDivElement | null>(null);
  let stackSidebar = $state(false);
  let sidebarPanelComponent = $state<SidebarPanelComponent | null>(null);

  const sidebarOpen = $derived(layoutStore.frame.sidebar.open);
  const sidebarEdge = $derived(layoutStore.frame.sidebar.edge);
  const SidebarPanel = $derived(sidebarPanelComponent);
  const mobile = $derived.by(() => {
    if (!frameEl) {
      return false;
    }
    return frameEl.clientWidth < MOBILE_LAYOUT_BREAKPOINT;
  });

  function syncFrameMode(): void {
    if (!frameEl) {
      stackSidebar = false;
      return;
    }
    stackSidebar = frameEl.clientWidth < SIDEBAR_STACK_BREAKPOINT;
  }

  function toggleSidebar(): void {
    layoutStore.toggleSidebar();
  }

  function closeSidebar(): void {
    layoutStore.setSidebarOpen(false);
  }

  async function ensureSidebarPanelLoaded(): Promise<void> {
    if (sidebarPanelComponent !== null) {
      return;
    }
    const module = await import('./SidebarPanel.svelte');
    sidebarPanelComponent = module.default;
  }

  function sidebarFly(edge: 'left' | 'right', stacked: boolean) {
    const offset = stacked ? SIDEBAR_STACK_OFFSET : SIDEBAR_DESKTOP_OFFSET;
    return {
      x: stacked ? 0 : edge === 'left' ? -offset : offset,
      y: stacked ? (edge === 'left' ? -offset : offset) : 0,
      duration: SIDEBAR_TRANSITION_MS,
      opacity: 0,
      easing: cubicOut,
    };
  }

  function desktopSidebarWidth(): string {
    return sidebarOpen ? SIDEBAR_WIDTH : '0rem';
  }

  function desktopSidebarGap(): string {
    return sidebarOpen ? DESKTOP_SIDEBAR_GAP : '0rem';
  }

  onMount(() => {
    syncFrameMode();
    if (!frameEl) {
      return;
    }
    const resizeObserver = new ResizeObserver(() => syncFrameMode());
    resizeObserver.observe(frameEl);
    return () => resizeObserver.disconnect();
  });

  $effect(() => {
    if (!sidebarOpen) {
      return;
    }
    void ensureSidebarPanelLoaded();
  });
</script>

<div
  bind:this={frameEl}
  class="h-screen flex flex-col overflow-hidden bg-transparent"
>
  <AppHeader
    {sidebarEdge}
    sidebarId={SIDEBAR_PANEL_ID}
    {mobile}
    {sidebarOpen}
    onToggleSidebar={toggleSidebar}
  />

  {#if stackSidebar}
    {#if sidebarOpen && sidebarEdge === 'left'}
      <section
        class="shrink-0 overflow-hidden px-3 pt-3"
        transition:fly={sidebarFly('left', true)}
      >
        {#if SidebarPanel}
          <SidebarPanel id={SIDEBAR_PANEL_ID} {mobile} onclose={closeSidebar} />
        {:else}
          <SidebarPanelSkeleton />
        {/if}
      </section>
    {/if}
    <main class="flex-1 min-h-0 p-3 pt-3">
      <div class="h-full w-full min-h-0">
        <TileContainer node={root} root />
      </div>
    </main>
    {#if sidebarOpen && sidebarEdge === 'right'}
      <section
        class="shrink-0 overflow-hidden px-3 pb-3"
        transition:fly={sidebarFly('right', true)}
      >
        {#if SidebarPanel}
          <SidebarPanel id={SIDEBAR_PANEL_ID} {mobile} onclose={closeSidebar} />
        {:else}
          <SidebarPanelSkeleton />
        {/if}
      </section>
    {/if}
  {:else}
    <main class="flex-1 min-h-0 p-3 pt-3">
      <div
        class="flex h-full w-full min-h-0 transition-[gap] ease-out"
        style:gap={desktopSidebarGap()}
        style:transition-duration={`${SIDEBAR_TRANSITION_MS}ms`}
      >
        {#if sidebarEdge === 'left'}
          <div
            class="relative shrink-0 min-w-0 min-h-0 overflow-hidden transition-[width] ease-out"
            style:width={desktopSidebarWidth()}
            style:transition-duration={`${SIDEBAR_TRANSITION_MS}ms`}
          >
            <div
              class="absolute inset-y-0 left-0 min-h-0"
              class:pointer-events-none={!sidebarOpen}
              style:width={SIDEBAR_WIDTH}
              aria-hidden={!sidebarOpen}
              inert={!sidebarOpen}
            >
              {#if SidebarPanel}
                <SidebarPanel
                  id={SIDEBAR_PANEL_ID}
                  {mobile}
                  onclose={closeSidebar}
                />
              {:else}
                <SidebarPanelSkeleton />
              {/if}
            </div>
          </div>
        {/if}

        <div class="min-w-0 min-h-0 flex-1">
          <TileContainer node={root} root />
        </div>

        {#if sidebarEdge === 'right'}
          <div
            class="relative shrink-0 min-w-0 min-h-0 overflow-hidden transition-[width] ease-out"
            style:width={desktopSidebarWidth()}
            style:transition-duration={`${SIDEBAR_TRANSITION_MS}ms`}
          >
            <div
              class="absolute inset-y-0 right-0 min-h-0"
              class:pointer-events-none={!sidebarOpen}
              style:width={SIDEBAR_WIDTH}
              aria-hidden={!sidebarOpen}
              inert={!sidebarOpen}
            >
              {#if SidebarPanel}
                <SidebarPanel
                  id={SIDEBAR_PANEL_ID}
                  {mobile}
                  onclose={closeSidebar}
                />
              {:else}
                <SidebarPanelSkeleton />
              {/if}
            </div>
          </div>
        {/if}
      </div>
    </main>
  {/if}

  <AppFooter {mobile} />
</div>
