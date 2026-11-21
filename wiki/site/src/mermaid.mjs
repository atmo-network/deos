/*
Domain: Wiki diagrams written as Mermaid
Owns: Turning authored ```mermaid blocks into inert placeholders, and the loader that renders them in a reader's browser.
Excludes: Mermaid itself, diagram meaning, custom infographics authored as SVG, and network access at build time.
*/

// The corpus declares which diagram syntaxes it uses. Vendoring one distribution must not silently
// promise the others, so an undeclared first keyword fails the build instead of the reader's browser.
const supported = new Set(['flowchart', 'graph']);

const escapeHtml = value => String(value).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');

const fence = /^(```|~~~)mermaid[ \t]*\r?\n([\s\S]*?)^\1[ \t]*$/gm;

function firstWord(source) {
  const line = source.trim().split('\n').find(candidate => candidate.trim() && !candidate.trim().startsWith('%%'));
  return line ? line.trim().split(/[\s{]/)[0] : '';
}

// A block becomes a hidden source element plus a visible target. Nothing is executed at build time,
// so the page keeps working without JavaScript: the source stays readable text.
export function renderDiagrams(markdown, where = 'mermaid') {
  let count = 0;
  const out = String(markdown).replace(fence, (match, ticks, source) => {
    const kind = firstWord(source);
    if (!supported.has(kind)) throw new Error(`${where}: diagram syntax "${kind || 'unknown'}" is not vendored; use ${[...supported].join(' or ')} or an authored SVG infographic`);
    // A block must declare a syntax and at least one statement; a lone "flowchart LR" draws nothing
    // and would publish an empty frame.
    const statements = source.trim().split('\n').map(line => line.trim()).filter(line => line && !line.startsWith('%%'));
    if (statements.length < 2) throw new Error(`${where}: a Mermaid diagram needs a syntax and at least one statement`);
    count++;
    return `<figure class="wiki-diagram">\n<div class="wiki-mermaid" data-diagram="${count}">${escapeHtml(source.trimEnd())}</div>\n</figure>\n`;
  });
  return { markdown: out, hasDiagram: count > 0, count };
}

export const mermaidLoaderName = 'wiki-mermaid.js';
