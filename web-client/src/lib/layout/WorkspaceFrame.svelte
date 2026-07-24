<!--
Domain: Workspace frame
Owns: Top-level responsive layout shell, reserved edge lanes, sidebar placement, and center tile host composition.
Excludes: Widget internals, domain store behavior, and transport/provider wiring.
Zone: Layout composition root under web-client; may import layout primitives and special lane widgets.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import type { Component } from 'svelte';

  import type { TileNode } from '$lib/layout/types';
  import { subscribeToWidgetDeepLinks } from '$lib/navigation/hash-navigation';

  import AppFooter from './AppFooter.svelte';
  import AppHeader from './AppHeader.svelte';
  import MobileSidebarSheet from './MobileSidebarSheet.svelte';
  import SidebarPanelSkeleton from './SidebarPanelSkeleton.svelte';
  import TileContainer from './TileContainer.svelte';
  import { layoutStore } from './index.svelte';
  import { MOBILE_LAYOUT_BREAKPOINT } from './types';

  type SidebarPanelComponent = Component<{
    id?: string;
    mobile: boolean;
    crossContainerDragEnabled?: boolean;
  }>;

  type Props = {
    root: TileNode;
  };

  const SIDEBAR_PANEL_ID = 'deos-sidebar-panel';
  const SIDEBAR_OVERLAY_BREAKPOINT = 900;
  const SIDEBAR_WIDTH = '22rem';
  const SIDEBAR_TRANSITION_MS = 180;
  const DESKTOP_SIDEBAR_GAP = '0.75rem';

  let { root }: Props = $props();
  let frameEl = $state<HTMLDivElement | null>(null);
  let overlaySidebar = $state(false);
  let mobile = $state(false);
  let sidebarPanelComponent = $state<SidebarPanelComponent | null>(null);

  const sidebarOpen = $derived(layoutStore.frame.sidebar.open);
  const SidebarPanel = $derived(sidebarPanelComponent);

  function syncFrameMode(): void {
    if (!frameEl) {
      overlaySidebar = false;
      mobile = false;
      return;
    }
    overlaySidebar = frameEl.clientWidth < SIDEBAR_OVERLAY_BREAKPOINT;
    mobile = frameEl.clientWidth < MOBILE_LAYOUT_BREAKPOINT;
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

  function desktopSidebarWidth(): string {
    return sidebarOpen ? SIDEBAR_WIDTH : '0rem';
  }

  function desktopSidebarGap(): string {
    return sidebarOpen ? DESKTOP_SIDEBAR_GAP : '0rem';
  }

  onMount(() => {
    const unsubscribeDeepLinks = subscribeToWidgetDeepLinks((link) => {
      if (link?.widget === 'wiki') {
        layoutStore.activatePanel('wiki');
        layoutStore.setMobileExpandedPanel('wiki');
      } else if (link?.widget === 'swap') {
        layoutStore.activatePanel('swap');
        layoutStore.setMobileExpandedPanel('swap');
      }
    });
    syncFrameMode();
    if (!frameEl) {
      return unsubscribeDeepLinks;
    }
    const resizeObserver = new ResizeObserver(() => syncFrameMode());
    resizeObserver.observe(frameEl);
    return () => {
      unsubscribeDeepLinks();
      resizeObserver.disconnect();
    };
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
  class="relative h-screen flex flex-col overflow-hidden bg-transparent"
>
  <div class={mobile ? 'absolute inset-x-0 top-0 z-30' : 'contents'}>
    <AppHeader
      sidebarId={SIDEBAR_PANEL_ID}
      {mobile}
      {sidebarOpen}
      sidebarPlacement={overlaySidebar ? 'bottom' : 'right'}
      onToggleSidebar={toggleSidebar}
    />
  </div>

  {#if overlaySidebar}
    <main
      class={mobile ? 'absolute inset-0 min-h-0' : 'flex-1 min-h-0 p-3 pt-3'}
    >
      <div class="h-full w-full min-h-0">
        <TileContainer node={root} root />
      </div>
    </main>
    <MobileSidebarSheet
      id={SIDEBAR_PANEL_ID}
      open={sidebarOpen}
      panelComponent={SidebarPanel}
      {mobile}
      onclose={closeSidebar}
    />
  {:else}
    <main class="flex-1 min-h-0 p-3 pt-3">
      <div
        class="flex h-full w-full min-h-0 transition-[gap] ease-out"
        style:gap={desktopSidebarGap()}
        style:transition-duration={`${SIDEBAR_TRANSITION_MS}ms`}
      >
        <div class="min-w-0 min-h-0 flex-1">
          <TileContainer node={root} root />
        </div>

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
                crossContainerDragEnabled
              />
            {:else}
              <SidebarPanelSkeleton />
            {/if}
          </div>
        </div>
      </div>
    </main>
  {/if}

  <div class={mobile ? 'absolute inset-x-0 bottom-0 z-30' : 'contents'}>
    <AppFooter {mobile} />
  </div>
</div>
