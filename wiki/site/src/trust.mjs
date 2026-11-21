/*
Domain: Reviewed Wiki Markdown trust
Owns: The reviewed Markdown trust boundary and the inert SVG diagram subset.
Excludes: Generic untrusted-Markdown sanitization, rendering and verification of source claims.
*/
export function assertTrustedMarkdown(text, path = '<input>') {
  for (const [pattern, reason] of [
    [/^\s*<([A-Za-z][A-Za-z0-9-]*)(\s|>|\/)/m, 'raw HTML tag blocks'],
    [/<(script|iframe|object|embed|link|style)\b/, 'executable or embedded HTML'],
    [/javascript:|vbscript:|data:text\/html/i, 'executable URL schemes'],
    [/\bon[a-z]+\s*=/i, 'inline DOM event handlers'],
  ]) if (pattern.test(text)) throw new Error(`${path}: ${reason} are not allowed in reviewed Wiki Markdown`);
}

// A closed geometry subset, not an SVG sanitizer. Published diagrams are openable
// as same-origin documents as well as images, so no active or external content is copied.
export function assertTrustedSvg(text, path = '<input>') {
  const [root, ...lines] = text.trimEnd().split('\n');
  const end = lines.pop();
  const shape = /^  (?:<path d="[MLHVZmlhvz0-9., -]+" fill="(?:none|#[0-9a-f]{6})"(?: stroke="#[0-9a-f]{6}" stroke-width="[1-9][0-9]*")?\/>|<circle cx="[0-9]+" cy="[0-9]+" r="[1-9][0-9]*" fill="#[0-9a-f]{6}"\/>|<text x="[0-9]+" y="[0-9]+" fill="#[0-9a-f]{6}" font-family="sans-serif" font-size="[1-9][0-9]*" text-anchor="middle">[0-9]+<\/text>)$/;
  if (!/^<svg xmlns="http:\/\/www\.w3\.org\/2000\/svg" viewBox="0 0 [1-9][0-9]* [1-9][0-9]*">$/.test(root) ||
    end !== '</svg>' || !lines.length || lines.some(line => !shape.test(line))) {
    throw new Error(`${path}: expected an inert local SVG diagram`);
  }
}
