/*
Domain: Trusted wiki rendering
Owns: Repo-local wiki markdown loading, trusted HTML rendering, and internal wiki/repo href resolution.
Excludes: Wiki content validation, route layout, external CMS input, and generic markdown sanitization.
Zone: Wiki helper; assumes repository validation enforces the trusted-content boundary.
*/
import { marked } from 'marked';

type WikiPageImport = () => Promise<string>;

type ResolvedWikiHref =
  | { kind: 'external'; href: string }
  | { kind: 'wiki'; path: string }
  | { kind: 'repo'; path: string };

export type TrustedWikiPage = {
  markdown: string;
  html: string;
};

const wikiPageImporters = import.meta.glob<string>('../../../../wiki/**/*.md', {
  query: '?raw',
  import: 'default',
});

const wikiPageCache = new Map<string, Promise<TrustedWikiPage>>();
const wikiPageImportersByPath = new Map<string, WikiPageImport>(
  Object.entries(wikiPageImporters)
    .map(([path, load]) => {
      const relativePath = path.split('/wiki/')[1];
      return relativePath ? [relativePath, load] : null;
    })
    .filter((entry): entry is [string, WikiPageImport] => entry !== null),
);

marked.use({
  async: false,
  gfm: true,
});

function escapeHtmlAttribute(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('"', '&quot;')
    .replaceAll('<', '&lt;');
}

function resolveWikiHref(href: string, sourcePath: string): ResolvedWikiHref {
  if (
    href.startsWith('#') ||
    href.startsWith('http://') ||
    href.startsWith('https://') ||
    href.startsWith('mailto:')
  ) {
    return { kind: 'external', href };
  }
  const resolvedUrl = new URL(href, `https://repo.local/wiki/${sourcePath}`);
  const resolvedPath = decodeURIComponent(resolvedUrl.pathname);
  if (resolvedPath.startsWith('/wiki/') && resolvedPath.endsWith('.md')) {
    return { kind: 'wiki', path: resolvedPath.slice('/wiki/'.length) };
  }
  return { kind: 'repo', path: resolvedPath.slice(1) };
}

function renderTrustedWikiMarkdown(
  markdown: string,
  sourcePath: string,
): string {
  let content = markdown;
  if (content.startsWith('---')) {
    content = content.replace(/^---[\s\S]*?\n---\n/, '');
  }
  const renderer = new marked.Renderer();
  renderer.link = function ({ href, title, tokens }) {
    const text = this.parser.parseInline(tokens);
    if (!href) {
      return text;
    }
    const resolvedHref = resolveWikiHref(href, sourcePath);
    if (resolvedHref.kind === 'external') {
      let html = `<a href="${escapeHtmlAttribute(resolvedHref.href)}"`;
      if (title) {
        html += ` title="${escapeHtmlAttribute(title)}"`;
      }
      if (
        resolvedHref.href.startsWith('http://') ||
        resolvedHref.href.startsWith('https://')
      ) {
        html += ' target="_blank" rel="noreferrer"';
      }
      html += `>${text}</a>`;
      return html;
    }
    const datasetName =
      resolvedHref.kind === 'wiki' ? 'data-wiki-path' : 'data-repo-path';
    let html = `<a href="#" ${datasetName}="${escapeHtmlAttribute(resolvedHref.path)}"`;
    if (title) {
      html += ` title="${escapeHtmlAttribute(title)}"`;
    }
    html += `>${text}</a>`;
    return html;
  };
  return marked.parse(content, { async: false, renderer });
}

export async function loadTrustedWikiPage(
  path: string,
): Promise<TrustedWikiPage> {
  const cachedPage = wikiPageCache.get(path);
  if (cachedPage) {
    return cachedPage;
  }
  const importer = wikiPageImportersByPath.get(path);
  if (!importer) {
    throw new Error(`Wiki page not found: ${path}`);
  }
  const pagePromise = importer().then((markdown) => ({
    markdown,
    html: renderTrustedWikiMarkdown(markdown, path),
  }));
  wikiPageCache.set(path, pagePromise);
  return pagePromise;
}
