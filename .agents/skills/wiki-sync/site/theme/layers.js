/*
Domain: Native mdBook layer presentation
Owns: Visible layer/locale controls, eligible navigation, graph rows and explicit cross-layer exits.
Excludes: Markdown rendering, membership inference, search ranking and parent-window access.
*/
(async () => {
  if (document.readyState === 'loading') await new Promise(resolve => document.addEventListener('DOMContentLoaded', resolve, { once: true }));
  const ru = document.documentElement.lang === 'ru';
  const labels = ru
    ? { layer: 'Слой знаний', all: 'Все', language: 'Язык', pages: 'Страниц', empty: 'В этом слое пока нет опубликованных страниц.', invalid: 'Неизвестный или повторяющийся параметр слоя.', unavailable: 'Не удалось загрузить данные слоёв. Фильтрация недоступна.', outside: 'Эта страница не входит в выбранный слой.', index: 'Открыть список слоя', reset: 'Открыть без фильтра', exit: 'Все материалы', exitHint: 'Вне выбранного слоя: откроется без фильтра', relations: 'В этом представлении нет связей.', previous: 'Предыдущая глава', next: 'Следующая глава' }
    : { layer: 'Knowledge layer', all: 'All', language: 'Language', pages: 'Pages', empty: 'No knowledge pages are published in this layer.', invalid: 'Unknown or repeated layer parameter.', unavailable: 'Layer data could not be loaded. Filtering is unavailable.', outside: 'This page is outside the selected layer.', index: 'Open layer index', reset: 'Open without a filter', exit: 'All knowledge', exitHint: 'Outside this layer: opens without a filter', relations: 'No relations in this view.', previous: 'Previous chapter', next: 'Next chapter' };
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
  document.querySelector('#mdbook-menu-bar')?.after(bar);
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
    document.documentElement.dataset.wikiLayer = state;
    status.textContent = message;
    status.setAttribute('role', 'alert');
    const reset = new URL(location.href);
    reset.searchParams.delete('layer');
    bar.append(anchor(labels.reset, reset.href, 'wiki-layer-recovery'));
    const search = document.querySelector('#mdbook-search-toggle');
    if (search) search.disabled = true;
    clearNavigation();
  }
  if (!context) {
    reject(labels.unavailable, 'error');
    return;
  }
  const { graph, locale, selection, view, withLayer } = context;
  const paths = new Map(graph.nodes.flatMap(node => Object.values(node.url).map(path => [path, node])));
  const normalize = path => path.endsWith('/') ? path + 'index.html' : path;
  const currentPath = normalize(location.pathname);
  const current = paths.get(currentPath);
  function allowed(path, layer = selection) {
    path = normalize(path);
    const node = paths.get(path);
    if (node) return layer === 'all' || node.layers.includes(layer);
    for (const candidate of graph.locales) {
      if ([`/${candidate}/index.html`, `/${candidate}/graph.html`, `/${candidate}/404.html`].includes(path)) return true;
      if (path === `/${candidate}/print.html`) return layer === 'all';
      for (const plane of graph.layers) if (path === `/${candidate}/${plane.id}/index.html` && !graph.nodes.some(node => node.plane === plane.id)) return layer === 'all' || layer === plane.id;
    }
    return null;
  }
  function selectionUrl(layer) {
    const url = new URL(withLayer(location.href, layer));
    if (locale && allowed(currentPath, layer) === false) {
      url.pathname = `/${locale}/index.html`;
      url.hash = '';
      url.searchParams.delete('highlight');
    }
    return url.href;
  }
  const choices = document.createElement('nav');
  choices.className = 'wiki-layer-choices';
  choices.setAttribute('aria-label', labels.layer);
  for (const item of [{ id: 'all', title: labels.all }, ...graph.layers]) {
    const link = anchor(item.id === 'all' ? item.title : `#${item.id}`, selectionUrl(item.id), 'wiki-layer-choice');
    link.dataset.layer = item.id;
    if (selection === item.id) link.setAttribute('aria-current', 'true');
    choices.append(link);
  }
  const languages = document.createElement('nav');
  languages.className = 'wiki-locale-choices';
  languages.setAttribute('aria-label', labels.language);
  for (const candidate of graph.locales) {
    const url = new URL(location.href);
    url.pathname = current ? current.url[candidate] : locale && currentPath === `/${locale}/graph.html` ? `/${candidate}/graph.html` : `/${candidate}/index.html`;
    url.hash = '';
    url.searchParams.delete('highlight');
    const link = anchor(candidate === 'ru' ? 'Русский' : 'English', url.href, 'wiki-locale-link');
    if (candidate === locale) link.setAttribute('aria-current', 'true');
    languages.append(link);
  }
  row.append(choices, languages);
  if (selection === null) {
    reject(labels.invalid, 'error');
    return;
  }
  if (locale && allowed(currentPath) === false) {
    reject(labels.outside, 'outside');
    bar.append(anchor(labels.index, withLayer(new URL(`/${locale}/index.html`, location.href).href, selection), 'wiki-layer-recovery'));
    return;
  }
  status.textContent = `${labels.pages}: ${view.nodes.length}${view.nodes.length === 0 ? ' · ' + labels.empty : ''}`;
  const selectedIds = new Set(view.nodes.map(node => node.id));
  function adaptLink(link) {
    const raw = link.getAttribute('href');
    if (!raw || raw.startsWith('#') || link.matches('.wiki-layer-choice, .wiki-locale-link, .wiki-layer-recovery, .wiki-open-current')) return;
    const url = new URL(raw, location.href);
    if (url.origin !== location.origin) return;
    const eligible = allowed(url.pathname);
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
    if (!['wiki-graph:start', 'wiki-pages:start'].includes(kind)) continue;
    for (let node = comment.nextSibling; node && !(node.nodeType === Node.COMMENT_NODE && node.data.trim() === kind.replace(':start', ':end')); node = node.nextSibling) {
      if (node.nodeType !== Node.ELEMENT_NODE) continue;
      if (kind === 'wiki-pages:start') for (const item of node.querySelectorAll('li')) item.hidden = allowed(new URL(item.querySelector('a').href).pathname) === false;
      if (kind === 'wiki-graph:start') {
        let visible = 0;
        for (const row of node.querySelectorAll('tbody tr')) {
          const endpoints = Array.from(row.querySelectorAll('a'), link => paths.get(new URL(link.href).pathname));
          row.hidden = endpoints.length !== 2 || endpoints.some(endpoint => !endpoint || !selectedIds.has(endpoint.id));
          if (!row.hidden) visible++;
        }
        if (node.matches('table, .table-wrapper') && visible === 0) {
          const empty = document.createElement('p');
          empty.className = 'wiki-graph-empty';
          empty.textContent = labels.relations;
          node.before(empty);
          node.hidden = true;
        }
      }
    }
  }
  const sidebar = document.querySelector('.sidebar-scrollbox .chapter');
  if (sidebar) {
    let heading, group = [];
    function settleGroup() { if (heading) heading.hidden = !group.some(item => !item.hidden); }
    for (const item of sidebar.children) {
      if (item.matches('.spacer')) { settleGroup(); heading = undefined; group = []; item.hidden = true; continue; }
      if (item.matches('.part-title')) { settleGroup(); heading = item; group = []; continue; }
      const link = item.querySelector(':scope > .chapter-link-wrapper > a');
      if (link) { item.hidden = allowed(new URL(link.href).pathname) === false; group.push(item); }
      else if (!item.textContent.trim()) item.hidden = true;
    }
    settleGroup();
  }
  if (locale) {
    const sequence = context.chapterOrder(graph, locale, selection);
    const position = sequence.indexOf(currentPath);
    for (const [direction, offset] of [['previous', -1], ['next', 1]]) {
      const destination = position < 0 ? undefined : sequence[position + offset];
      for (const link of document.querySelectorAll(`.nav-chapters.${direction}, .mobile-nav-chapters.${direction}`)) {
        if (!destination) link.remove();
        else {
          link.href = withLayer(new URL(destination, location.href).href, selection);
          link.title = direction === 'next' ? labels.next : labels.previous;
          link.setAttribute('aria-label', link.title);
        }
      }
    }
    if (selection !== 'all') for (const link of document.querySelectorAll('#mdbook-menu-bar a[href]')) if (new URL(link.href).pathname === `/${locale}/print.html`) link.remove();
  }
  for (const link of document.querySelectorAll('a[href]')) adaptLink(link);
  // Native search creates result links later and changes its own query URL without an event.
  for (const type of ['pointerdown', 'focusin', 'click', 'auxclick', 'contextmenu']) document.addEventListener(type, event => {
    const link = event.target.closest?.('a[href]');
    if (link) adaptLink(link);
  }, true);
  document.documentElement.dataset.wikiLayer = 'ready';
})();
