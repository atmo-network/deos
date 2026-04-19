<script lang="ts">
	import '../app.css';
	let { children } = $props();
</script>

<svelte:head>
	<title>DEOS</title>
</svelte:head>

{@render children()}
