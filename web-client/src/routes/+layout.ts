/*
Domain: Application rendering mode
Owns: SvelteKit static prerender and client-only rendering flags for the web client.
Excludes: Route UI, domain stores, adapter lifecycle, and deployment hosting config.
Zone: SvelteKit route configuration.
*/
export const prerender = true;
export const ssr = false;
