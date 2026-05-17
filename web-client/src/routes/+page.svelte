<!--
Domain: Application route
Owns: Root workspace page composition and client bootstrap trigger.
Excludes: Layout implementation, store internals, and adapter startup details.
Zone: SvelteKit route entrypoint; composes layout shell through public contracts only.
-->
<script lang="ts">
  import { onMount } from 'svelte';

  import WorkspaceFrame from '$lib/layout/WorkspaceFrame.svelte';
  import { layoutStore } from '$lib/layout/index.svelte';

  onMount(() => {
    void import('$lib/system/bootstrap-client').then(
      ({ bootstrapClientWorkspace }) => bootstrapClientWorkspace(),
    );
  });
</script>

<svelte:head>
  <title>DEOS Network</title>
</svelte:head>

<WorkspaceFrame root={layoutStore.root} />
