<!--
Domain: Mobile tile container
Owns: Responsive/mobile rendering of center tile tree leaves and splits.
Excludes: Reserved edge lanes, widget internals, and layout mutation policy.
Zone: Layout rendering component; recursively renders tile structure from layout contracts.
-->
<script lang="ts">
  import { onMount } from 'svelte';

  import {
    MOBILE_LAYOUT_BREAKPOINT,
    type TileLeaf,
    type TileNode,
  } from '$lib/layout/types';

  import PaneHost from './PaneHost.svelte';
  import SplitHandle from './SplitHandle.svelte';

  type Props = {
    node: TileNode;
    root?: boolean;
  };

  // Tailwind-style mobile breakpoint: only collapse the root tree into a
  // linear ribbon below `sm`; desktop/tablet keeps the split-stack workspace.
  const RIBBON_LEAF_HEIGHT = 'clamp(15rem, 34vh, 24rem)';

  let { node, root = false }: Props = $props();
  let containerEl = $state<HTMLDivElement | null>(null);
  let ribbonMode = $state(false);

  const ribbonLeaves = $derived.by(() =>
    root && ribbonMode ? collectLeaves(node) : [],
  );

  function splitTemplate(node: Exclude<TileNode, { type: 'leaf' }>): string {
    const primary = `${Math.max(0.15, Math.min(0.85, node.ratio))}fr`;
    const secondary = `${Math.max(0.15, Math.min(0.85, 1 - node.ratio))}fr`;
    return node.direction === 'horizontal'
      ? `minmax(0, ${primary}) auto minmax(0, ${secondary})`
      : `minmax(0, ${primary}) auto minmax(0, ${secondary})`;
  }

  function collectLeaves(node: TileNode): TileLeaf[] {
    if (node.type === 'leaf') {
      return [node];
    }
    return [
      ...collectLeaves(node.children[0]),
      ...collectLeaves(node.children[1]),
    ];
  }

  function syncRibbonMode() {
    if (!root || !containerEl) {
      ribbonMode = false;
      return;
    }
    ribbonMode = containerEl.clientWidth < MOBILE_LAYOUT_BREAKPOINT;
  }

  onMount(() => {
    syncRibbonMode();
    if (!root || !containerEl) {
      return;
    }
    const resizeObserver = new ResizeObserver(() => syncRibbonMode());
    resizeObserver.observe(containerEl);
    return () => resizeObserver.disconnect();
  });
</script>

<div bind:this={containerEl} class="h-full w-full min-h-0">
  {#if root && ribbonMode}
    <div class="grid h-full min-h-0 content-start gap-3 overflow-y-auto pr-1">
      {#each ribbonLeaves as leaf (leaf.id)}
        <div class="min-h-70" style:height={RIBBON_LEAF_HEIGHT}>
          <PaneHost {leaf} />
        </div>
      {/each}
    </div>
  {:else}
    {@render tileTree(node)}
  {/if}
</div>

{#snippet tileTree(tileNode: TileNode)}
  {#if tileNode.type === 'leaf'}
    <PaneHost leaf={tileNode} />
  {:else}
    <div
      class="grid h-full w-full"
      style:grid-template-columns={tileNode.direction === 'horizontal'
        ? splitTemplate(tileNode)
        : undefined}
      style:grid-template-rows={tileNode.direction === 'vertical'
        ? splitTemplate(tileNode)
        : undefined}
    >
      <div style:min-width="0" style:min-height="0">
        {@render tileTree(tileNode.children[0])}
      </div>
      <SplitHandle splitId={tileNode.id} direction={tileNode.direction} />
      <div style:min-width="0" style:min-height="0">
        {@render tileTree(tileNode.children[1])}
      </div>
    </div>
  {/if}
{/snippet}
