/*
Domain: Static browser fixture serving
Owns: Bounded local file responses and explicit test-only data/failure overlays.
Excludes: Website compilation, replacement documents, public hosting and process persistence.
*/
import { createServer } from 'node:http';
import { readFileSync, statSync } from 'node:fs';
import { extname, join, resolve, sep } from 'node:path';

const types = { '.html': 'text/html; charset=utf-8', '.css': 'text/css', '.js': 'application/javascript', '.json': 'application/json', '.svg': 'image/svg+xml', '.png': 'image/png', '.woff2': 'font/woff2' };

// `prefix` mirrors a static host that serves the site beneath a path such as a GitHub Pages
// project site: requests outside it are 404 and the prefix maps to the artifact root.
export function serveStatic(root, requests, overlays = new Map(), prefix = '/') {
  root = resolve(root);
  return createServer((request, response) => {
    try {
      const pathname = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
      if (!pathname.startsWith(prefix)) throw new Error('Outside the served prefix');
      let path = resolve(root, './' + pathname.slice(prefix.length));
      if (path !== root && !path.startsWith(root + sep)) throw new Error('Outside static root');
      let bytes;
      if (overlays.has(pathname)) {
        if (overlays.get(pathname) === null) throw new Error('Explicit unavailable fixture');
        bytes = overlays.get(pathname);
      } else {
        if (statSync(path).isDirectory()) path = join(path, 'index.html');
        bytes = readFileSync(path);
      }
      requests.push({ path: pathname, status: 200 });
      response.writeHead(200, { 'Content-Type': types[extname(path)] ?? 'application/octet-stream', 'Cache-Control': 'no-store' });
      response.end(bytes);
    } catch {
      requests.push({ path: request.url, status: 404 });
      response.writeHead(404, { 'Content-Type': 'text/html; charset=utf-8' });
      response.end(readFileSync(join(root, '404.html')));
    }
  });
}
