/*
Domain: Wiki graph-to-book projection
Owns: One composed graph (kept behind the reader UI), metadata-ordered SUMMARY and plain Markdown inputs for mdBook.
Excludes: Markdown/HTML rendering, filesystem publication, hosting and runtime/wallet access.
*/
import { posix } from 'node:path';
import { basePath, bookSource, localizeLinks, pageRoute } from './corpus.mjs';
import { assignMemberships } from './layers.mjs';
import { contextScript } from './context.mjs';

const ui = {
  en: { site: title => `${title} Wiki`, contents: 'Contents', intro: 'Every published page, grouped by section. Press S to search.', status: 'Status', sources: 'Recorded sources', empty: 'No pages are published in this plane.', fallback: 'Layer filters require JavaScript. Without it, reading is unfiltered.', unknown: 'Page not found.', home: 'Open the wiki', layer: 'Knowledge layer' },
  ru: { site: title => `Вики ${title}`, contents: 'Содержание', intro: 'Все опубликованные страницы, сгруппированные по разделам. Нажмите S для поиска.', status: 'Статус', sources: 'Указано источников', empty: 'В этой плоскости пока нет опубликованных страниц.', fallback: 'Для фильтрации по слоям нужен JavaScript. Без него доступны все материалы.', unknown: 'Страница не найдена.', home: 'Открыть вики', layer: 'Слой знаний' },
};
const languageNames = { en: 'English', ru: 'Русский' };

