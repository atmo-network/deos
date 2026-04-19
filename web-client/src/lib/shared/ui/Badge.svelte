<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = {
		variant?: 'tmc' | 'xyk' | 'info';
		class?: string;
		children: Snippet;
	};

	let { variant = 'info', children, class: cls = '' }: Props = $props();

	const variants = {
		tmc: 'bg-(--mono-green)/15 text-(--mono-green)',
		xyk: 'bg-(--mono-purple)/15 text-(--mono-purple)',
		info: 'bg-(--mono-border) text-white'
	};
</script>

<span class={["text-xs px-2 py-0.5 rounded-full font-medium text-nowrap", variants[variant], cls]}>
	{@render children()}
</span>
