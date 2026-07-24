<!--
Domain: Wiki widget
Owns: In-app wiki navigation, trusted markdown rendering presentation, search/filter UI, and wiki metadata browsing.
Excludes: Wiki content generation, trust validation scripts, layout ownership, and generic markdown sanitization.
Zone: Presentation widget; consumes repo-local wiki metadata and trusted wiki helpers.
-->
<script lang="ts">
  import { Search } from '@lucide/svelte';
  import { onDestroy, onMount } from 'svelte';

  import {
    currentWidgetDeepLink,
    navigateToWiki,
    subscribeToWidgetDeepLinks,
  } from '$lib/navigation/hash-navigation';
  import { BackButton, Button, Card, Icon, TextField, Tooltip } from '$lib/ui';
  import type {
    LocalizedValue,
    RelatedWikiItem,
    ResolvedWikiNavigationSection,
    WikiAliasManifest,
    WikiGraphManifest,
    WikiNavigationManifest,
    WikiSearchManifest,
  } from '$lib/wiki/metadata-contract';
  import { iconForWikiPage } from '$lib/wiki/page-icons';
  import {
    matchWikiSearchBodies,
    normalizeWikiSearchText,
  } from '$lib/wiki/search';
  import { type TrustedWikiPage, loadTrustedWikiPage } from '$lib/wiki/trusted';

  import aliases from '../../../../wiki/_meta/aliases.json';
  import graph from '../../../../wiki/_meta/graph.json';
  import navigation from '../../../../wiki/_meta/navigation.json';

  const wikiAliases: WikiAliasManifest = aliases;
  const wikiGraph: WikiGraphManifest = graph;
  const wikiNavigation: WikiNavigationManifest = navigation;
  const availableLocales = wikiNavigation.available_locales;

  let currentLocale = $state(wikiNavigation.default_locale);
  let searchQuery = $state('');
  let searchManifest = $state<WikiSearchManifest | null>(null);
  let searchManifestLoading = $state(false);
  let searchManifestFailed = $state(false);
  let selectedPageId = $state<string | null>(null);
  let selectedPage = $state<TrustedWikiPage | null>(null);
  let selectedError = $state<string | null>(null);
  let loadingPage = $state(false);
  let showLoadingPage = $state(false);
  let loadingIndicatorTimer: ReturnType<typeof setTimeout> | null = null;
  let pageRequestSerial = 0;

  function resolveLocale(candidate: string | null | undefined) {
    if (!candidate) {
      return wikiNavigation.default_locale;
    }
    if (availableLocales.includes(candidate)) {
      return candidate;
    }
    const baseLocale = candidate.split('-')[0];
    if (availableLocales.includes(baseLocale)) {
      return baseLocale;
    }
    return wikiNavigation.default_locale;
  }

  function pickLocalized(value: LocalizedValue) {
    return (
      value[currentLocale] ??
      value[wikiNavigation.default_locale] ??
      Object.values(value)[0] ??
      ''
    );
  }

  function normalizeSearchText(value: string) {
    return normalizeWikiSearchText(value, currentLocale);
  }

  async function ensureSearchManifestLoaded(): Promise<void> {
    if (searchManifest || searchManifestLoading || searchManifestFailed) {
      return;
    }
    searchManifestLoading = true;
    try {
      const module = await import('../../../../wiki/_meta/search.json');
      searchManifest = module.default as WikiSearchManifest;
    } catch {
      searchManifestFailed = true;
    } finally {
      searchManifestLoading = false;
    }
  }

  function formatRelationLabel(value: string) {
    return value.replace(/-/g, ' ');
  }

  function getDefaultPageId() {
    return (
      wikiNavigation.entrypoints[0] ??
      wikiNavigation.sections[0]?.items[0]?.id ??
      null
    );
  }

  function openPage(itemId: string) {
    selectedPageId = itemId;
    navigateToWiki(itemId);
  }

  function closePage() {
    selectedPageId = null;
    navigateToWiki(null);
  }

  function anchorFromEventTarget(target: EventTarget | null) {
    if (!(target instanceof HTMLElement)) {
      return null;
    }
    const anchor = target.closest('a');
    return anchor instanceof HTMLAnchorElement ? anchor : null;
  }

  function handleRenderedContentClick(event: MouseEvent) {
    const anchor = anchorFromEventTarget(event.target);
    if (!anchor) {
      return;
    }
    const wikiPath = anchor.dataset.wikiPath;
    if (wikiPath) {
      const matchingItem = allItems.find((item) => item.path === wikiPath);
      if (!matchingItem) {
        return;
      }
      event.preventDefault();
      openPage(matchingItem.id);
      return;
    }
  }

  function bindRenderedContentInteractions(node: HTMLDivElement) {
    node.addEventListener('click', handleRenderedContentClick);
    return {
      destroy() {
        node.removeEventListener('click', handleRenderedContentClick);
      },
    };
  }

  const sections: ResolvedWikiNavigationSection[] = $derived(
    wikiNavigation.sections.map((section) => ({
      id: section.id,
      title: pickLocalized(section.title),
      items: section.items.map((item) => ({
        id: item.id,
        title: pickLocalized(item.title),
        path: pickLocalized(item.path),
        page_type: item.page_type,
        summary: pickLocalized(item.summary),
      })),
    })),
  );

  const allItems = $derived(sections.flatMap((section) => section.items));
  const aliasTermsById: Map<string, string[]> = $derived.by(() => {
    const termsById = new Map<string, string[]>();
    const locales = [currentLocale, wikiAliases.default_locale].filter(
      (locale, index, array) => array.indexOf(locale) === index,
    );
    for (const locale of locales) {
      const aliasMap = wikiAliases.aliases[locale] ?? {};
      for (const [term, id] of Object.entries(aliasMap)) {
        const terms = termsById.get(id) ?? [];
        if (!terms.includes(term)) {
          terms.push(term);
        }
        termsById.set(id, terms);
      }
    }
    return termsById;
  });
  const normalizedSearchQuery = $derived(normalizeSearchText(searchQuery));
  const bodyMatchesById: Map<string, string> = $derived.by(() =>
    searchManifest
      ? matchWikiSearchBodies(searchManifest, searchQuery, currentLocale)
      : new Map<string, string>(),
  );
  const aliasMatchesById: Map<string, string[]> = $derived.by(() => {
    if (!normalizedSearchQuery) {
      return new Map<string, string[]>();
    }
    const matchesById = new Map<string, string[]>();
    for (const [id, aliases] of aliasTermsById.entries()) {
      const matchingAliases = aliases.filter((alias) =>
        normalizeSearchText(alias).includes(normalizedSearchQuery),
      );
      if (matchingAliases.length > 0) {
        matchesById.set(id, matchingAliases);
      }
    }
    return matchesById;
  });
  const filteredSections = $derived(
    !normalizedSearchQuery
      ? sections
      : sections
          .map((section) => ({
            ...section,
            items: section.items.filter(
              (item) =>
                [
                  item.title,
                  item.summary,
                  section.title,
                  ...(aliasTermsById.get(item.id) ?? []),
                ].some((value) =>
                  normalizeSearchText(value).includes(normalizedSearchQuery),
                ) || bodyMatchesById.has(item.id),
            ),
          }))
          .filter((section) => section.items.length > 0),
  );
  const allItemsById = $derived(
    new Map(allItems.map((item) => [item.id, item])),
  );
  const selectedItem = $derived(
    allItems.find((item) => item.id === selectedPageId) ?? null,
  );
  const graphRelatedItems: RelatedWikiItem[] = $derived.by(() => {
    if (!selectedPageId) {
      return [];
    }
    const itemsById = allItemsById;
    const relations = new Map<string, Set<string>>();
    for (const edge of wikiGraph.edges) {
      if (edge.from === selectedPageId && edge.to !== selectedPageId) {
        const labels = relations.get(edge.to) ?? new Set<string>();
        labels.add(formatRelationLabel(edge.type));
        relations.set(edge.to, labels);
      }
      if (edge.to === selectedPageId && edge.from !== selectedPageId) {
        const labels = relations.get(edge.from) ?? new Set<string>();
        labels.add(formatRelationLabel(edge.type));
        relations.set(edge.from, labels);
      }
    }
    return [...relations.entries()]
      .map(([id, labels]) => {
        const item = itemsById.get(id);
        if (!item) {
          return null;
        }
        return {
          id: item.id,
          title: item.title,
          path: item.path,
          summary: item.summary,
          relation: [...labels].join(' · '),
        };
      })
      .filter((item): item is RelatedWikiItem => item !== null)
      .sort((left, right) =>
        left.title.localeCompare(right.title, currentLocale),
      );
  });
  const authoredRelatedItems: RelatedWikiItem[] = $derived.by(() =>
    (selectedPage?.relatedWikiPaths ?? [])
      .map((path) => {
        const item = allItems.find((candidate) => candidate.path === path);
        return item
          ? {
              id: item.id,
              title: item.title,
              path: item.path,
              summary: item.summary,
              relation: currentLocale === 'ru' ? 'Из статьи' : 'From article',
            }
          : null;
      })
      .filter((item): item is RelatedWikiItem => item !== null),
  );
  const relatedItems: RelatedWikiItem[] = $derived.by(() => {
    const merged = new Map<string, RelatedWikiItem>();
    for (const item of authoredRelatedItems) {
      merged.set(item.id, item);
    }
    for (const item of graphRelatedItems) {
      if (!merged.has(item.id)) {
        merged.set(item.id, item);
      }
    }
    return [...merged.values()];
  });
  const widgetText = $derived(
    currentLocale === 'ru'
      ? {
          title: 'Wiki',
          search: 'Поиск',
          searchPlaceholder: 'Название или тема',
          noMatchesTitle: 'Совпадений не найдено',
          searchingBody: 'Поиск по тексту страниц…',
          noMatchesBody:
            'Измените запрос или очистите фильтр, чтобы снова увидеть разделы wiki.',
          relatedPages: 'Связанные страницы',
          aliasMatch: 'Alias',
          clearSearch: 'Очистить',
          back: 'Назад',
          loading: 'Загрузка wiki-страницы...',
          emptyTitle: 'Выберите страницу',
          emptyBody:
            'Откройте любую wiki-страницу из навигации, чтобы увидеть её содержимое прямо в клиенте.',
          loadError: 'Не удалось загрузить wiki-страницу',
        }
      : {
          title: 'Wiki',
          search: 'Search',
          searchPlaceholder: 'Title or topic',
          noMatchesTitle: 'No matches',
          searchingBody: 'Searching page text…',
          noMatchesBody:
            'Adjust the query or clear the filter to restore the full wiki navigation graph.',
          relatedPages: 'Related',
          aliasMatch: 'Alias',
          clearSearch: 'Clear',
          back: 'Back',
          loading: 'Loading wiki page...',
          emptyTitle: 'Choose a page',
          emptyBody:
            'Open any wiki page from the navigation list to render its content directly in the client.',
          loadError: 'Failed to load wiki page',
        },
  );

  function clearLoadingIndicatorTimer(): void {
    if (loadingIndicatorTimer === null) {
      return;
    }
    clearTimeout(loadingIndicatorTimer);
    loadingIndicatorTimer = null;
  }

  function finishPageLoad(): void {
    clearLoadingIndicatorTimer();
    loadingPage = false;
    showLoadingPage = false;
  }

  $effect(() => {
    if (normalizedSearchQuery) {
      void ensureSearchManifestLoaded();
    }
  });

  $effect(() => {
    const pagePath = selectedItem?.path ?? null;
    const requestId = ++pageRequestSerial;
    clearLoadingIndicatorTimer();
    showLoadingPage = false;
    if (!pagePath) {
      loadingPage = false;
      selectedError = null;
      selectedPage = null;
      return;
    }
    loadingPage = true;
    selectedError = null;
    selectedPage = null;
    loadingIndicatorTimer = setTimeout(() => {
      if (requestId === pageRequestSerial && loadingPage) {
        showLoadingPage = true;
      }
      loadingIndicatorTimer = null;
    }, 120);
    void loadTrustedWikiPage(pagePath)
      .then((page) => {
        if (requestId !== pageRequestSerial) {
          return;
        }
        selectedPage = page;
        finishPageLoad();
      })
      .catch((error: unknown) => {
        if (requestId !== pageRequestSerial) {
          return;
        }
        selectedError = error instanceof Error ? error.message : String(error);
        finishPageLoad();
      });
  });

  onDestroy(clearLoadingIndicatorTimer);

  onMount(() => {
    currentLocale = resolveLocale(navigator.language);
    const initialLink = currentWidgetDeepLink();
    selectedPageId =
      initialLink?.widget === 'wiki' ? initialLink.pageId : getDefaultPageId();
    return subscribeToWidgetDeepLinks((link) => {
      if (link?.widget === 'wiki') {
        selectedPageId = link.pageId;
      }
    });
  });
