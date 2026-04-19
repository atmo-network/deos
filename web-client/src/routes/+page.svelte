<script lang="ts">
  import { onMount } from "svelte";

  import WorkspaceFrame from "$lib/layout/WorkspaceFrame.svelte";
  import { layoutStore } from "$lib/layout/index.svelte";

  onMount(() => {
    void import("$lib/system/bootstrap-client").then(({ bootstrapClientWorkspace }) =>
      bootstrapClientWorkspace(),
    );
  });
</script>

<svelte:head>
  <title>DEOS Network</title>
</svelte:head>

<WorkspaceFrame root={layoutStore.root} />
