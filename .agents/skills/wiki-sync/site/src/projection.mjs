/*
Domain: Wiki graph-to-book projection
Owns: One composed graph, metadata-ordered SUMMARY and plain Markdown inputs for mdBook.
Excludes: Markdown/HTML rendering, filesystem publication, hosting and runtime/wallet access.
*/
import { pageRoute } from './corpus.mjs';
import { formatRussianRelation } from './relations.mjs';
import { assignMemberships } from './layers.mjs';
import { contextScript } from './context.mjs';

const ui = {
  en: { graph: 'Knowledge graph', relation: 'Relation', source: 'Source', target: 'Target', status: 'Status', sources: 'Recorded sources', empty: 'No pages are published in this plane.', languages: 'Languages', planes: 'Knowledge planes', browse: 'Browse the graph or choose a published page.', pages: 'Published pages', fallback: 'Layer filters require JavaScript. Without it, reading is unfiltered.', unknown: 'Page not found. Use the navigation to choose a published page.' },
  ru: { graph: 'Граф знаний', relation: 'Связь', source: 'Источник', target: 'Цель', status: 'Статус', sources: 'Указано источников', empty: 'В этой плоскости пока нет опубликованных страниц.', languages: 'Языки', planes: 'Плоскости знаний', browse: 'Откройте граф знаний или выберите опубликованную страницу.', pages: 'Опубликованные страницы', fallback: 'Для фильтрации по слоям нужен JavaScript. Без него доступны все материалы.', unknown: 'Страница не найдена. Выберите опубликованную страницу в навигации.' },
};

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

export function unifiedGraph(corpus) {
  const nodes = [], edges = [], linear = [];
  for (const plane of corpus.planes) {
    for (const id of linearOrder(plane, plane.defaultLocale)) {
      const mirrors = [...plane.pages.values()].filter(page => page.id === id);
      nodes.push({ id: qualified(plane, id), plane: plane.id, canonical_page_id: id,
        title: Object.fromEntries(mirrors.map(page => [page.locale, page.meta.title])),
        url: Object.fromEntries(mirrors.map(page => [page.locale, pageRoute(page.locale, plane.id, page.sourcePath)])),
        provenance: Object.fromEntries(mirrors.map(page => [page.locale, { source_page: `${plane.id}/${page.sourcePath}`, sources: page.meta.sources, status: page.meta.status ?? 'stable' }])),
      });
      linear.push(qualified(plane, id));
    }
    edges.push(...plane.graph.edges.map(edge => ({ from: qualified(plane, edge.from), to: qualified(plane, edge.to), type: edge.type })));
  }
  const layers = corpus.planes.map(({ id, title }) => ({ id, title }));
  return { schema_version: 1, layers, locales: corpus.locales, default_locale: corpus.defaultLocale, nodes: assignMemberships(nodes, layers, corpus.memberships), edges, linear_order: linear };
}

function graphTable(graph, locale, edges) {
  const labels = ui[locale];
  const byId = new Map(graph.nodes.map(node => [node.id, node]));
  const rows = edges.map(edge => {
    const source = byId.get(edge.from), target = byId.get(edge.to);
    if (!source?.url[locale] || !target?.url[locale]) throw new Error(`Missing localized graph endpoint: ${edge.from} -> ${edge.to}`);
    const relation = locale === 'ru' ? formatRussianRelation(edge.type, source.title[locale], target.title[locale]).label : edge.type.replaceAll('-', ' ');
    return `| ${link(source.title[locale], source.url[locale])} | ${text(relation)} | ${link(target.title[locale], target.url[locale])} |`;
  });
  return '<!-- wiki-graph:start -->\n\n' + [`| ${labels.source} | ${labels.relation} | ${labels.target} |`, '| --- | --- | --- |', ...rows].join('\n') + '\n\n<!-- wiki-graph:end -->';
}

function bookConfig(locale, title, search) {
  return `[book]\ntitle = ${JSON.stringify(title)}\nlanguage = ${JSON.stringify(locale || 'en')}\nsrc = "src"\n\n[build]\ncreate-missing = false\nuse-default-preprocessors = false\n\n[output.html]\nsite-url = ${JSON.stringify(locale ? `/${locale}/` : '/')}\nno-section-label = true\ndefault-theme = "light"\nadditional-css = ["theme/wiki.css"]\nadditional-js = ${JSON.stringify([...(locale ? [] : ['theme/root-context.js']), 'theme/wiki.js', 'theme/layers.js'])}\n\n[output.html.search]\nenable = ${search}\nuse-boolean-and = true\n\n[output.html.playground]\nrunnable = false\n`;
}

