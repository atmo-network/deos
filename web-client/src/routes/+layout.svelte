<!--
Domain: Application layout route
Owns: Global stylesheet import, root document title, and child route rendering shell.
Excludes: Workspace layout implementation, domain stores, and widget composition.
Zone: SvelteKit layout entrypoint with no product-domain ownership.
-->
<script lang="ts">
  import '../app.css';

  let { children } = $props();
</script>

<svelte:head>
  <title>DEOS</title>
</svelte:head>

{@render children()}
