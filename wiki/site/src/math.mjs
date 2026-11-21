/*
Domain: Wiki mathematical notation
Owns: Rendering authored $...$ and $$...$$ notation to static markup before the renderer runs.
Excludes: What a formula claims, subtle typography review, browser-side typesetting and network access.
*/
import katex from '../theme/katex/katex.mjs';

// A build, not a viewer, decides. A malformed formula fails the build instead of shipping red text to
// a reader, so the artifact never contains maths the project has not seen typeset.
const inline = { displayMode: false, throwOnError: true, output: 'htmlAndMathml', strict: false };
const display = { displayMode: true, throwOnError: true, output: 'htmlAndMathml', strict: false };

const escapeHtml = value => String(value).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');

// Fenced blocks, inline code and escaped dollars are literal text, so they are copied through
// untouched. Everything else is scanned for the two delimiters the corpus documents.
const literal = /(^|\n)(```[\s\S]*?\n```|~~~[\s\S]*?\n~~~)|(`+)([\s\S]*?)\3|\\\$/g;

function splitLiterals(markdown) {
  const parts = [];
  let last = 0;
  for (const match of markdown.matchAll(literal)) {
    if (match.index > last) parts.push({ math: true, text: markdown.slice(last, match.index) });
    parts.push({ math: false, text: match[0] });
    last = match.index + match[0].length;
  }
  if (last < markdown.length) parts.push({ math: true, text: markdown.slice(last) });
  return parts;
}

// `$$...$$` may run across lines; `$...$` stays on one line, otherwise ordinary prose containing two
// dollar signs would swallow a paragraph.
function typeset(text, stats) {
  const out = [];
  let index = 0;
  while (index < text.length) {
    const open = text.indexOf('$$', index);
    const single = text.indexOf('$', index);
    if (open === -1 && single === -1) break;
    const displayMode = open !== -1 && open === single;
    const start = displayMode ? open : single;
    if (start > index) out.push(text.slice(index, start));
    const close = displayMode ? text.indexOf('$$', start + 2) : nextInlineClose(text, start + 1);
    if (close === -1) {
      out.push(text.slice(start));
      index = text.length;
      break;
    }
    const source = text.slice(start + (displayMode ? 2 : 1), close);
    if (displayMode) stats.display++;
    else stats.inline++;
    // The guard runs on real output, not only in tests: a hostile or accidental attribute would
    // otherwise reach every reader of the page.
    out.push(assertTypesetMarkup(katex.renderToString(source.trim(), displayMode ? display : inline), displayMode ? 'display math' : 'inline math'));
    index = close + (displayMode ? 2 : 1);
  }
  out.push(text.slice(index));
  return out.join('');
}

function nextInlineClose(text, from) {
  for (let index = from; index < text.length; index++) {
    if (text[index] === '\\') { index++; continue; }
    if (text[index] === '\n') return -1;
    if (text[index] === '$') return index;
  }
  return -1;
}

// Returns the markdown with typeset markup in place of the notation, plus the counts that tell a book
// whether it needs the stylesheet at all.
export function renderMath(markdown) {
  const stats = { inline: 0, display: 0 };
  const parts = splitLiterals(String(markdown)).map(part => (part.math ? typeset(part.text, stats) : part.text));
  return { markdown: parts.join(''), ...stats, hasMath: stats.inline + stats.display > 0 };
}

// KaTeX markup must stay inert: no script, no event handler, no remote reference and no colour of its
// own, so the formula inherits the page text colour on both themes instead of painting a background.
export function assertTypesetMarkup(markup, where = 'math') {
  if (/<script|on[a-z]+\s*=|javascript:|<iframe|<img/i.test(markup)) throw new Error(`${where}: typeset markup must stay inert`);
  if (/background(-color)?\s*:/i.test(markup)) throw new Error(`${where}: typeset markup must not paint a background`);
  if (/\ssrc\s*=/.test(markup)) throw new Error(`${where}: typeset markup must not reference remote assets`);
  return markup;
}

// Small helper for documentation and tests that need one formula without a whole page.
export function typesetInline(source) {
  return assertTypesetMarkup(katex.renderToString(source, inline));
}

export function typesetDisplay(source) {
  return assertTypesetMarkup(katex.renderToString(source, display));
}

export const escapeFormulaText = escapeHtml;
