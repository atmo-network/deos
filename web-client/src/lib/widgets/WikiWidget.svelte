<script lang="ts">
  import aliases from "../../../../wiki/_meta/aliases.json";
  import graph from "../../../../wiki/_meta/graph.json";
  import navigation from "../../../../wiki/_meta/navigation.json";
  import wikiStateJson from "../../../../wiki/_meta/state.json";
  import { onMount } from "svelte";

  import { Card, TextField } from "$lib/shared/ui";
  import { loadTrustedWikiPage, type TrustedWikiPage } from "$lib/wiki/trusted";

  type LocalizedValue = Record<string, string>;

  type WikiGraphNode = {
    id: string;
    title: LocalizedValue;
    page_type: string;
    path: LocalizedValue;
  };

  type WikiGraphEdge = {
    from: string;
    to: string;
    type: string;
  };

  type WikiGraphManifest = {
    default_locale: string;
    available_locales: string[];
    nodes: WikiGraphNode[];
    edges: WikiGraphEdge[];
  };

  type WikiAliasManifest = {
    default_locale: string;
    available_locales: string[];
    aliases: Record<string, Record<string, string>>;
  };

  type WikiStatePage = {
    path: LocalizedValue;
    page_type: string;
    title: LocalizedValue;
    status: string;
    audience: string;
    confidence: number;
    sources: Record<string, string[]>;
  };

  type WikiStateManifest = {
    generated_at: string;
    mode: string;
    source_root: string;
    default_locale: string;
    available_locales: string[];
    pages: Record<string, WikiStatePage>;
  };

  type WikiNavigationItem = {
    id: string;
    title: LocalizedValue;
    path: LocalizedValue;
    page_type: string;
    summary: LocalizedValue;
  };

  type WikiNavigationSection = {
    id: string;
    title: LocalizedValue;
    items: WikiNavigationItem[];
  };

  type WikiNavigationManifest = {
    default_locale: string;
    available_locales: string[];
    entrypoints: string[];
    sections: WikiNavigationSection[];
  };

  type ResolvedWikiNavigationItem = {
    id: string;
    title: string;
    path: string;
    page_type: string;
    summary: string;
  };

  type ResolvedWikiNavigationSection = {
    id: string;
    title: string;
    items: ResolvedWikiNavigationItem[];
  };

  type RelatedWikiItem = {
    id: string;
    title: string;
    path: string;
    summary: string;
    relation: string;
  };

  type WikiSearchAliasMatch = {
    id: string;
    aliases: string[];
  };

  const wikiAliases = aliases as WikiAliasManifest;
  const wikiGraph = graph as WikiGraphManifest;
  const wikiNavigation = navigation as WikiNavigationManifest;
  const wikiState = wikiStateJson as WikiStateManifest;
  const featuredPageIds = new Set(wikiNavigation.entrypoints);
  const availableLocales = wikiNavigation.available_locales;

  let rootEl = $state<HTMLDivElement | null>(null);
  let viewport = $state({ width: 0, height: 0 });
  let currentLocale = $state(wikiNavigation.default_locale);
  let searchQuery = $state("");
  let hoveredWikiPath = $state<string | null>(null);
  let selectedPageId = $state<string | null>(null);
  let selectedPage = $state<TrustedWikiPage | null>(null);
  let selectedError = $state<string | null>(null);
  let loadingPage = $state(false);
  let pageRequestSerial = 0;

  function syncViewport() {
    if (!rootEl) {
      viewport = { width: 0, height: 0 };
      return;
    }
    viewport = {
      width: rootEl.clientWidth,
      height: rootEl.clientHeight,
    };
  }

  function resolveLocale(candidate: string | null | undefined) {
    if (!candidate) {
      return wikiNavigation.default_locale;
    }
    if (availableLocales.includes(candidate)) {
      return candidate;
    }
    const baseLocale = candidate.split("-")[0];
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
      ""
    );
  }

  function normalizeSearchText(value: string) {
    return value.trim().toLocaleLowerCase(currentLocale);
  }

  function formatRelationLabel(value: string) {
    return value.replace(/-/g, " ");
  }

  function formatConfidence(value: number) {
    return `${Math.round(value * 100)}%`;
  }

  function localizedSources(value: Record<string, string[]>) {
    return value[currentLocale] ?? value[wikiState.default_locale] ?? [];
  }

  function repoWikiPath(path: string) {
    return `wiki/${path}`;
  }

  function getDefaultPageId() {
    return (
      wikiNavigation.entrypoints[0] ??
      wikiNavigation.sections[0]?.items[0]?.id ??
      null
    );
  }

  async function copyPath(path: string) {
    try {
      await navigator.clipboard.writeText(path);
    } catch {}
  }

  function openPage(itemId: string) {
    selectedPageId = itemId;
  }

  function closePage() {
    selectedPageId = null;
  }

  function anchorFromEventTarget(target: EventTarget | null) {
    if (!(target instanceof HTMLElement)) {
      return null;
    }
    const anchor = target.closest("a");
    return anchor instanceof HTMLAnchorElement ? anchor : null;
  }

  function syncHoveredWikiPreview(target: EventTarget | null) {
    const anchor = anchorFromEventTarget(target);
    hoveredWikiPath = anchor?.dataset.wikiPath ?? null;
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
    const repoPath = anchor.dataset.repoPath;
    if (!repoPath) {
      return;
    }
    event.preventDefault();
    void copyPath(repoPath);
  }

  function handleRenderedContentMouseover(event: MouseEvent) {
    syncHoveredWikiPreview(event.target);
  }

  function handleRenderedContentFocusin(event: FocusEvent) {
    syncHoveredWikiPreview(event.target);
  }

  function clearHoveredWikiPreview() {
    hoveredWikiPath = null;
  }

  function bindRenderedContentInteractions(node: HTMLDivElement) {
    node.addEventListener("click", handleRenderedContentClick);
    node.addEventListener("mouseover", handleRenderedContentMouseover);
    node.addEventListener("focusin", handleRenderedContentFocusin);
    node.addEventListener("mouseleave", clearHoveredWikiPreview);
    node.addEventListener("focusout", clearHoveredWikiPreview);
    return {
      destroy() {
        node.removeEventListener("click", handleRenderedContentClick);
        node.removeEventListener("mouseover", handleRenderedContentMouseover);
        node.removeEventListener("focusin", handleRenderedContentFocusin);
        node.removeEventListener("mouseleave", clearHoveredWikiPreview);
        node.removeEventListener("focusout", clearHoveredWikiPreview);
      },
    };
  }

  const sections = $derived<ResolvedWikiNavigationSection[]>(
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
  const aliasTermsById = $derived.by<Map<string, string[]>>(() => {
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
  const aliasMatchesById = $derived.by<Map<string, string[]>>(() => {
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
            items: section.items.filter((item) =>
              [
                item.title,
                item.summary,
                item.path,
                section.title,
                ...(aliasTermsById.get(item.id) ?? []),
              ].some((value) =>
                normalizeSearchText(value).includes(normalizedSearchQuery),
              ),
            ),
          }))
          .filter((section) => section.items.length > 0),
  );
  const filteredItemCount = $derived(
    filteredSections.reduce(
      (count, section) => count + section.items.length,
      0,
    ),
  );
  const allItemsById = $derived(
    new Map(allItems.map((item) => [item.id, item])),
  );
  const hoveredWikiItem = $derived(
    allItems.find((item) => item.path === hoveredWikiPath) ?? null,
  );
  const selectedItem = $derived(
    allItems.find((item) => item.id === selectedPageId) ?? null,
  );
  const selectedPageState = $derived(
    selectedPageId ? (wikiState.pages[selectedPageId] ?? null) : null,
  );
  const selectedPageSources = $derived(
    selectedPageState ? localizedSources(selectedPageState.sources) : [],
  );
  const relatedItems = $derived.by<RelatedWikiItem[]>(() => {
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
          relation: [...labels].join(" · "),
        };
      })
      .filter((item): item is RelatedWikiItem => item !== null)
      .sort((left, right) =>
        left.title.localeCompare(right.title, currentLocale),
      );
  });
  const dualPane = $derived(viewport.width >= 960);
  const showReaderOnly = $derived(!dualPane && selectedItem !== null);

  const widgetText = $derived(
    currentLocale === "ru"
      ? {
          title: "Wiki",
          subtitle:
            "Сгенерированная wiki-навигация и рендер trusted repo-local markdown",
          pages: "Страниц",
          search: "Фильтр wiki",
          searchPlaceholder: "Искать по title, summary или path",
          searchHelper:
            "Поиск идет только по сгенерированным navigation + aliases manifest, а не по архивному full-text индексу.",
          noMatchesTitle: "Совпадений не найдено",
          noMatchesBody:
            "Измените запрос или очистите фильтр, чтобы снова увидеть разделы wiki.",
          linkPreview: "Связанная страница",
          relatedPages: "Связанные страницы",
          provenance: "Собранная provenance",
          status: "Статус",
          confidence: "Уверенность",
          generatedAt: "Собрано",
          sources: "Источники",
          aliasMatch: "Alias",
          clearSearch: "Очистить",
          copyPath: "Копировать путь",
          back: "Назад к навигации",
          loading: "Загрузка wiki-страницы...",
          emptyTitle: "Выберите страницу",
          emptyBody:
            "Откройте любую wiki-страницу из навигации, чтобы увидеть её содержимое прямо в клиенте.",
          loadError: "Не удалось загрузить wiki-страницу",
          trustedHint:
            "Repo-local trusted markdown rendered in-browser via marked",
        }
      : {
          title: "Wiki",
          subtitle:
            "Generated wiki navigation and trusted repo-local markdown rendering",
          pages: "Pages",
          search: "Wiki filter",
          searchPlaceholder: "Search title, summary, or path",
          searchHelper:
            "This search only scans the generated navigation + aliases manifests, not a full-text archive index.",
          noMatchesTitle: "No matches",
          noMatchesBody:
            "Adjust the query or clear the filter to restore the full wiki navigation graph.",
          linkPreview: "Linked page",
          relatedPages: "Related pages",
          provenance: "Compiled provenance",
          status: "Status",
          confidence: "Confidence",
          generatedAt: "Generated",
          sources: "Sources",
          aliasMatch: "Alias",
          clearSearch: "Clear",
          copyPath: "Copy path",
          back: "Back to navigation",
          loading: "Loading wiki page...",
          emptyTitle: "Choose a page",
          emptyBody:
            "Open any wiki page from the navigation list to render its content directly in the client.",
          loadError: "Failed to load wiki page",
          trustedHint:
            "Repo-local trusted markdown rendered in-browser via marked",
        },
  );

  $effect(() => {
    const pagePath = selectedItem?.path ?? null;
    const requestId = ++pageRequestSerial;
    if (!pagePath) {
      loadingPage = false;
      selectedError = null;
      selectedPage = null;
      return;
    }
    loadingPage = true;
    selectedError = null;
    selectedPage = null;
    void loadTrustedWikiPage(pagePath)
      .then((page) => {
        if (requestId !== pageRequestSerial) {
          return;
        }
        selectedPage = page;
        loadingPage = false;
      })
      .catch((error: unknown) => {
        if (requestId !== pageRequestSerial) {
          return;
        }
        selectedError = error instanceof Error ? error.message : String(error);
        loadingPage = false;
      });
  });

  onMount(() => {
    currentLocale = resolveLocale(navigator.language);
    selectedPageId = getDefaultPageId();
    syncViewport();
    if (!rootEl) {
      return;
    }
    const resizeObserver = new ResizeObserver(() => syncViewport());
    resizeObserver.observe(rootEl);
    return () => resizeObserver.disconnect();
  });
