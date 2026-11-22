/*
Domain: Wiki math rendering regressions
Owns: Delimiter handling, literal-text protection, failure on malformed notation and the inertness of the produced markup.
Excludes: What a formula claims, typographic review and browser layout.
*/
import assert from 'node:assert/strict';
import test from 'node:test';
import { renderMath, assertTypesetMarkup, typesetDisplay, typesetInline } from '../src/math.mjs';
import { katexStylesheet, katexFonts } from '../src/math-assets.mjs';

test('inline and display notation are typeset, and both are counted', () => {
  const result = renderMath('A curve with $x^2 + y^2$ and a block:\n\n$$F = \\frac{m}{1 - \\lambda}$$\n');
  assert.equal(result.inline, 1);
  assert.equal(result.display, 1);
  assert.equal(result.hasMath, true);
  assert.match(result.markdown, /class="katex"/);
  assert.match(result.markdown, /class="katex-display"/);
  assert.match(result.markdown, /<math/, 'MathML travels with the visual markup for assistive readers');
});

test('prose without notation is returned unchanged and needs no stylesheet', () => {
  const source = 'Plain words, an asterisk, a price of 5 and 6, and 100% coverage.\n';
  const result = renderMath(source);
  assert.equal(result.hasMath, false);
  assert.equal(result.markdown, source);
});

test('code, fenced blocks and escaped dollars stay literal', () => {
  const fence = '```\n$$not math$$\n```';
  const code = '`$not math$`';
  const result = renderMath(`Cost \\$5 and \\$6 here.\n\n${code}\n\n${fence}\n\nReal $a+b$.\n`);
  assert.ok(result.markdown.includes('\\$5 and \\$6'), 'an escaped dollar never opens a formula');
  assert.ok(result.markdown.includes(code), 'inline code is not typeset');
  assert.ok(result.markdown.includes('$$not math$$'), 'a fenced block is not typeset');
  assert.equal(result.inline, 1);
  assert.equal(result.display, 0);
});

test('an unterminated or line-crossing inline delimiter stays text', () => {
  const result = renderMath('Half a formula $x + y and then text.\n\nLine $a\nb$ break.\n');
  assert.equal(result.hasMath, false);
  assert.match(result.markdown, /Half a formula \$x \+ y/);
});

test('malformed notation fails the build instead of shipping red text', () => {
  assert.throws(() => renderMath('$$\\frac{1}{$$'), /KaTeX|Parse error|Expected/);
});

test('typeset markup is inert and paints no background of its own', () => {
  const inline = typesetInline('a^2');
  assert.match(inline, /class="katex"/);
  assert.doesNotMatch(inline, /background/i);
  assert.match(typesetDisplay('\\sum_{i=0}^{n} i'), /katex-display/);
  assert.throws(() => assertTypesetMarkup('<span style="background:#fff">x</span>'), /must not paint a background/);
  assert.throws(() => assertTypesetMarkup('<span onclick="x()">x</span>'), /must stay inert/);
  assert.throws(() => assertTypesetMarkup('<img src="https://example.invalid/x.png">'), /must stay inert/);
});

test('the vendored stylesheet names only available WOFF2 faces', () => {
  const [path, css] = katexStylesheet;
  assert.equal(path, 'theme/katex/katex.min.css');
  const faces = [...css.matchAll(/url\(fonts\/([^)]+)\)/g)].map(match => match[1]);
  assert.ok(faces.length >= 20, 'the stylesheet names the KaTeX faces');
  const published = new Set(katexFonts.map(([name]) => name.replace('theme/katex/fonts/', '')));
  for (const face of faces) assert.ok(published.has(face), `${face}: vendored font is missing`);
  for (const face of faces) assert.ok(face.endsWith('.woff2'), `${face}: only woff2 is vendored`);
  assert.doesNotMatch(css, /background|url\(fonts\/[^)]+\.(woff|ttf)\)/, 'no removed format and no painted background');
});