function text(value) {
  return String(value).replace(/\\/g, '\\\\').replace(/[\[\]`*_{}|]/g, '\\$&').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replace(/[\r\n]+/g, ' ');
}

function qualified(plane, id) { return `${plane.id}/${id}`; }
function link(label, href) { return `[${text(label)}](${href})`; }
export function linearOrder(plane, defaultLocale) {
  if (plane.pages.size === 0) return [];
  const order = ['index'];
  const seen = new Set(order);
  for (const section of plane.navigation.sections) for (const item of section.items) {
    if (!seen.has(item.id)) { seen.add(item.id); order.push(item.id); }
  }
  const declared = [...plane.pages.values()].filter(page => page.locale === defaultLocale).map(page => page.id);
  if (order.some(id => !declared.includes(id)) || declared.some(id => !seen.has(id))) throw new Error(`${plane.id}: every graph page needs an explicit navigation position`);
  return order;
}

export function unifiedGraph(corpus, base = '/') {
  const base_path = basePath(base);
  const nodes = [], edges = [], linear = [];
  for (const plane of corpus.planes) {
    for (const id of linearOrder(plane, plane.defaultLocale)) {
      const mirrors = [...plane.pages.values()].filter(page => page.id === id);
      nodes.push({ id: qualified(plane, id), plane: plane.id, canonical_page_id: id,
        title: Object.fromEntries(mirrors.map(page => [page.locale, page.meta.title])),
        description: Object.fromEntries(mirrors.map(page => [page.locale, page.meta.description])),
        url: Object.fromEntries(mirrors.map(page => [page.locale, pageRoute(page.locale, plane.id, page.sourcePath, base_path)])),
        provenance: Object.fromEntries(mirrors.map(page => [page.locale, { source_page: `${plane.id}/${page.sourcePath}`, sources: page.meta.sources, status: page.meta.status ?? 'stable' }])),
      });
      linear.push(qualified(plane, id));
    }
    edges.push(...plane.graph.edges.map(edge => ({ from: qualified(plane, edge.from), to: qualified(plane, edge.to), type: edge.type })));
  }
  // Dropping locale suffixes must never merge two sources into one public route.
  const routes = nodes.flatMap(node => Object.values(node.url));
  const duplicate = routes.find((route, index) => routes.indexOf(route) !== index);
  if (duplicate) throw new Error(`Two pages share the route ${duplicate}`);
  const layers = corpus.planes.map(({ id, title }) => ({ id, title }));
  return { schema_version: 1, base_path, layers, locales: corpus.locales, default_locale: corpus.defaultLocale, nodes: assignMemberships(nodes, layers, corpus.memberships), edges, linear_order: linear };
}

// Book-relative path of a composed route (`{base}{locale}/...`).
function bookPath(route, base, locale) {
  const prefix = `${base}${locale}/`;
  if (!route.startsWith(prefix)) throw new Error(`Route ${route} is outside locale ${locale}`);
  return route.slice(prefix.length);
}

// Relative href from one book source file to a book-relative output path. Relative links keep the
// artifact servable from any URL prefix; only the native 404 page relies on the configured base.
function relativeHref(from, to) {
  return posix.relative(posix.dirname(from), to) || posix.basename(to);
}

function bookConfig(locale, title, search, base) {
  return `[book]\ntitle = ${JSON.stringify(title)}\nlanguage = ${JSON.stringify(locale || 'en')}\nsrc = "src"\n\n[build]\ncreate-missing = false\nuse-default-preprocessors = false\n\n[output.html]\nsite-url = ${JSON.stringify(locale ? `${base}${locale}/` : base)}\nno-section-label = true\nsidebar-header-nav = false\ndefault-theme = "light"\npreferred-dark-theme = "navy"\nadditional-css = ["theme/wiki.css"]\nadditional-js = ${JSON.stringify([...(locale ? [] : ['theme/root-context.js']), 'theme/wiki.js', 'theme/layers.js'])}\n\n[output.html.fold]\nenable = true\nlevel = 0\n\n[output.html.print]\nenable = false\n\n[output.html.search]\nenable = ${search}\nuse-boolean-and = true\n\n[output.html.playground]\nrunnable = false\n`;
}

// Runs first in <head>: records the rendered chapter and repairs a directory index that a
// clean-URL server delivered without its trailing slash, before any relative URL resolves.
const headPartial = `<script>
(() => {
  const source = "{{ path }}";
  window.wikiSource = source;
  const path = location.pathname;
  if (/(^|\\/)index\\.md$/.test(source) && !path.endsWith('/') && !path.endsWith('.html') && !path.endsWith('/index')) {
    const base = new URL(location.href);
    base.pathname = path + '/';
    base.search = '';
    base.hash = '';
    document.write('<base href="' + base.href + '">');
  }
})();
</script>
`;

// mdBook hides the sidebar below 1080px; open it from 768px unless the reader chose otherwise.
// Runs right after mdBook's own decision and before the page is painted.
const sidebarDefault = `<script>
(() => {
  const toggle = document.getElementById('mdbook-sidebar-toggle-anchor');
  let saved = null;
  try { saved = localStorage.getItem('mdbook-sidebar'); } catch {}
  if (toggle && saved === null && !toggle.checked && document.body.clientWidth >= 768) {
    toggle.checked = true;
    document.documentElement.classList.add('sidebar-visible');
  }
})();
</script>
`;

// The site root opens the default plane's landing (or a requested layer's contents) in the reader's
// last language, never a language chooser.
function siteIndex(title, locales, defaultLocale, landing) {
  const targets = JSON.stringify(Object.fromEntries(locales.map(locale => [locale, landing[locale]])));
  return `<!DOCTYPE html>
<html lang="${defaultLocale}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${title}</title>
<meta http-equiv="refresh" content="0; url=${landing[defaultLocale]}">
<script>
(() => {
  const targets = ${targets};
  let locale = ${JSON.stringify(defaultLocale)};
  try {
    const saved = localStorage.getItem('wiki-locale');
    if (Object.hasOwn(targets, saved)) locale = saved;
  } catch {}
  const here = new URL(location.href);
  if (!here.pathname.endsWith('/') && !here.pathname.endsWith('.html')) here.pathname += '/';
  // State arrives in the fragment, which a static host never sees; a layer link means "open this
  // layer", whose contents always belong to it, unlike the base landing.
  const fragment = here.hash.slice(1), stateful = fragment.startsWith('!');
  const state = new URLSearchParams(stateful ? fragment.slice(1) : '');
  const target = new URL(here.searchParams.has('layer') || state.has('layer') ? locale + '/index.html' : targets[locale], here);
  target.search = location.search;
  target.hash = location.hash;
  location.replace(target.href);
})();
</script>
</head>
<body>
<nav>
${locales.map(locale => `<p><a href="${landing[locale]}" hreflang="${locale}">${title} — ${languageNames[locale]}</a> · <a href="${locale}/index.html" hreflang="${locale}">${ui[locale].contents}</a></p>`).join('\n')}
</nav>
</body>
</html>
`;
}

function layerTags(node, locale, from) {
  if (!node.layers.length) return '';
  const contents = relativeHref(from, 'index.html');
  const label = ui[locale].layer;
  return `<p class="wiki-tags" aria-label="${label}">${node.layers.map(layer => `<a class="wiki-tag" href="${contents}#!layer=${encodeURIComponent(layer)}">#${layer}</a>`).join(' ')}</p>\n\n`;
}

export function projectBooks(corpus, assets, base = '/') {
  const graph = unifiedGraph(corpus, base);
  const root = graph.base_path;
  const multiplePlanes = corpus.planes.length > 1;
  const baseTitle = corpus.planes.find(plane => plane.id === corpus.defaultPlane).title;
  const byId = new Map(graph.nodes.map(node => [node.id, node]));
  const files = new Map([['projection.json', JSON.stringify(graph, null, 2) + '\n']]);
  const landingNode = byId.get(qualified({ id: corpus.defaultPlane }, 'index'));
  const landing = Object.fromEntries(corpus.locales.map(locale => [locale, landingNode?.url[locale] ? landingNode.url[locale].slice(root.length) : `${locale}/index.html`]));
  function addBook(key, locale, title, search, markdown, summary) {
    files.set(`books/${key}/book.toml`, bookConfig(locale, title, search, root));
    files.set(`books/${key}/src/SUMMARY.md`, summary);
    for (const [path, content] of markdown) files.set(`books/${key}/src/${path}`, content);
    for (const [path, content] of Object.entries(assets)) files.set(`books/${key}/theme/${path}`, content);
    if (!locale) files.set(`books/${key}/theme/root-context.js`, contextScript(graph, null, null));
    files.set(`books/${key}/theme/head.hbs`, headPartial);
    // Locale books live one directory below the site root; the root book is the site root.
    const up = locale ? '../' : '';
    const fallback = locale ? ui[locale].fallback : `${ui.en.fallback} / ${ui.ru.fallback}`;
    const languages = [
      ...corpus.locales.map(other => `<a href="{{ path_to_root }}${up}${landing[other]}" lang="${other}" hreflang="${other}">${languageNames[other]}</a>`),
      // The contents page lists every article as a static link for script-less readers and crawlers.
      ...(locale ? [`<a href="{{ path_to_root }}index.html">${ui[locale].contents}</a>`] : []),
    ].join(' ');
    files.set(`books/${key}/theme/header.hbs`, `${sidebarDefault}<noscript><div class="wiki-context wiki-basic-notice"><p>${fallback}</p><nav>${languages}</nav></div></noscript>\n`);
  }
  const rootTitle = ui[corpus.defaultLocale].site(baseTitle);
  const notFound = `# 404\n\n${ui.en.unknown} / ${ui.ru.unknown}\n\n${corpus.locales.map(locale => `- ${link(`${ui[locale].home} (${languageNames[locale]})`, landing[locale])}`).join('\n')}\n`;
  addBook('root', '', rootTitle, false, [['index.md', `# ${text(rootTitle)}\n`], ['404.md', notFound]], '# Summary\n\n[Wiki](index.md)\n');
  files.set('site-index.html', siteIndex(rootTitle, corpus.locales, corpus.defaultLocale, landing));
  for (const locale of corpus.locales) {
    const labels = ui[locale];
    const summary = ['# Summary', '', link(labels.contents, 'index.md'), ''];
    const contents = [`# ${labels.contents}`, '', labels.intro, '', '<!-- wiki-pages:start -->', ''];
    const markdown = [['404.md', `# 404\n\n${labels.unknown}\n\n${link(labels.home, relativeHref('404.md', bookPath(`${root}${landing[locale]}`, root, locale)))}\n`]];
    for (const plane of corpus.planes) {
      if (multiplePlanes) contents.push(`## ${text(plane.title)}`, '');
      if (plane.pages.size === 0) {
        summary.push(`- ${link(plane.title, `${plane.id}/index.md`)}`);
        markdown.push([`${plane.id}/index.md`, `# ${text(plane.title)}\n\n${labels.empty}\n`]);
        contents.push(`- ${link(plane.title, `${plane.id}/index.html`)}`, '');
        continue;
      }
      if (!plane.locales.includes(locale)) throw new Error(`${plane.id}: locale ${locale} has no complete projection`);
      const order = linearOrder(plane, plane.defaultLocale);
      const pageOf = id => plane.pages.get(`${locale}/${id}`);
      const pathOf = id => `${plane.id}/${bookSource(locale, pageOf(id).sourcePath)}`;
      const entry = (id, summaryText) => `- ${link(pageOf(id).meta.title, pathOf(id).replace(/\.md$/, '.html'))}${summaryText ? ` <span class="wiki-summary">— ${text(summaryText)}</span>` : ''}`;
      // Classic tree: the plane landing, then each navigation section as a foldable node of its pages.
      // Several planes nest one level deeper under a node per plane.
      const indent = multiplePlanes ? '  ' : '';
      if (multiplePlanes) summary.push(`- [${text(plane.title)}]()`);
      summary.push(`${indent}- ${link(pageOf('index').meta.title, pathOf('index'))}`);
      contents.push(entry('index', pageOf('index').meta.description), '');
      const placed = new Set(['index']);
      for (const section of plane.navigation.sections) {
        const ids = section.items.map(item => item.id).filter(id => !placed.has(id));
        if (!ids.length) continue;
        const sectionTitle = section.title?.[locale] ?? section.title?.[plane.defaultLocale] ?? section.id;
        summary.push(`${indent}- [${text(sectionTitle)}]()`);
        contents.push(`${multiplePlanes ? '###' : '##'} ${text(sectionTitle)}`, '');
        for (const id of ids) {
          placed.add(id);
          const item = section.items.find(candidate => candidate.id === id);
          summary.push(`${indent}  - ${link(pageOf(id).meta.title, pathOf(id))}`);
          contents.push(entry(id, item.summary?.[locale] ?? pageOf(id).meta.description));
        }
        contents.push('');
      }
      if (order.some(id => !placed.has(id))) throw new Error(`${plane.id}: navigation sections do not place every page`);
      for (const id of order) {
        const page = pageOf(id), path = pathOf(id), node = byId.get(qualified(plane, id));
        const status = locale === 'ru' ? { stable: 'Стабильно', draft: 'Черновик', deprecated: 'Устарело' }[page.meta.status ?? 'stable'] : page.meta.status ?? 'stable';
        markdown.push([path, `${layerTags(node, locale, path)}${localizeLinks(page.body, locale).trimEnd()}\n\n---\n\n<p class="wiki-meta">${labels.status}: ${status}. ${labels.sources}: ${page.meta.sources.length}.</p>\n`]);
      }
    }
    contents.push('<!-- wiki-pages:end -->', '');
    markdown.unshift(['index.md', contents.join('\n')]);
    addBook(locale, locale, labels.site(baseTitle), true, markdown, summary.join('\n'));
  }
  return { graph, files, locales: corpus.locales };
}
