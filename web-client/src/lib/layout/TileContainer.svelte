<!--
Domain: Mobile tile container
Owns: Responsive/mobile rendering of center tile tree leaves and splits.
Excludes: Reserved edge lanes, widget internals, and layout mutation policy.
Zone: Layout rendering component; recursively renders tile structure from layout contracts.
-->
<script lang="ts">
  import { onMount } from 'svelte';

  import {
    MIN_PANE_STACK_HEIGHT_PX,
    MOBILE_LAYOUT_BREAKPOINT,
    SPLIT_HANDLE_EXTENT_PX,
    type TileNode,
  } from '$lib/layout/types';

  import MobileWorkspaceStack from './MobileWorkspaceStack.svelte';
  import PaneHost from './PaneHost.svelte';
  import SplitHandle from './SplitHandle.svelte';
  import {
    minimumTileHeight as calculateMinimumTileHeight,
    collectDirectionalSegments,
  } from './split-groups';

  type Props = {
    node: TileNode;
    root?: boolean;
  };
  let { node, root = false }: Props = $props();
  let containerEl = $state<HTMLDivElement | null>(null);
  let mobileStackMode = $state(false);

  function minimumTileHeight(node: TileNode): number {
    return calculateMinimumTileHeight(
      node,
      MIN_PANE_STACK_HEIGHT_PX,
      SPLIT_HANDLE_EXTENT_PX,
    );
  }

  function syncMobileStackMode() {
    if (!root || !containerEl) {
      mobileStackMode = false;
      return;
    }
    mobileStackMode = containerEl.clientWidth < MOBILE_LAYOUT_BREAKPOINT;
  }

  onMount(() => {
    syncMobileStackMode();
    if (!root || !containerEl) {
      return;
    }
    const resizeObserver = new ResizeObserver(() => syncMobileStackMode());
    resizeObserver.observe(containerEl);
    return () => resizeObserver.disconnect();
  });
</script>

<div
  bind:this={containerEl}
  class={[
    'h-full w-full min-h-0',
    root && mobileStackMode ? 'overflow-hidden' : 'overflow-auto',
  ]}
>
  {#if root && mobileStackMode}
    <MobileWorkspaceStack {node} />
  {:else}
    <div
      class="h-full min-h-full w-full"
      style:min-height={`${minimumTileHeight(node)}px`}
    >
      {@render tileTree(node)}
    </div>
  {/if}
</div>

{#snippet tileTree(tileNode: TileNode)}
  {#if tileNode.type === 'leaf'}
    <PaneHost leaf={tileNode} />
  {:else}
    {@const segments = collectDirectionalSegments(tileNode, tileNode.direction)}
    <div
      class={[
        'flex h-full w-full',
        tileNode.direction === 'horizontal' ? 'flex-row' : 'flex-col',
      ]}
    >
      {#each segments as segment, index (segment.node.id)}
        <div
          class="min-h-0 min-w-0"
          data-split-segment={segment.node.id}
          data-split-weight={segment.weight}
          style:flex-basis="0px"
          style:flex-grow={segment.weight}
          style:flex-shrink="1"
          style:min-height={tileNode.direction === 'vertical'
            ? `${minimumTileHeight(segment.node)}px`
            : undefined}
        >
          {@render tileTree(segment.node)}
        </div>
        {#if index < segments.length - 1}
          <SplitHandle
            splitId={tileNode.id}
            direction={tileNode.direction}
            minPrimaryPx={minimumTileHeight(segment.node)}
            minSecondaryPx={minimumTileHeight(segments[index + 1].node)}
          />
        {/if}
      {/each}
    </div>
  {/if}
{/snippet}
