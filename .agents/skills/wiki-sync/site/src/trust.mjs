/*
Domain: Reviewed Wiki Markdown trust
Owns: This Skill's no-HTML/no-executable-URL contract shared by compilation and audits.
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