</script>

<Card class="min-h-full flex flex-col">
  <div bind:this={rootEl} class="h-full flex flex-col min-h-0">
    <div
      class={[
        "min-h-0 flex-1 p-3",
        dualPane ? "grid gap-3 grid-cols-[15rem_minmax(0,1fr)]" : "grid gap-3",
      ]}
    >
      {#if dualPane || !showReaderOnly}
        <div class="min-h-0 overflow-y-auto pr-2 grid content-start gap-4">
          <section class="grid gap-2">
            <TextField
              label={widgetText.search}
              bind:value={searchQuery}
              placeholder={widgetText.searchPlaceholder}
              helper={widgetText.searchHelper}
              inputClass="px-2 py-1.5 text-xs"
            />
            <div
              class="flex items-center justify-between gap-2 px-1 text-[10px] text-(--mono-muted)"
            >
              <span
                >{filteredItemCount} / {allItems.length}
                {widgetText.pages}</span
              >
              {#if searchQuery}
                <button
                  class="rounded-lg border border-(--mono-border) px-2 py-1 text-[10px] text-(--mono-text) hover:border-(--mono-purple)"
                  onclick={() => (searchQuery = "")}
                >
                  {widgetText.clearSearch}
                </button>
              {/if}
            </div>
          </section>
          {#if filteredSections.length === 0}
            <div
              class="rounded-xl border border-dashed border-(--mono-border) p-3 text-xs text-(--mono-muted)"
            >
              <div class="font-medium text-(--mono-text)">
                {widgetText.noMatchesTitle}
              </div>
              <div class="mt-1">{widgetText.noMatchesBody}</div>
            </div>
          {:else}
            {#each filteredSections as section}
              <section class="grid gap-1">
                <div
                  class="font-semibold text-(--mono-text) mb-1 px-2 uppercase tracking-widest text-[10px]"
                >
                  {section.title}
                </div>
                <div class="grid gap-0.5 text-xs">
                  {#each section.items as item}
                    <button
                      class={[
                        "text-left px-2 py-1.5 rounded-md hover:bg-(--mono-bg) transition-colors flex items-center justify-between gap-2",
                        item.id === selectedPageId
                          ? "bg-(--mono-bg) font-medium text-(--mono-purple)"
                          : "text-(--mono-text) hover:text-(--mono-purple)",
                      ]}
                      onclick={() => openPage(item.id)}
                    >
                      <span class="min-w-0 grid gap-0.5">
                        <span class="truncate">{item.title}</span>
                        {#if searchQuery}
                          <span class="truncate text-[10px] text-(--mono-muted)"
                            >{item.summary}</span
                          >
                          {#if (aliasMatchesById.get(item.id) ?? []).length > 0}
                            <span
                              class="flex flex-wrap items-center gap-1 text-[10px] text-(--mono-muted)"
                            >
                              <span class="uppercase tracking-[0.08em]"
                                >{widgetText.aliasMatch}</span
                              >
                              {#each aliasMatchesById.get(item.id) ?? [] as aliasTerm}
                                <span
                                  class="rounded-full border border-(--mono-border) bg-white px-1.5 py-0.5 text-[10px] text-(--mono-text)"
                                  >{aliasTerm}</span
                                >
                              {/each}
                            </span>
                          {/if}
                        {/if}
                      </span>
                      {#if featuredPageIds.has(item.id)}
                        <span
                          class="shrink-0 text-[8px] uppercase tracking-widest text-(--mono-purple)"
                        >
                          ★
                        </span>
                      {/if}
                    </button>
                  {/each}
                </div>
              </section>
            {/each}
          {/if}
        </div>
      {/if}
      {#if dualPane || showReaderOnly}
        <section
          class="min-h-0 rounded-xl border border-(--mono-border) bg-white flex flex-col overflow-hidden"
        >
          <div class="border-b border-(--mono-border) px-3 py-2 grid gap-2">
            <div class="flex items-start justify-between gap-3">
              <div class="grid gap-1 min-w-0">
                {#if !dualPane && selectedItem}
                  <button
                    class="justify-self-start rounded-lg border border-(--mono-border) px-2 py-1 text-[10px] text-(--mono-text) hover:border-(--mono-purple)"
                    onclick={closePage}
                  >
                    {widgetText.back}
                  </button>
                {/if}
                <div class="font-medium text-(--mono-text)">
                  {selectedItem?.title ?? widgetText.emptyTitle}
                </div>
                <div class="text-[10px] text-(--mono-muted)">
                  {selectedItem?.summary ?? widgetText.emptyBody}
                </div>
              </div>
              {#if selectedItem}
                <button
                  class="shrink-0 rounded-lg border border-(--mono-border) px-2 py-1 text-[10px] text-(--mono-text) hover:border-(--mono-purple)"
                  onclick={() => copyPath(repoWikiPath(selectedItem.path))}
                >
                  {widgetText.copyPath}
                </button>
              {/if}
            </div>
            {#if selectedItem}
              <div class="text-[10px] text-(--mono-muted) tabnum break-all">
                {repoWikiPath(selectedItem.path)}
              </div>
            {/if}
            <div class="text-[10px] text-(--mono-muted)">
              {widgetText.trustedHint}
            </div>
          </div>
          <div
            class="min-h-0 overflow-auto p-4 sm:p-5 grid content-start gap-3"
          >
            {#if hoveredWikiItem}
              <div
                class={[
                  "rounded-xl border border-(--mono-border) bg-(--mono-bg) p-3 text-xs text-(--mono-text)",
                  !dualPane && "order-last",
                ]}
              >
                <div
                  class="text-[10px] uppercase tracking-widest text-(--mono-muted)"
                >
                  {widgetText.linkPreview}
                </div>
                <div class="mt-1 font-medium">{hoveredWikiItem.title}</div>
                <div class="mt-1 text-(--mono-muted)">
                  {hoveredWikiItem.summary}
                </div>
                <div
                  class="mt-2 text-[10px] text-(--mono-muted) tabnum break-all"
                >
                  {repoWikiPath(hoveredWikiItem.path)}
                </div>
              </div>
            {/if}
            {#if relatedItems.length > 0}
              <div
                class={[
                  "rounded-xl border border-(--mono-border) bg-(--mono-bg) p-3 grid gap-2 text-xs text-(--mono-text)",
                  !dualPane && "order-last",
                ]}
              >
                <div
                  class="text-[10px] uppercase tracking-widest text-(--mono-muted)"
                >
                  {widgetText.relatedPages}
                </div>
                <div class="grid gap-1.5">
                  {#each relatedItems as item}
                    <button
                      class="rounded-lg border border-(--mono-border) bg-white px-3 py-2 text-left hover:border-(--mono-purple)"
                      onclick={() => openPage(item.id)}
                    >
                      <div class="flex items-start justify-between gap-2">
                        <div class="min-w-0 grid gap-0.5">
                          <div class="truncate font-medium text-(--mono-text)">
                            {item.title}
                          </div>
                          <div
                            class="text-[10px] uppercase tracking-[0.08em] text-(--mono-muted)"
                          >
                            {item.relation}
                          </div>
                        </div>
                      </div>
                      <div
                        class="mt-1 line-clamp-2 text-[10px] text-(--mono-muted)"
                      >
                        {item.summary}
                      </div>
                    </button>
                  {/each}
                </div>
              </div>
            {/if}
            {#if selectedPageState}
              <div
                class={[
                  "rounded-xl border border-(--mono-border) bg-(--mono-bg) p-3 grid gap-2 text-xs text-(--mono-text)",
                  !dualPane && "order-last",
                ]}
              >
                <div
                  class="text-[10px] uppercase tracking-widest text-(--mono-muted)"
                >
                  {widgetText.provenance}
                </div>
                <div class="grid gap-1 sm:grid-cols-3">
                  <div
                    class="rounded-lg border border-(--mono-border) bg-white px-3 py-2"
                  >
                    <div
                      class="text-[10px] uppercase tracking-[0.08em] text-(--mono-muted)"
                    >
                      {widgetText.status}
                    </div>
                    <div class="mt-1 font-medium">
                      {selectedPageState.status}
                    </div>
                  </div>
                  <div
                    class="rounded-lg border border-(--mono-border) bg-white px-3 py-2"
                  >
                    <div
                      class="text-[10px] uppercase tracking-[0.08em] text-(--mono-muted)"
                    >
                      {widgetText.confidence}
                    </div>
                    <div class="mt-1 font-medium">
                      {formatConfidence(selectedPageState.confidence)}
                    </div>
                  </div>
                  <div
                    class="rounded-lg border border-(--mono-border) bg-white px-3 py-2"
                  >
                    <div
                      class="text-[10px] uppercase tracking-[0.08em] text-(--mono-muted)"
                    >
                      {widgetText.generatedAt}
                    </div>
                    <div class="mt-1 font-medium">{wikiState.generated_at}</div>
                  </div>
                </div>
                {#if selectedPageSources.length > 0}
                  <div class="grid gap-1">
                    <div
                      class="text-[10px] uppercase tracking-[0.08em] text-(--mono-muted)"
                    >
                      {widgetText.sources}
                    </div>
                    <div class="grid gap-1">
                      {#each selectedPageSources as sourcePath}
                        <button
                          class="rounded-lg border border-(--mono-border) bg-white px-3 py-2 text-left text-[10px] text-(--mono-muted) hover:border-(--mono-purple)"
                          onclick={() => copyPath(sourcePath)}
                        >
                          {sourcePath}
                        </button>
                      {/each}
                    </div>
                  </div>
                {/if}
              </div>
            {/if}
            {#if loadingPage}
              <div
                class="rounded-xl border border-dashed border-(--mono-border) p-3 text-xs text-(--mono-muted)"
              >
                {widgetText.loading}
              </div>
            {:else if selectedError}
              <div
                class="rounded-xl border border-red-300 bg-red-50 p-3 text-xs text-red-700"
              >
                <div class="font-medium">{widgetText.loadError}</div>
                <div class="mt-1 break-all">{selectedError}</div>
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
          </div>
        </section>
      {/if}
    </div>
  </div>
</Card>

<style>
  :global(.wiki-markdown) {
    display: block;
    line-height: 1.6;
    font-size: 0.85rem;
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
    margin-top: 0.75rem;
    margin-bottom: 0.75rem;
  }
  :global(.wiki-markdown h1),
  :global(.wiki-markdown h2),
  :global(.wiki-markdown h3),
  :global(.wiki-markdown h4) {
    color: var(--mono-text);
    font-weight: 600;
    line-height: 1.3;
    margin-top: 1.5rem;
    margin-bottom: 0.75rem;
  }
  :global(.wiki-markdown h1) {
    font-size: 1.25rem;
    border-bottom: 1px solid var(--mono-border);
    padding-bottom: 0.3rem;
  }
  :global(.wiki-markdown h2) {
    font-size: 1.1rem;
    border-bottom: 1px solid var(--mono-border);
    padding-bottom: 0.2rem;
  }
  :global(.wiki-markdown h3) {
    font-size: 1rem;
  }
  :global(.wiki-markdown p),
  :global(.wiki-markdown ul),
  :global(.wiki-markdown ol),
  :global(.wiki-markdown blockquote) {
    color: var(--mono-text);
  }
  :global(.wiki-markdown ul) {
    padding-left: 1.25rem;
    list-style-type: disc;
  }
  :global(.wiki-markdown ol) {
    padding-left: 1.25rem;
    list-style-type: decimal;
  }
  :global(.wiki-markdown li) {
    margin-top: 0.25rem;
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
    border-radius: 0.35rem;
    padding: 0.1rem 0.25rem;
    font-size: 0.95em;
  }
  :global(.wiki-markdown pre) {
    background: var(--mono-bg);
    border: 1px solid var(--mono-border);
    border-radius: 0.75rem;
    overflow: auto;
    padding: 0.75rem;
  }
  :global(.wiki-markdown pre code) {
    background: transparent;
    border: none;
    padding: 0;
  }
  :global(.wiki-markdown blockquote) {
    border-left: 3px solid var(--mono-purple);
    padding-left: 0.75rem;
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
    padding: 0.4rem 0.55rem;
    text-align: left;
    vertical-align: top;
  }
</style>