export function projectBooks(corpus, assets) {
  const graph = unifiedGraph(corpus);
  const baseTitle = corpus.planes.find(plane => plane.id === corpus.defaultPlane).title;
  const files = new Map([['projection.json', JSON.stringify(graph, null, 2) + '\n']]);
  function addBook(key, locale, title, search, markdown, summary) {
    files.set(`books/${key}/book.toml`, bookConfig(locale, title, search));
    files.set(`books/${key}/src/SUMMARY.md`, summary);
    for (const [path, content] of markdown) files.set(`books/${key}/src/${path}`, content);
    for (const [path, content] of Object.entries(assets)) files.set(`books/${key}/theme/${path}`, content);
    if (!locale) files.set(`books/${key}/theme/root-context.js`, contextScript(graph, null, null));
    const fallback = locale ? ui[locale].fallback : ui.en.fallback + ' / ' + ui.ru.fallback;
    const language = locale ? ui[locale].languages : 'Language / Язык';
    files.set(`books/${key}/theme/header.hbs`, `<noscript><div class="wiki-context wiki-basic-notice"><p>${fallback}</p><a href="/">${language}</a></div></noscript>\n`);
  }
  const rootLinks = corpus.locales.map(locale => `- ${link(locale === 'ru' ? 'Русский' : 'English', `${locale}/index.html`)}`).join('\n');
  addBook('root', '', `${baseTitle} Wiki`, false, [['index.md', `# Wiki / Вики\n\n${rootLinks}\n`]], '# Summary\n\n[Wiki](index.md)\n');
  for (const locale of corpus.locales) {
    const labels = ui[locale];
    const wikiTitle = locale === 'ru' ? 'Вики' : 'Wiki';
    const summary = ['# Summary', '', link(wikiTitle, 'index.md'), ''];
    const markdown = [['index.md', `# ${wikiTitle}\n\n${labels.browse}\n\n${link(labels.graph, `/${locale}/graph.html`)}\n\n## ${labels.pages}\n\n<!-- wiki-pages:start -->\n\n${graph.nodes.map(node => `- ${link(node.title[locale], node.url[locale])}`).join('\n')}\n\n<!-- wiki-pages:end -->\n`], ['404.md', `# 404\n\n${labels.unknown}\n`]];
    for (const plane of corpus.planes) {
      summary.push(`# ${text(plane.title)}`, '');
      if (plane.pages.size === 0) {
        summary.push(`- ${link(plane.title, `${plane.id}/index.md`)}`, '');
        markdown.push([`${plane.id}/index.md`, `# ${text(plane.title)}\n\n${labels.empty}\n`]);
        continue;
      }
      if (!plane.locales.includes(locale)) throw new Error(`${plane.id}: locale ${locale} has no complete projection`);
      for (const id of linearOrder(plane, plane.defaultLocale)) {
        const page = plane.pages.get(`${locale}/${id}`);
        summary.push(`- ${link(page.meta.title, `${plane.id}/${page.sourcePath}`)}`);
        const incident = graph.edges.filter(edge => edge.from === qualified(plane, id) || edge.to === qualified(plane, id));
        const status = locale === 'ru' ? { stable: 'Стабильно', draft: 'Черновик', deprecated: 'Устарело' }[page.meta.status ?? 'stable'] : page.meta.status ?? 'stable';
        markdown.push([`${plane.id}/${page.sourcePath}`, `${page.body.trimEnd()}\n\n---\n\n${labels.status}: ${status}. ${labels.sources}: ${page.meta.sources.length}.\n\n## ${labels.graph}\n\n${graphTable(graph, locale, incident)}\n\n${link(labels.graph, `/${locale}/graph.html`)}\n`]);
      }
      summary.push('');
    }
    summary.push('---', '', link(labels.graph, 'graph.md'), '');
    markdown.push(['graph.md', `# ${labels.graph}\n\n${graphTable(graph, locale, graph.edges)}\n`]);
    addBook(locale, locale, locale === 'ru' ? `Вики ${baseTitle} — Русский` : `${baseTitle} Wiki — English`, true, markdown, summary.join('\n'));
  }
  return { graph, files, locales: corpus.locales };
}