</script>

<Card class="min-h-full flex flex-col">
  <div class="wiki-container h-full min-h-0 [container-type:inline-size]">
    <div class="wiki-layout grid h-full min-h-0 gap-3 px-3 py-5">
      <nav
        class={[
          'wiki-navigation min-h-0 grid content-start gap-5 pr-1',
          selectedItem && 'is-reader-open',
        ]}
        aria-label={widgetText.title}
      >
        <section class="grid gap-2">
          <div class="relative">
            <Icon
              icon={Search}
              size="sm"
              class="pointer-events-none absolute top-1/2 left-2.5 z-10 -translate-y-1/2 text-(--mono-muted)"
            />
            <TextField
              aria-label={widgetText.search}
              bind:value={searchQuery}
              placeholder={widgetText.searchPlaceholder}
              inputClass="py-1.5 pr-2 pl-8 text-sm"
            />
          </div>
          {#if searchQuery}
            <Button
              size="sm"
              variant="secondary"
              class="justify-self-start rounded-lg px-2 py-1 text-xs"
              onclick={() => (searchQuery = '')}
            >
              {widgetText.clearSearch}
            </Button>
          {/if}
        </section>
        {#if filteredSections.length === 0}
          <div
            class="rounded-xl border border-dashed border-(--mono-border) p-3 text-sm text-(--mono-muted)"
            aria-live="polite"
          >
            {#if searchManifestLoading}
              <div>{widgetText.searchingBody}</div>
            {:else}
              <div class="font-medium text-(--mono-text)">
                {widgetText.noMatchesTitle}
              </div>
              <div class="mt-1">{widgetText.noMatchesBody}</div>
            {/if}
          </div>
        {:else}
          {#each filteredSections as section}
            <section
              class="grid gap-1 [contain-intrinsic-size:auto_12rem] [content-visibility:auto]"
            >
              <div
                class="mb-1 px-2 text-compact font-semibold uppercase tracking-widest text-(--mono-muted)"
              >
                {section.title}
              </div>
              <div class="grid gap-0.5 text-sm">
                {#each section.items as item}
                  <Button
                    size="sm"
                    variant="ghost"
                    class={[
                      'grid min-w-0 justify-stretch rounded-md px-2 py-1.5 text-left hover:bg-(--mono-bg)',
                      item.id === selectedPageId
                        ? 'bg-(--mono-bg) font-medium text-(--mono-purple)'
                        : 'text-(--mono-text) hover:text-(--mono-purple)',
                    ]}
                    aria-current={item.id === selectedPageId
                      ? 'page'
                      : undefined}
                    onclick={() => openPage(item.id)}
                  >
                    <span
                      class="grid min-w-0 grid-cols-[auto_minmax(0,1fr)] items-start gap-2"
                    >
                      <Icon
                        icon={iconForWikiPage(item.id)}
                        size="sm"
                        class={[
                          'mt-0.5',
                          item.id === selectedPageId
                            ? 'text-(--mono-purple)'
                            : 'text-(--mono-muted)',
                        ]}
                      />
                      <span class="min-w-0 grid gap-0.5">
                        <span class="truncate">{item.title}</span>
                        {#if searchQuery}
                          <span class="line-clamp-2 text-xs text-(--mono-muted)"
                            >{bodyMatchesById.get(item.id) ??
                              item.summary}</span
                          >
                          {#if (aliasMatchesById.get(item.id) ?? []).length > 0}
                            <span
                              class="flex flex-wrap items-center gap-1 text-xs text-(--mono-muted)"
                            >
                              <span class="uppercase tracking-[0.08em]"
                                >{widgetText.aliasMatch}</span
                              >
                              {#each aliasMatchesById.get(item.id) ?? [] as aliasTerm}
                                <span
                                  class="rounded-full border border-(--mono-border) bg-white px-1.5 py-0.5 text-(--mono-text)"
                                  >{aliasTerm}</span
                                >
                              {/each}
                            </span>
                          {/if}
                        {/if}
                      </span>
                    </span>
                  </Button>
                {/each}
              </div>
            </section>
          {/each}
        {/if}
      </nav>

      <section
        class={[
          'wiki-reader flex min-h-0 flex-col rounded-xl bg-white',
          !selectedItem && 'is-empty',
        ]}
      >
        <div class="wiki-reader-scroll min-h-0">
          <div class="wiki-reader-content grid content-start gap-5 py-1 px-4">
            <main
              class="grid w-full max-w-[56rem] min-w-0 justify-self-center content-start gap-3"
            >
              {#if selectedItem}
                <div class="flex items-center @min-[60rem]:hidden">
                  <BackButton
                    onclick={closePage}
                    label={widgetText.back}
                    text={widgetText.back}
                  />
                </div>
              {/if}
              {#if loadingPage}
                {#if showLoadingPage}
                  <div
                    class="grid animate-pulse gap-3 py-1"
                    role="status"
                    aria-label={widgetText.loading}
                  >
                    <div class="h-5 w-2/5 rounded-md bg-(--mono-bg)"></div>
                    <div class="grid gap-2">
                      <div class="h-3 w-full rounded bg-(--mono-bg)"></div>
                      <div class="h-3 w-11/12 rounded bg-(--mono-bg)"></div>
                      <div class="h-3 w-4/5 rounded bg-(--mono-bg)"></div>
                    </div>
                  </div>
                {/if}
              {:else if selectedError}
                <div
                  class="rounded-xl border border-red-300 bg-red-50 p-3 text-xs text-red-700"
                >
                  <div class="font-medium">{widgetText.loadError}</div>
                </div>
              {:else if selectedPage}
                <div
                  class="wiki-markdown text-xs text-(--mono-text)"
                  use:bindRenderedContentInteractions
                >
                  {@html selectedPage.html}
                </div>
              {:else}
                <div
                  class="rounded-xl border border-dashed border-(--mono-border) p-3 text-xs text-(--mono-muted)"
                >
                  <div class="font-medium text-(--mono-text)">
                    {widgetText.emptyTitle}
                  </div>
                  <div class="mt-1">{widgetText.emptyBody}</div>
                </div>
              {/if}
            </main>

            {#if relatedItems.length > 0}
              <aside
                class="wiki-related grid content-start gap-2"
                aria-label={widgetText.relatedPages}
              >
                <div
                  class="text-2xs font-semibold uppercase tracking-widest text-(--mono-muted)"
                >
                  {widgetText.relatedPages}
                </div>
                <div class="wiki-related-list grid gap-1.5">
                  {#each relatedItems as item}
                    <Tooltip
                      class="grid min-w-0 justify-stretch rounded-lg bg-(--mono-bg) px-3 py-2 text-left text-xs transition-colors hover:bg-(--mono-purple)/10"
                      onclick={() => openPage(item.id)}
                    >
                      {#snippet content()}
                        <div
                          class="text-3xs font-semibold uppercase tracking-[0.08em] text-(--mono-muted)"
                        >
                          {item.relation}
                        </div>
                        <div class="mt-1">{item.summary}</div>
                      {/snippet}
                      <span
                        class="flex min-w-0 items-center gap-2 font-medium text-(--mono-text)"
                      >
                        <Icon
                          icon={iconForWikiPage(item.id)}
                          size="sm"
                          class="text-(--mono-muted)"
                        />
                        <span class="truncate">{item.title}</span>
                      </span>
                    </Tooltip>
                  {/each}
                </div>
              </aside>
            {/if}
          </div>
        </div>
      </section>
    </div>
  </div>
</Card>

<style>
  .wiki-navigation.is-reader-open,
  .wiki-reader.is-empty {
    display: none;
  }
  @container (min-width: 672px) {
    .wiki-related-list {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  @container (min-width: 960px) {
    .wiki-layout {
      grid-template-columns: 15rem minmax(0, 1fr);
    }
    .wiki-navigation,
    .wiki-navigation.is-reader-open {
      display: grid;
    }
    .wiki-reader.is-empty {
      display: flex;
    }
  }
  @container (min-width: 1248px) {
    .wiki-reader-content {
      grid-template-columns: minmax(0, 1fr) 15rem;
    }
    .wiki-related-list {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  :global(.wiki-markdown) {
    display: block;
    line-height: 1.6;
    font-size: 1em;
  }
  :global(.wiki-markdown > *:first-child) {
    margin-top: 0 !important;
  }
  :global(.wiki-markdown > *:last-child) {
    margin-bottom: 0 !important;
  }
  :global(.wiki-markdown p),
  :global(.wiki-markdown ul),
  :global(.wiki-markdown ol),
  :global(.wiki-markdown blockquote),
  :global(.wiki-markdown pre),
  :global(.wiki-markdown table) {
    margin-top: calc(var(--widget-em) * 0.75);
    margin-bottom: calc(var(--widget-em) * 0.75);
  }
  :global(.wiki-markdown h1),
  :global(.wiki-markdown h2),
  :global(.wiki-markdown h3),
  :global(.wiki-markdown h4) {
    color: var(--mono-text);
    font-weight: 600;
    line-height: 1.3;
    margin-top: calc(var(--widget-em) * 1.5);
    margin-bottom: calc(var(--widget-em) * 0.75);
  }
  :global(.wiki-markdown h1) {
    font-size: 1.47em;
  }
  :global(.wiki-markdown h2) {
    font-size: 1.294em;
  }
  :global(.wiki-markdown h3) {
    font-size: 1.176em;
  }
  :global(.wiki-markdown p),
  :global(.wiki-markdown ul),
  :global(.wiki-markdown ol),
  :global(.wiki-markdown blockquote) {
    color: var(--mono-text);
  }
  :global(.wiki-markdown ul) {
    padding-left: calc(var(--widget-em) * 1.25);
    list-style-type: disc;
  }
  :global(.wiki-markdown ol) {
    padding-left: calc(var(--widget-em) * 1.25);
    list-style-type: decimal;
  }
  :global(.wiki-markdown li) {
    margin-top: calc(var(--widget-em) * 0.25);
  }
  :global(.wiki-markdown a) {
    color: var(--mono-purple);
    text-decoration: underline;
    text-underline-offset: 0.15em;
    word-break: break-word;
  }
  :global(.wiki-markdown code) {
    background: var(--mono-bg);
    border: 1px solid var(--mono-border);
    border-radius: calc(var(--widget-em) * 0.35);
    padding: calc(var(--widget-em) * 0.1) calc(var(--widget-em) * 0.25);
    font-size: 0.95em;
    line-height: 1.8;
    -webkit-box-decoration-break: clone;
    box-decoration-break: clone;
  }
  :global(.wiki-markdown pre) {
    background: var(--mono-bg);
    border: 1px solid var(--mono-border);
    border-radius: calc(var(--widget-em) * 0.75);
    overflow: auto;
    padding: calc(var(--widget-em) * 0.75);
  }
  :global(.wiki-markdown pre code) {
    background: transparent;
    border: none;
    padding: 0;
  }
  :global(.wiki-markdown blockquote) {
    border-left: 3px solid var(--mono-purple);
    padding-left: calc(var(--widget-em) * 0.75);
    color: var(--mono-muted);
    font-style: italic;
  }
  :global(.wiki-markdown hr) {
    border: 0;
    border-top: 1px solid var(--mono-border);
  }
  :global(.wiki-markdown table) {
    border-collapse: collapse;
    display: block;
    max-width: 100%;
    overflow: auto;
  }
  :global(.wiki-markdown th),
  :global(.wiki-markdown td) {
    border: 1px solid var(--mono-border);
    padding: calc(var(--widget-em) * 0.4) calc(var(--widget-em) * 0.55);
    text-align: left;
    vertical-align: top;
  }
</style>
