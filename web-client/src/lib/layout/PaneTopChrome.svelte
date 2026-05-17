<!--
Domain: Pane tab chrome
Owns: Tab strip rendering, tab drag affordances, and active/preview tab controls for a center pane.
Excludes: Tile tree mutation policy, widget rendering, and reserved edge-lane controls.
Zone: Layout rendering component; receives state/callbacks from PaneHost.
-->
<script lang="ts">
  import { Button } from '$lib/ui';

  import type { PreviewTabItem } from './pane-dnd';
  import type { TabFlipAnimate } from './tab-flip';
  import type { PanelId, TileLeaf } from './types';

  type Props = {
    leaf: TileLeaf;
    previewTabs: PreviewTabItem[];
    panelLabels: Record<PanelId, string>;
    tabBarEl: HTMLDivElement | null;
    paneGripEl: HTMLButtonElement | null;
    animateTabs: TabFlipAnimate;
    isLiftedSourceTab: (tabId: PanelId) => boolean;
    canMergePaneHere: boolean;
    isLiftedSourcePane: boolean;
    isPaneDragging: boolean;
    paneMergeHovered: boolean;
    onTabDragStart: (event: DragEvent, tabId: PanelId) => void;
    onAnyDragEnd: () => void;
    onSelectTab: (tabId: PanelId) => void;
    onTabBarDragOver: (event: DragEvent) => void;
    onTabBarDragLeave: (event: DragEvent) => void;
    onTabBarDrop: (event: DragEvent) => void;
    onPaneDragStart: (event: DragEvent) => void;
    onPanePlateDragLeave: (event: DragEvent) => void;
    onPanePlateDragOver: (event: DragEvent) => void;
    onPanePlateDrop: (event: DragEvent) => void;
  };

  let {
    leaf,
    previewTabs,
    panelLabels,
    tabBarEl = $bindable(),
    paneGripEl = $bindable(),
    animateTabs,
    isLiftedSourceTab,
    canMergePaneHere,
    isLiftedSourcePane,
    isPaneDragging,
    paneMergeHovered,
    onTabDragStart,
    onAnyDragEnd,
    onSelectTab,
    onTabBarDragOver,
    onTabBarDragLeave,
    onTabBarDrop,
    onPaneDragStart,
    onPanePlateDragLeave,
    onPanePlateDragOver,
    onPanePlateDrop,
  }: Props = $props();
</script>

<div class="relative min-w-0">
  <div class="absolute inset-0">
    <!-- Raw button is intentional: drag geometry needs direct HTMLButtonElement bind ownership. -->
    <button
      bind:this={paneGripEl}
      type="button"
      draggable="true"
      ondragstart={onPaneDragStart}
      ondragend={onAnyDragEnd}
      ondragover={onPanePlateDragOver}
      ondragleave={onPanePlateDragLeave}
      ondrop={onPanePlateDrop}
      title="Move pane stack"
      aria-label="Move pane stack"
      class={[
        'group relative flex h-full w-full appearance-none select-none items-center justify-center rounded-xl border border-transparent bg-transparent p-0 transition-[background-color,border-color,box-shadow,opacity,transform] duration-150',
        canMergePaneHere || !isPaneDragging
          ? 'cursor-grab active:cursor-grabbing'
          : 'cursor-default',
        paneMergeHovered
          ? 'border-(--mono-purple)/35 bg-(--mono-purple)/8 shadow-[0_10px_24px_rgba(117,77,165,0.14)]'
          : 'hover:bg-(--mono-purple)/5',
        isLiftedSourcePane ? 'scale-[0.99] opacity-40' : 'opacity-100',
      ]}
    >
      {#if paneMergeHovered && isPaneDragging}
        <div
          class="pointer-events-none absolute inset-0 rounded-xl bg-(--mono-purple)/14 blur-[10px] opacity-75"
        ></div>
      {/if}
    </button>
  </div>

  <div
    class="relative z-10 flex w-full min-w-0 justify-start px-1 py-1 pointer-events-none sm:justify-center"
  >
    <div
      bind:this={tabBarEl}
      ondragover={onTabBarDragOver}
      ondragleave={onTabBarDragLeave}
      ondrop={onTabBarDrop}
      class="pointer-events-auto flex w-fit max-w-full min-w-0 items-center gap-0.5 overflow-x-auto scrollbar-none"
      role="tablist"
      tabindex="-1"
    >
      {#each previewTabs as item (item.key)}
        <div
          animate:animateTabs
          class="relative shrink-0 will-change-transform"
        >
          {#if item.kind === 'projection'}
            <div
              class="pointer-events-none relative shrink-0 select-none rounded-lg border border-(--mono-purple)/35 bg-(--mono-purple)/10 px-2.5 py-1 text-[11px] font-medium whitespace-nowrap text-(--mono-purple) sm:px-3 sm:text-xs z-1"
            >
              {panelLabels[item.tabId]}
            </div>
          {:else}
            <Button
              variant="ghost"
              size="sm"
              data-tab-id={item.tabId}
              draggable="true"
              ondragstart={(event) => onTabDragStart(event, item.tabId)}
              ondragend={onAnyDragEnd}
              onclick={() => onSelectTab(item.tabId)}
              role="tab"
              aria-selected={leaf.activeTab === item.tabId}
              class={[
                'rounded-lg border border-transparent px-2.5 py-1 text-[11px] font-medium select-none cursor-pointer active:cursor-grabbing whitespace-nowrap transition-[transform,opacity,color,background-color,border-color] duration-150 sm:px-3 sm:text-xs active:border-(--mono-border)',
                leaf.activeTab === item.tabId
                  ? 'bg-(--mono-border) text-white'
                  : 'text-(--mono-border) hover:bg-white hover:text-(--mono-text)',
                isLiftedSourceTab(item.tabId)
                  ? 'scale-[0.98] opacity-40'
                  : 'opacity-100',
              ]}
            >
              {panelLabels[item.tabId]}
            </Button>
          {/if}
        </div>
      {/each}
    </div>
  </div>
</div>
