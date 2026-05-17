<!--
Domain: Split resize handle
Owns: Pointer-driven split resizing interaction for workspace tile splits.
Excludes: Tile tree topology changes, widget rendering, and persisted layout policy.
Zone: Layout interaction component; delegates ratio updates to layout store.
-->
<script lang="ts">
  import { layoutStore } from '$lib/layout/index.svelte';

  type Props = {
    splitId: string;
    direction: 'horizontal' | 'vertical';
  };

  let { splitId, direction }: Props = $props();
  let handleEl: HTMLDivElement;
  let dragging = $state(false);

  function onPointerDown(event: PointerEvent) {
    dragging = true;
    handleEl.setPointerCapture(event.pointerId);
    event.preventDefault();
  }

  function onPointerMove(event: PointerEvent) {
    if (!dragging) {
      return;
    }
    const parent = handleEl.parentElement;
    if (!parent) {
      return;
    }
    const rect = parent.getBoundingClientRect();
    const ratio =
      direction === 'horizontal'
        ? (event.clientX - rect.left) / rect.width
        : (event.clientY - rect.top) / rect.height;
    layoutStore.resizeSplit(splitId, ratio);
  }

  function onPointerUp() {
    dragging = false;
  }

  function onDoubleClick() {
    layoutStore.resizeSplit(splitId, 0.5);
  }
</script>

<div
  bind:this={handleEl}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  ondblclick={onDoubleClick}
  role="separator"
  class={[
    'shrink-0 z-10 bg-clip-content duration-150 bg-transparent hover:bg-(--mono-cyan) rounded-md m-auto ease-in-out transition-all active:bg-(--mono-border)',
    direction === 'horizontal'
      ? 'px-1 w-3 cursor-col-resize h-full active:h-1/3'
      : 'py-1 h-3 cursor-row-resize w-full active:w-1/3',
  ]}
></div>
