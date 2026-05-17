<!--
Domain: Pane drop overlay
Owns: Visual drop-target overlay for tab/pane drag interactions.
Excludes: Drop decision logic, tree mutation, and drag event lifecycle.
Zone: Layout rendering component; receives derived drag/drop state from PaneHost.
-->
<script lang="ts">
  import { Button } from '$lib/ui';

  import type { DropEdge } from './types';

  type Props = {
    canDropEdge: boolean;
    hoveredEdge: DropEdge | null;
    isDragging: boolean;
    isPaneDragging: boolean;
    paneMergeHovered: boolean;
    overlayTop: number;
    contentProjectionTop: number;
    payloadLabel: string | null;
    onOverlayDragOver: (event: DragEvent) => void;
    onOverlayDragLeave: () => void;
    onOverlayDrop: (event: DragEvent) => void;
    edgeProjectionShellClass: (edge: DropEdge) => string;
  };

  let {
    canDropEdge,
    hoveredEdge,
    isDragging,
    isPaneDragging,
    paneMergeHovered,
    overlayTop,
    contentProjectionTop,
    payloadLabel,
    onOverlayDragOver,
    onOverlayDragLeave,
    onOverlayDrop,
    edgeProjectionShellClass,
  }: Props = $props();
</script>

{#if isPaneDragging && paneMergeHovered}
  <div
    class="pointer-events-none absolute inset-x-2 bottom-2 z-20"
    style:top={`${contentProjectionTop}px`}
  >
    <div
      class="absolute inset-0 rounded-[1.35rem] bg-(--mono-purple)/12 blur-[14px] opacity-80"
    ></div>
    <div
      class="relative h-full rounded-[1.25rem] border border-(--mono-purple)/30 bg-(--mono-purple)/8 shadow-[0_12px_32px_rgba(117,77,165,0.16)]"
    ></div>
  </div>
{/if}

{#if isDragging && canDropEdge}
  <Button
    variant="ghost"
    size="sm"
    aria-label="Workspace drop target"
    class="absolute right-0 bottom-0 left-0 z-40 rounded-none bg-transparent p-0 text-left hover:bg-transparent"
    style={`top:${overlayTop}px`}
    ondragover={onOverlayDragOver}
    ondragleave={onOverlayDragLeave}
    ondrop={onOverlayDrop}
  >
    {#if hoveredEdge}
      <div class={edgeProjectionShellClass(hoveredEdge)}>
        <div
          class="absolute inset-0 rounded-[1.35rem] bg-(--mono-purple)/12 blur-[14px] opacity-80"
        ></div>
        <div
          class="relative flex h-full items-center justify-center rounded-[1.25rem] border border-(--mono-purple)/32 bg-(--mono-purple)/8 shadow-[0_12px_32px_rgba(117,77,165,0.16)]"
        >
          {#if payloadLabel}
            <div class="pointer-events-none relative shrink-0 select-none">
              <div
                class="absolute inset-0 rounded-lg bg-(--mono-purple)/18 blur-[10px] scale-[1.08] opacity-80"
              ></div>
              <div
                class="relative rounded-lg border border-(--mono-purple)/35 bg-(--mono-purple)/10 px-2.5 py-1 text-[11px] font-medium whitespace-nowrap text-(--mono-purple) shadow-[0_10px_24px_rgba(117,77,165,0.2)] sm:px-3 sm:text-xs"
              >
                {payloadLabel}
              </div>
            </div>
          {:else if isPaneDragging}
            <div
              class="relative flex items-center justify-center gap-1.5 opacity-80"
            >
              <span class="h-1 w-14 rounded-full bg-(--mono-purple)/28"></span>
              <span class="h-1 w-3 rounded-full bg-(--mono-purple)/20"></span>
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </Button>
{/if}
