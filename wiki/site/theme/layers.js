/*
Domain: Native mdBook layer presentation
Owns: Menu-bar language switch, layer controls, eligible navigation and explicit cross-layer exits.
Excludes: Markdown rendering, membership inference, search ranking and parent-window access.
*/
(async () => {
  if (document.readyState === 'loading') await new Promise(resolve => document.addEventListener('DOMContentLoaded', resolve, { once: true }));
  const ru = document.documentElement.lang === 'ru';
  const labels = ru
    ? { layer: 'Слой знаний', all: 'Все', language: 'Язык', pages: 'Страниц', empty: 'В этом слое пока нет опубликованных страниц.', invalid: 'Неизвестный или повторяющийся параметр слоя.', unavailable: 'Не удалось загрузить данные слоёв. Фильтрация недоступна.', outside: 'Эта страница не входит в выбранный слой.', index: 'Открыть содержание слоя', reset: 'Открыть без фильтра', exit: 'Все материалы', exitHint: 'Вне выбранного слоя: откроется без фильтра', previous: 'Предыдущая глава', next: 'Следующая глава' }
    : { layer: 'Knowledge layer', all: 'All', language: 'Language', pages: 'Pages', empty: 'No knowledge pages are published in this layer.', invalid: 'Unknown or repeated layer parameter.', unavailable: 'Layer data could not be loaded. Filtering is unavailable.', outside: 'This page is outside the selected layer.', index: 'Open layer contents', reset: 'Open without a filter', exit: 'All knowledge', exitHint: 'Outside this layer: opens without a filter', previous: 'Previous chapter', next: 'Next chapter' };
  const languageNames = { en: 'English', ru: 'Русский' };
  const context = window.wikiContext;
  const main = document.querySelector('main');
  const bar = document.createElement('section');
  bar.className = 'wiki-context';
  bar.setAttribute('aria-label', labels.layer);
  const row = document.createElement('div');
  row.className = 'wiki-context-row';
  const status = document.createElement('p');
  status.className = 'wiki-context-status';
  status.setAttribute('role', 'status');
  bar.append(row, status);
  // The layer bar is shown only when it carries information: several layers, an active filter or an error.
  const mountBar = () => { if (!bar.isConnected) document.querySelector('#mdbook-menu-bar')?.after(bar); };
  function anchor(label, href, className) {
    const item = document.createElement('a');
    item.textContent = label;
    item.href = href;
    item.className = className;
    return item;
  }
  function clearNavigation() {
    for (const item of document.querySelectorAll('.nav-chapters, .mobile-nav-chapters')) item.remove();
  }
  function reject(message, state) {
    mountBar();
    document.documentElement.dataset.wikiLayer = state;
    status.textContent = message;
    status.setAttribute('role', 'alert');
    // This path also runs without reader context, so the reset link drops the layer key with plain
    // URL APIs instead of the state codec.
    const reset = new URL(location.href);
    const fragment = reset.hash.slice(1);
    const params = new URLSearchParams(fragment.startsWith('!') ? fragment.slice(1) : '');
    params.delete('layer');
    reset.hash = params.toString() === '' ? '' : `!${params.toString()}`;
    bar.append(anchor(labels.reset, reset.href, 'wiki-layer-recovery'));
    const search = document.querySelector('#mdbook-search-toggle');
    if (search) search.disabled = true;
    clearNavigation();
  }
  if (!context) {
    reject(labels.unavailable, 'error');
    return;
  }
  const { graph, locale, selection, view, withLayer, withState, readState, siteRoot } = context;
  // Back/Forward can restore a fragment whose layer view was never rendered in this document.
  addEventListener('popstate', () => {
    if (readState(location.href).layer !== selection) location.reload();
  });
  const base = graph.base_path || '/';
  const root = new URL(siteRoot);
  // Graph routes carry the build's base path; the reader works in paths relative to the actual site root,
  // so the same artifact serves from a host root, a GitHub Pages prefix or any static directory.
  const key = route => route.slice(base.length);
  const resolve = path => new URL(path, root).href;
  function relative(href) {
    const target = new URL(href, document.baseURI);
    if (target.origin !== root.origin || !target.pathname.startsWith(root.pathname)) return null;
    const path = target.pathname.slice(root.pathname.length);
    if (path === '' || path.endsWith('/')) return path + 'index.html';
    // A clean-URL server omits `.html`; the rendered file still carries it.
    return path.endsWith('.html') ? path : path + '.html';
  }
  const paths = new Map(graph.nodes.flatMap(node => Object.values(node.url).map(route => [key(route), node])));
  const currentPath = context.page ?? relative(location.href);
  const current = paths.get(currentPath);
  function allowed(path, layer = selection) {
    if (path === null) return null;
    const node = paths.get(path);
    if (node) return layer === 'all' || node.layers.includes(layer);
    for (const candidate of graph.locales) {
      if ([`${candidate}/index.html`, `${candidate}/404.html`].includes(path)) return true;
      for (const plane of graph.layers) if (path === `${candidate}/${plane.id}/index.html` && !graph.nodes.some(node => node.plane === plane.id)) return layer === 'all' || layer === plane.id;
    }
    return null;
  }
  function selectionUrl(layer) {
    if (locale && allowed(currentPath, layer) === false) {
      // A layer that cannot show this page opens its contents without a stale section or highlight.
      return withState(withLayer(resolve(`${locale}/index.html`), layer), { highlight: null, at: null }, location.href);
    }
    return withLayer(location.href, layer);
  }

  // Language switch in the menu bar, keeping the current page, layer and search state.
  if (locale) {
    try { localStorage.setItem('wiki-locale', locale); } catch {}
    const languages = document.createElement('nav');
    languages.className = 'wiki-locale-choices';
    languages.setAttribute('aria-label', labels.language);
    const state = readState(location.href);
    for (const candidate of graph.locales) {
      const target = current?.url[candidate] ? key(current.url[candidate]) : `${candidate}/index.html`;
      const url = new URL(resolve(target));
      const link = anchor(candidate.toUpperCase(), withState(withLayer(url.href, state.layer), { search: state.search, highlight: null, at: null }, location.href), 'wiki-locale-link');
      link.lang = candidate;
      link.hreflang = candidate;
      link.title = languageNames[candidate] ?? candidate;
      link.setAttribute('aria-label', link.title);
      if (candidate === locale) link.setAttribute('aria-current', 'true');
      languages.append(link);
    }
    const buttons = document.querySelector('#mdbook-menu-bar .right-buttons');
    if (buttons) buttons.prepend(languages);
    else row.append(languages);
  }

  if (graph.layers.length > 1 || selection !== 'all') {
    mountBar();
    const choices = document.createElement('nav');
    choices.className = 'wiki-layer-choices';
    choices.setAttribute('aria-label', labels.layer);
    for (const item of [{ id: 'all', title: labels.all }, ...graph.layers]) {
      const link = anchor(item.id === 'all' ? item.title : `#${item.id}`, selectionUrl(item.id), 'wiki-layer-choice');
      link.dataset.layer = item.id;
      if (item.id !== 'all') link.title = item.title;
      if (selection === item.id) link.setAttribute('aria-current', 'true');
      choices.append(link);
    }
    row.prepend(choices);
  }
  if (selection === null) {
    reject(labels.invalid, 'error');
    return;
  }
  if (locale && allowed(currentPath) === false) {
    reject(labels.outside, 'outside');
    bar.append(anchor(labels.index, withLayer(resolve(`${locale}/index.html`), selection), 'wiki-layer-recovery'));
    return;
  }
  status.textContent = `${labels.pages}: ${view.nodes.length}${view.nodes.length === 0 ? ' · ' + labels.empty : ''}`;
  const selectedIds = new Set(view.nodes.map(node => node.id));
  function adaptLink(link) {
    const raw = link.getAttribute('href');
    if (!raw || raw.startsWith('#') || link.matches('.wiki-layer-choice, .wiki-locale-link, .wiki-layer-recovery, .wiki-open-current, .wiki-tag')) return;
    const url = new URL(raw, document.baseURI);
    const eligible = allowed(relative(url.href));
    if (eligible === null) return;
    link.href = withLayer(url.href, eligible ? selection : 'all');
    if (!eligible) {
      link.dataset.wikiLayerExit = labels.exit;
      link.title = labels.exitHint;
      link.setAttribute('aria-label', link.textContent + ' · ' + labels.exitHint);
    }
  }
  // Work only in compiler-marked chrome regions; authored tables/lists remain untouched.
  const comments = document.createTreeWalker(main, NodeFilter.SHOW_COMMENT);
  let comment;
  while ((comment = comments.nextNode())) {
    const kind = comment.data.trim();
    if (kind !== 'wiki-pages:start') continue;
    const region = [];
    for (let node = comment.nextSibling; node && !(node.nodeType === Node.COMMENT_NODE && node.data.trim() === kind.replace(':start', ':end')); node = node.nextSibling) {
      if (node.nodeType === Node.ELEMENT_NODE) region.push(node);
    }
    for (const node of region) for (const item of node.querySelectorAll('li')) item.hidden = allowed(relative(item.querySelector('a').href)) === false;
    // Hide a contents heading once every entry beneath it is filtered out.
    const level = node => /^H[2-6]$/.test(node.tagName) ? Number(node.tagName[1]) : 0;
    region.forEach((node, index) => {
      if (!level(node)) return;
      let visible = false;
      for (const next of region.slice(index + 1)) {
        if (level(next) && level(next) <= level(node)) break;
        if (next.matches('ul, ol') && Array.from(next.children).some(item => !item.hidden)) { visible = true; break; }
      }
      node.hidden = !visible;
    });
  }
  // The sidebar is a tree: a section node stays visible while any page beneath it does.
  function filterTree(list) {
    let any = false;
    for (const item of list.children) {
      if (item.tagName !== 'LI') continue;
      const link = item.querySelector(':scope > .chapter-link-wrapper > a:not(.chapter-fold-toggle)');
      const children = item.querySelector(':scope > ol');
      const below = children ? filterTree(children) : false;
      const own = link ? allowed(relative(link.href)) !== false : false;
      const visible = link ? own || below : children ? below : !!item.textContent.trim();
      item.hidden = !visible;
      any ||= visible;
    }
    return any;
  }
  const sidebar = document.querySelector('.sidebar-scrollbox .chapter');
  if (sidebar) {
    filterTree(sidebar);
    // mdBook marks the current chapter by comparing against location, which misses a directory index
    // served without its trailing slash. The rendered chapter is known exactly, so mark it from that.
    for (const link of sidebar.querySelectorAll('.chapter-link-wrapper > a:not(.chapter-fold-toggle)')) {
      const raw = link.getAttribute('href');
      if (!raw || raw.startsWith('#') || relative(link.href) !== currentPath || link.classList.contains('active')) continue;
      link.classList.add('active');
      for (let parent = link.parentElement; parent; parent = parent.parentElement) if (parent.matches?.('li.chapter-item')) parent.classList.add('expanded');
    }
  }
  if (locale) {
    const sequence = context.chapterOrder(graph, locale, selection).map(key);
    const position = sequence.indexOf(currentPath);
    for (const [direction, offset] of [['previous', -1], ['next', 1]]) {
      const destination = position < 0 ? undefined : sequence[position + offset];
      for (const link of document.querySelectorAll(`.nav-chapters.${direction}, .mobile-nav-chapters.${direction}`)) {
        if (!destination) link.remove();
        else {
          link.href = withLayer(resolve(destination), selection);
          link.title = direction === 'next' ? labels.next : labels.previous;
          link.setAttribute('aria-label', link.title);
        }
      }
    }
  }
  for (const link of document.querySelectorAll('a[href]')) adaptLink(link);
  // Reader state lives in the fragment, so a same-document fragment change is a state change. The
  // layer view, sidebar and search index are derived once per document, so a layer change reloads,
  // and a plain `#section` click must carry the current state instead of replacing it.
  document.addEventListener('click', event => {
    const link = event.target.closest?.('a[href]');
    if (!link || event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    const url = new URL(link.href, location.href);
    if (url.origin !== location.origin || url.pathname !== location.pathname || url.search !== location.search) return;
    if (link.matches('.wiki-layer-choice, .wiki-layer-recovery')) {
      event.preventDefault();
      location.hash = url.hash;
      location.reload();
      return;
    }
    const href = link.getAttribute('href') ?? '';
    if (!href.startsWith('#')) return;
    const state = readState(location.href);
    if (!state.stateful) return;
    let at = null;
    try { at = href.length > 1 ? decodeURIComponent(href.slice(1)) : null; } catch { at = href.slice(1); }
    event.preventDefault();
    history.pushState({}, document.title, withState(location.href, { at }));
    const target = at === null ? null : document.getElementById(at);
    if (target) target.scrollIntoView();
    else window.scrollTo(0, 0);
  }, true);
  // Native search creates result links later and changes its own query URL without an event.
  for (const type of ['pointerdown', 'focusin', 'click', 'auxclick', 'contextmenu']) document.addEventListener(type, event => {
    const link = event.target.closest?.('a[href]');
    if (link) adaptLink(link);
  }, true);
  document.documentElement.dataset.wikiLayer = 'ready';
})();
