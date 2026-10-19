/*
Domain: Bounded Chrome test fixture
Owns: One isolated Chrome process group, CDP pipe requests and deterministic teardown.
Excludes: Site assertions, HTTP serving, browser security bypasses and persistent processes.
*/
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

export async function withChrome(size, signal, callback) {
  const profile = mkdtempSync(join(tmpdir(), 'deos-wiki-chrome-'));
  const chrome = spawn(process.env.CHROME_BIN ?? 'google-chrome', [
    '--headless=new', '--no-first-run', '--no-default-browser-check',
    '--disable-background-networking', '--no-startup-window', '--remote-debugging-pipe',
    `--user-data-dir=${profile}`,
  ], { detached: true, stdio: ['ignore', 'ignore', 'pipe', 'pipe', 'pipe'] });
  const pending = new Map(), errors = [];
  let nextId = 0, buffer = '', diagnostics = '';
  function rejectPending(error) {
    for (const entry of pending.values()) { clearTimeout(entry.timer); entry.reject(error); }
    pending.clear();
  }
  const exited = new Promise(resolve => chrome.once('close', (code, termination) => {
    rejectPending(new Error(`Chrome closed (${code ?? termination}): ${diagnostics}`));
    resolve();
  }));
  chrome.on('error', rejectPending);
  chrome.stdio[3].on('error', rejectPending);
  chrome.stdio[4].on('error', rejectPending);
  chrome.stderr.on('data', bytes => { diagnostics = (diagnostics + bytes).slice(-4096); });
  chrome.stdio[4].setEncoding('utf8');
  chrome.stdio[4].on('data', bytes => {
    buffer += bytes.toString();
    let boundary;
    while ((boundary = buffer.indexOf('\0')) !== -1) {
      const message = JSON.parse(buffer.slice(0, boundary));
      buffer = buffer.slice(boundary + 1);
      if (message.id) {
        const entry = pending.get(message.id);
        if (!entry) continue;
        pending.delete(message.id);
        clearTimeout(entry.timer);
        if (message.error) entry.reject(new Error(`${entry.method}: ${JSON.stringify(message.error)}`));
        else entry.resolve(message.result);
      } else if (message.method === 'Runtime.exceptionThrown' || message.method === 'Network.loadingFailed' && !message.params.canceled) errors.push(message);
    }
  });
  function send(method, params = {}, sessionId) {
    const id = ++nextId;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Chrome request timed out: ${method}`)); }, 10000);
      pending.set(id, { resolve, reject, timer, method });
      chrome.stdio[3].write(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }) + '\0', error => {
        if (error && pending.delete(id)) { clearTimeout(timer); reject(error); }
      });
    });
  }
  const killGroup = signal => {
    if (!chrome.pid) return;
    try { process.kill(-chrome.pid, signal); } catch (error) { if (error.code !== 'ESRCH') throw error; }
  };
  const abort = () => killGroup('SIGTERM');
  signal?.addEventListener('abort', abort, { once: true });
  try {
    if (signal?.aborted) throw new Error('Browser test aborted before launch');
    await send('Browser.getVersion');
    const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
    const command = (method, params) => send(method, params, sessionId);
    await command('Page.enable');
    await command('Runtime.enable');
    await command('Network.enable');
    await command('Emulation.setDeviceMetricsOverride', { width: size[0], height: size[1], deviceScaleFactor: 1, mobile: false });
    async function evaluate(expression) {
      const reply = await command('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true, userGesture: true });
      if (reply.exceptionDetails) throw new Error(JSON.stringify(reply.exceptionDetails));
      return reply.result.value;
    }
    async function waitFor(expression) {
      const deadline = Date.now() + 8000;
      do {
        if (await evaluate(expression)) return;
        await delay(25, undefined, { signal });
      } while (Date.now() < deadline);
      const state = await evaluate('({url: location.href, title: document.title, search: document.querySelector("#mdbook-searchresults-header")?.textContent})');
      throw new Error(`Browser condition timed out: ${expression}\n${JSON.stringify(state)}`);
    }
    async function navigate(url, reload = false) {
      const previous = await evaluate('performance.timeOrigin');
      const result = await command(reload ? 'Page.reload' : 'Page.navigate', reload ? {} : { url });
      if (result.errorText) throw new Error(result.errorText);
      await waitFor(`performance.timeOrigin !== ${previous} && location.href === ${JSON.stringify(url)} && document.readyState === 'complete' && !!document.querySelector('.wiki-open-current')`);
      await evaluate('document.fonts.ready.then(() => true)');
    }
    await callback({ command, evaluate, waitFor, navigate, errors,
      async screenshot(path) {
        const { data } = await command('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
        writeFileSync(path, Buffer.from(data, 'base64'));
      },
    });
  } finally {
    signal?.removeEventListener('abort', abort);
    await send('Browser.close').catch(() => {});
    let closed = await Promise.race([exited.then(() => true), delay(3000, undefined, { ref: false }).then(() => false)]);
    if (!closed) {
      killGroup('SIGTERM');
      closed = await Promise.race([exited.then(() => true), delay(1000, undefined, { ref: false }).then(() => false)]);
      if (!closed) { killGroup('SIGKILL'); await exited; }
    }
    rmSync(profile, { recursive: true, force: true });
  }
}
