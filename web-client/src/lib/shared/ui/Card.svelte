<script lang="ts">
	import type { Snippet } from 'svelte';

	type OverflowMode = 'none' | 'x' | 'y' | 'both';
	type Props = {
		level?: 0 | 1 | 2 | 3;
		overflow?: OverflowMode;
		class?: string;
		children: Snippet;
	};

	let { level = 1, overflow = 'none', children, class: cls = '' }: Props = $props();
	let rootEl = $state<HTMLDivElement | null>(null);
	let hasVerticalOverflow = $state(false);
	let isScrolled = $state(false);

	const levels = {
		0: 'bg-(--mono-bg)',
		1: 'bg-white',
		2: 'bg-(--mono-bg)',
		3: 'bg-[#eaece3]'
	};
	const overflows: Record<OverflowMode, string> = {
		none: 'overflow-visible',
		x: 'overflow-x-auto overflow-y-hidden',
		y: 'overflow-y-auto overflow-x-hidden',
		both: 'overflow-auto'
	};

	function tracksVerticalOverflow(mode: OverflowMode) {
		return mode === 'y' || mode === 'both';
	}

	function updateScrollState() {
		if (!rootEl || !tracksVerticalOverflow(overflow)) {
			hasVerticalOverflow = false;
			isScrolled = false;
			return;
		}
		hasVerticalOverflow = rootEl.scrollHeight > rootEl.clientHeight + 1;
		isScrolled = rootEl.scrollTop > 0;
	}

	$effect(() => {
		if (!rootEl || !tracksVerticalOverflow(overflow)) {
			updateScrollState();
			return;
		}
		updateScrollState();
		const resizeObserver = new ResizeObserver(() => updateScrollState());
		const mutationObserver = new MutationObserver(() => updateScrollState());
		resizeObserver.observe(rootEl);
		mutationObserver.observe(rootEl, {
			childList: true,
			subtree: true,
			characterData: true
		});
		return () => {
			resizeObserver.disconnect();
			mutationObserver.disconnect();
		};
	});
</script>

<div
	bind:this={rootEl}
	class={[levels[level], overflows[overflow], 'relative rounded-2xl', cls]}
	onscroll={updateScrollState}
>
	{#if hasVerticalOverflow}
		<div class="pointer-events-none sticky top-0 z-20 h-0">
			<div
				class="h-4 w-full rounded-t-2xl bg-linear-to-b from-[rgba(44,50,30,0.14)] via-[rgba(44,50,30,0.06)] to-transparent transition-opacity duration-150"
				class:opacity-100={isScrolled}
				class:opacity-65={!isScrolled}
			></div>
		</div>
	{/if}
	{@render children()}
</div>
