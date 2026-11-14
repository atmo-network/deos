import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

// Structural/protocol leaf of network-assurance-local.sh, not a second orchestrator.
const [mode, executable, directory, input, label = 'checkpoint'] = process.argv.slice(2);
const save = (name, value) => fs.writeFileSync(path.join(directory, name), JSON.stringify(value, null, 2) + '\n', { flag: 'wx' });
// This finite profile does not clear general CLI usage or refresh advisory review.
// Relevant advisory changes require independent re-review before reuse; no offline
// preflight fetches an advisory database. Expiry is an upper bound, not renewal.
const diagnosticProfile = Object.freeze({
  tree: '4958357419d153a37f4b983d814b86b18d7ce443a04389b600dd7b3df983b5b1',
  node: '1bec56ef7cfa9a76f3e0b7c0a87f220eb73f23102b9c0b4c7529a3f7c3ce7c31',
  expires: '2026-12-12T00:00:00Z',
  binaries: {
    polkadot: '7c21c5831eb0c093e3b91e23857cfaecf30ba15b4d14256c13f942a43542c21b',
    'polkadot-prepare-worker': 'b33b8a288b386ff5d1bbd28eb6a3ee70e0ada968e93c369888ee0cf67303e771',
    'polkadot-execute-worker': '16ecbd306afa6562d911a8d533e976e194c6d5e3f404321ea83ec4171f7fe5f6',
  },
});
const reviewedTopology = {"settings":{"provider":"native","timeout":300,"bootnode":false,"node_verifier":"None","telemetry":false,"enable_tracing":false},"relaychain":{"default_command":"polkadot","chain":"rococo-local","default_substrate_cli_args_version":3,"default_args":["-:--insecure-validator-i-know-what-i-do","-:--prometheus-external","--no-prometheus","--listen-addr","/ip4/127.0.0.1/tcp/30333/ws","--workers-path","$WORKERS","--log","info,parachain::pvf=trace,parachain::candidate-validation=debug,parachain::candidate-backing=debug,parachain::collator-protocol=debug"],"genesis":{"runtimeGenesis":{"patch":{"configuration":{"config":{"scheduler_params":{"max_validators_per_core":2}}}}}},"nodes":[{"name":"alice","validator":true,"rpc_port":9944,"p2p_port":30334},{"name":"bob","validator":true,"rpc_port":9955,"p2p_port":30335}]},"parachains":[{"id":2000,"chain_spec_path":"$SPEC","onboard_as_parachain":true,"collators":[{"name":"alice","rpc_port":9988,"p2p_port":30336,"command":"polkadot-omni-node-deos","substrate_cli_args_version":3,"args":["--force-authoring","-:--prometheus-external","--no-prometheus","--no-mdns","--no-telemetry","--log","info,cumulus-consensus=debug,parachain::collator-protocol=debug","--","--no-mdns","--no-telemetry","--no-prometheus","--listen-addr=/ip4/127.0.0.1/tcp/0/ws","-:--port"]},{"name":"bob","rpc_port":9999,"p2p_port":30337,"command":"polkadot-omni-node-deos","substrate_cli_args_version":3,"args":["--force-authoring","-:--prometheus-external","--no-prometheus","--no-mdns","--no-telemetry","--log","info,cumulus-consensus=debug,parachain::collator-protocol=debug","--","--no-mdns","--no-telemetry","--no-prometheus","--listen-addr=/ip4/127.0.0.1/tcp/0/ws","-:--port"]}]}]};
const digest = value => createHash('sha256').update(value).digest('hex');
const fileDigest = file => digest(fs.readFileSync(file));
function treeDigest(root) {
  const hash = createHash('sha256');
  function walk(relative) {
    const full = path.join(root, relative), stat = fs.lstatSync(full);
    let record;
    if (stat.isSymbolicLink()) {
      assert(fs.realpathSync(full).startsWith(root + path.sep), 'External installed-tool symlink');
      record = [relative, 'link', fs.readlinkSync(full)];
    } else if (stat.isDirectory()) record = [relative, 'directory'];
    else {
      assert(stat.isFile(), 'Unsupported installed-tool entry');
      record = [relative, 'file', stat.mode & 0o111, fileDigest(full)];
    }
    hash.update(JSON.stringify(record) + '\n');
    if (stat.isDirectory()) for (const name of fs.readdirSync(full).sort()) walk(path.posix.join(relative, name));
  }
  walk('');
  return hash.digest('hex');
}
function injectionEnvironment(env) {
  for (const key of [...Object.keys(env).filter(key => /^(?:BASH_FUNC_|ZOMBIE_)/.test(key)), 'NODE_OPTIONS', 'NODE_PATH', 'NODE_EXTRA_CA_CERTS', 'NODE_REPL_EXTERNAL_MODULE', 'LD_PRELOAD', 'LD_LIBRARY_PATH', 'DYLD_INSERT_LIBRARIES', 'BASH_ENV', 'ENV', 'ZOMBIENET_IMAGE', 'RUN_IN_CONTAINER', 'PUSHGATEWAY_URL']) {
    assert(!env[key], `Diagnostic injection environment: ${key}`);
  }
}
function installedProfile(root, runtime, env = process.env, now = Date.now(), profile = diagnosticProfile, platform = process.platform, arch = process.arch) {
  // Only self-test supplies a different profile; no CLI/environment expected hash.
  injectionEnvironment(env);
  assert.equal(platform, 'linux', 'Diagnostic target must be Linux');
  assert.equal(arch, 'x64', 'Diagnostic target must be x86_64');
  assert(now < Date.parse(profile.expires), 'Diagnostic source review expired');
  assert.equal(fileDigest(runtime), profile.node, 'Diagnostic Node executable drift');
  assert.equal(treeDigest(root), profile.tree, 'Diagnostic installed source drift');
  const pkg = JSON.parse(fs.readFileSync(path.join(root, 'package.json')));
  assert.equal(pkg.name, '@zombienet/cli');
  assert.equal(pkg.version, '1.3.138');
}
function reviewedInput(file) {
  assert.equal(path.extname(file), '.json', 'Diagnostic input must have actual .json suffix');
  const bytes = fs.readFileSync(file);
  assert(!/\{\{|\{%|\{#/.test(bytes.toString('utf8')), 'Diagnostic template delimiters forbidden');
  const config = JSON.parse(bytes.toString('utf8'));
  diagnosticConfig(config);
  const normalized = structuredClone(config);
  const workerIndex = normalized.relaychain.default_args.indexOf('--workers-path');
  assert(workerIndex >= 0);
  const workers = normalized.relaychain.default_args[workerIndex + 1];
  const spec = normalized.parachains[0].chain_spec_path;
  for (const location of [workers, spec]) {
    assert(typeof location === 'string' && path.isAbsolute(location) && /^[a-z0-9_./-]+$/i.test(location), 'Plain absolute diagnostic input paths required');
  }
  normalized.relaychain.default_args[workerIndex + 1] = '$WORKERS';
  normalized.parachains[0].chain_spec_path = '$SPEC';
  assert.deepEqual(normalized, reviewedTopology, 'Unreviewed diagnostic topology/action/key/reporter/override');
  return { config, workers, spec, configSha256: digest(bytes) };
}
function diagnosticBinaries(workers, accepted = diagnosticProfile.binaries) {
  const binaries = {};
  for (const [name, expected] of Object.entries(accepted)) {
    const binary = path.join(workers, name);
    assert.equal(fileDigest(binary), expected, `Diagnostic ${name} drift`);
    binaries[name] = { path: fs.realpathSync(binary), sha256: expected };
  }
  return binaries;
}
function diagnosticIdentity(file) {
  const cli = fs.realpathSync(executable), root = path.dirname(path.dirname(cli));
  assert.equal(cli, path.join(root, 'dist/cli.js'), 'Exact CLI entry required');
  installedProfile(root, process.execPath);
  assert.equal(process.version, 'v22.22.0');
  const reviewed = reviewedInput(file);
  const binaries = diagnosticBinaries(reviewed.workers);
  // Keep the existing explicit candidate selection, not a fixed future build hash.
  // Reviewed JS derives peer-key bytes from fixed names; no child key output
  // enters that protobuf decode. Binary binding is a separate target obligation.
  const candidate = process.env.DEOS_DIAGNOSTIC_OMNI_PATH;
  const expected = process.env.DEOS_DIAGNOSTIC_OMNI_SHA256;
  assert(candidate && /^[a-f0-9]{64}$/.test(expected ?? ''), 'Existing explicit candidate SHA route required');
  assert.equal(fileDigest(candidate), expected, 'Diagnostic candidate drift');
  binaries.omni = { path: fs.realpathSync(candidate), sha256: expected, version: process.env.DEOS_DIAGNOSTIC_OMNI_VERSION };
  assert(binaries.omni.version, 'Explicit candidate version required');
  const specSha256 = fileDigest(reviewed.spec);
  if (process.env.CHAIN_SPEC_PATH) assert.equal(fileDigest(process.env.CHAIN_SPEC_PATH), specSha256, 'Selected raw spec differs');
  return { cli, root, runtime: fs.realpathSync(process.execPath), tree: diagnosticProfile.tree, node: diagnosticProfile.node, expires: diagnosticProfile.expires, config: fs.realpathSync(file), configSha256: reviewed.configSha256, spec: fs.realpathSync(reviewed.spec), specSha256, binaries };
}
function verifyFrozenDiagnostic(file = input, evidenceDir = directory, identify = diagnosticIdentity) {
  const frozen = JSON.parse(fs.readFileSync(path.join(evidenceDir, 'diagnostic-inputs.json')));
  assert.deepEqual(identify(file), frozen, 'Frozen diagnostic input drift');
  return frozen;
}
const requireTool = () => {
  if (process.env.DEOS_NETWORK_DIAGNOSTIC === '1') {
    if (mode === 'preflight') diagnosticIdentity(input);
    else {
      const frozen = JSON.parse(fs.readFileSync(path.join(directory, 'diagnostic-inputs.json')));
      verifyFrozenDiagnostic(frozen.config);
    }
  }
  return createRequire(fs.realpathSync(executable));
};
function topPairs(value) {
  assert(Array.isArray(value) && value.length > 0, 'Complete nonempty TOP required');
  assert(value.every(pair => Array.isArray(pair) && pair.length === 2 && pair.every(x => typeof x === 'string' && /^0x([a-f0-9]{2})*$/i.test(x))), 'Malformed TOP');
  const pairs = value.map(pair => pair.map(x => x.toLowerCase())).sort((a, b) => a[0].localeCompare(b[0]));
  assert(new Set(pairs.map(x => x[0])).size === pairs.length, 'Duplicate TOP key');
  assert(pairs.some(x => x[0] === '0x3a636f6465'), 'Missing runtime CODE');
  return pairs;
}
function configStrings(value) {
  if (typeof value === 'string') assert(!/[\u0000-\u001f\u007f-\u009f\u2028\u2029]/u.test(value), 'Control character in diagnostic input');
  else if (value && typeof value === 'object') Object.values(value).forEach(configStrings);
}
function safeCommand(command) {
  configStrings(command);
  assert(!/--(?:insecure-validator-i-know-what-i-do|disable-worker-version-check|unsafe-rpc-external|rpc-external|prometheus-external|unsafe-ws-external)\b/.test(command), 'Unsafe generated command');
  assert(!/0\.0\.0\.0|\/ip6\/|webrtc|https?:\/\/|wss?:\/\//i.test(command), 'Non-loopback/service command');
  assert(/^[a-z0-9_./,:= +@%-]+$/i.test(command), 'Only plain unquoted diagnostic command words are supported');
  assert(!/--(?:experimental-rpc-endpoint|public-addr|reserved-nodes|rpc-(?:addr|listen-addr|interface|host)|ws-(?:addr|listen-addr|interface|host))\b/.test(command), 'Unsupported diagnostic endpoint override');
  assert(!/\/(?:dns[46]?|dnsaddr|onion3?|p2p-circuit|unix|memory)\//i.test(command), 'Unsupported diagnostic address protocol');
  assert(!/--(?:no-mdns|no-telemetry|no-prometheus)=/.test(command), 'Safety flags accept no override values');
  const tokens = command.split(/\s+/);
  for (let index = 0; index < tokens.length; index++) {
    if (!/^--(?:listen-addr|bootnodes)(?:=|$)/.test(tokens[index])) continue;
    const option = tokens[index];
    const values = option.includes('=') ? [option.slice(option.indexOf('=') + 1)] : [];
    while (index + 1 < tokens.length && !tokens[index + 1].startsWith('--')) values.push(tokens[++index]);
    assert(values.length > 0, 'Multiaddress option requires a value');
    for (const value of values.flatMap(value => value.split(','))) {
      const match = /^\/ip4\/127\.0\.0\.1\/tcp\/(\d{1,5})(?:\/ws)?(?:\/p2p\/[a-z0-9]+)?$/i.exec(value);
      assert(match && Number(match[1]) <= 65535, 'Non-loopback or unsupported multiaddress');
    }
  }
  for (const part of command.split(' -- ')) {
    for (const flag of ['--no-mdns', '--no-telemetry', '--no-prometheus']) {
      assert.equal(part.split(/\s+/).filter(token => token === flag).length, 1, `Exactly one ${flag} required`);
    }
    assert(/--listen-addr[= ]\/ip4\/127\.0\.0\.1\/tcp\//.test(part), 'Explicit loopback P2P required');
    assert(!/(?:^| )--port(?:=| )/.test(part), 'P2P --port conflicts with explicit --listen-addr');
  }
}
function diagnosticCommand(command) {
  safeCommand(command);
  assert(['polkadot', 'polkadot-omni-node-deos'].includes(command.split(/\s+/)[0]), 'Unknown diagnostic executable');
}
function diagnosticConfig(config) {
  configStrings(config);
  assert.equal(config.settings?.provider, 'native');
  assert.equal(config.settings?.bootnode, false, 'No auxiliary service bootnode');
  assert.equal(config.relaychain.chain, 'rococo-local');
  assert.equal(config.relaychain.default_command, 'polkadot');
  assert(config.relaychain.nodes.every(node => !node.command || node.command === 'polkadot'));
  assert(config.parachains.every(para => para.collators.every(node => node.command === 'polkadot-omni-node-deos')));
  assert.deepEqual(config.relaychain.nodes.map(n => n.name), ['alice', 'bob']);
  assert(config.relaychain.nodes.every(n => n.validator === true));
  assert.equal(config.parachains.length, 1);
  assert.equal(config.parachains[0].id, 2000);
  assert.deepEqual(config.parachains[0].collators.map(n => n.name), ['alice', 'bob']);
  const serialized = JSON.stringify(config);
  assert(!/https?:|command_with_args|chain_spec_command|default_overrides|"overrides"|db_snapshot|node_groups|collator_groups|telemetry_url|bootnodes|"env"/i.test(serialized), 'Unsupported diagnostic override');
  assert(!config.relaychain.chain_spec_path && config.relaychain.nodes.every(node => !node.chain_spec_path), 'Diagnostic relay spec must be generated from rococo-local');
}
async function preflight() {
  const identity = diagnosticIdentity(input);
  const { config } = reviewedInput(input);
  const spec = JSON.parse(fs.readFileSync(config.parachains[0].chain_spec_path, 'utf8'));
  assert.equal(spec.extensions?.para_id ?? spec.para_id, 2000);
  assert(spec.genesis?.raw?.top?.['0x3a636f6465']);
  assert(!spec.bootNodes?.length);
  const req = requireTool();
  const { generateNetworkSpec } = req('@zombienet/orchestrator/dist/configGenerator.js');
  const { genCmd, genCumulusCollatorCmd } = req('@zombienet/orchestrator/dist/cmdGenerator.js');
  config.configBasePath = path.dirname(input);
  const network = await generateNetworkSpec(config);
  const commands = [];
  for (const node of network.relaychain.nodes) commands.push(...await genCmd(node, '/local/cfg', '/local/data', false));
  for (const node of network.parachains[0].collators) commands.push(...await genCumulusCollatorCmd(node, '/local/cfg', '/local/data', '/local/relay-data', false));
  commands.forEach(diagnosticCommand);
  assert.deepEqual(diagnosticIdentity(input), identity, 'Diagnostic input changed during generation');
  save('diagnostic-inputs.json', identity);
  save('preflight-commands.json', commands);
  console.log('Non-promoting diagnostic native/loopback/secure command preflight passed');
}
function freshCommand(command, networkDir) {
  safeCommand(command);
  const basePath = path.join(fs.realpathSync(networkDir), 'bob-1-fresh');
  assert(!fs.existsSync(basePath) && !fs.existsSync(path.join(networkDir, 'bob-1-fresh')), 'Fresh base path must not exist');
  assert.throws(() => fs.lstatSync(basePath), { code: 'ENOENT' });
  let count = 0;
  const result = command.replace(/--base-path[= ]([^ ]+)/g, (_, oldPath) => {
    const suffix = ++count === 1 ? 'data' : 'relay-data';
    assert.equal(oldPath, path.join(fs.realpathSync(networkDir), 'bob-1', suffix));
    return `--base-path ${path.join(basePath, suffix)}`;
  }).replace(/ --(?:collator|force-authoring)(?= |$)/g, '');
  assert.equal(count, 2, 'Separate parachain and embedded relay base paths required');
  safeCommand(result);
  return { command: result, basePath, role: 'non-authoring full node', absentAt: new Date().toISOString() };
}
function rootFailure(networkDir) {
  for (const file of fs.readdirSync(networkDir).filter(name => name.endsWith('.log'))) {
    const log = fs.readFileSync(path.join(networkDir, file), 'utf8');
    if (/mandatory.{0,160}(?:resource|admission|budget).{0,160}(?:fail|exceed|insufficient)|system quarter|MandatoryDispatch|ExhaustsResources/i.test(log)) throw new Error(`Mandatory resource boundary: ${file}`);
  }
}
async function fixedHistory() {
  const { ApiPromise, WsProvider } = requireTool()('@polkadot/api');
  const providers = ['ws://127.0.0.1:9988', 'ws://127.0.0.1:9999'].map(url => new WsProvider(url));
  const apis = await Promise.all(providers.map(provider => ApiPromise.create({ provider, noInitWarn: true, throwOnConnect: true })));
  try {
    const checkpoint = JSON.parse(fs.readFileSync(path.join(directory, 'checkpoint.json'), 'utf8'));
    const deadline = Date.now() + 300_000;
    const snapshots = [];
    while (Date.now() < deadline) {
      rootFailure(input);
      const local = await providers[1].send('chain_getHeader', [checkpoint.paraHash]);
      snapshots.push({ at: new Date().toISOString(), header: local, peers: (await apis[1].rpc.system.peers()).toJSON(), health: (await apis[1].rpc.system.health()).toJSON() });
      if (local) {
        for (const [index, api] of apis.entries()) {
          assert.equal((await api.rpc.chain.getBlockHash(0)).toHex(), checkpoint.genesis[index + 1]);
          assert.equal((await api.rpc.chain.getBlockHash(checkpoint.paraNumber)).toHex(), checkpoint.paraHash);
          assert.deepEqual((await api.rpc.chain.getHeader(checkpoint.paraHash)).toJSON(), checkpoint.paraHeader);
          const pairs = topPairs(await providers[index].send('state_getPairs', ['0x', checkpoint.paraHash]));
          assert.deepEqual(pairs, JSON.parse(fs.readFileSync(path.join(directory, 'checkpoint-primary.top.json'), 'utf8')));
          save(`${label}-${index === 0 ? 'primary' : 'secondary'}.top.json`, pairs);
        }
        save(`${label}.json`, { observedAt: new Date().toISOString(), fixedCheckpoint: checkpoint, snapshots });
        console.log(`Pre-existing accepted history converged at ${checkpoint.paraHash}`);
        return;
      }
      await new Promise(resolve => setTimeout(resolve, 2000));
    }
    save(`${label}-incomplete.json`, snapshots);
    throw new Error('Fresh peer did not import fixed pre-existing checkpoint within 300s');
  } finally { await Promise.allSettled(apis.map(api => api.disconnect())); }
}
async function checkpoint() {
  const { ApiPromise, WsProvider } = requireTool()('@polkadot/api');
  const endpoints = ['ws://127.0.0.1:9944', 'ws://127.0.0.1:9988', 'ws://127.0.0.1:9999'];
  const providers = endpoints.map(endpoint => new WsProvider(endpoint));
  const apis = await Promise.all(providers.map(provider => ApiPromise.create({ provider, noInitWarn: true, throwOnConnect: true })));
  const [relay, primary, secondary] = apis;
  let seq = 0;
  const record = (kind, value) => save(`${label}-${String(++seq).padStart(4, '0')}-${kind}.json`, value);
  const deadline = Date.now() + 300_000;
  const sleep = () => new Promise(resolve => setTimeout(resolve, 2000));
  try {
    record('metadata', { relay: (await relay.rpc.state.getMetadata()).toHex(), para: (await primary.rpc.state.getMetadata()).toHex() });
    const genesis = await Promise.all(apis.map(api => api.rpc.chain.getBlockHash(0)));
    assert.equal(genesis[1].toHex(), genesis[2].toHex());
    record('genesis', genesis.map(x => x.toHex()));
    let accepted;
    while (Date.now() < deadline) {
      rootFailure(input);
      const h = await relay.rpc.chain.getFinalizedHead();
      const header = await relay.rpc.chain.getHeader(h);
      if (header.number.toNumber() === 0) { await sleep(); continue; }
      const at = await relay.at(h);
      assert(at.query.paras?.heads, 'Required Paras metadata surface');
      const head = await at.query.paras.heads(2000);
      if (head.isNone) { await sleep(); continue; }
      const p = primary.registry.createType('Header', head.unwrap().toU8a(true));
      if (p.number.toNumber() === 0) { await sleep(); continue; }
      const previous = label === 'after-restart' ? JSON.parse(fs.readFileSync(path.join(directory, 'checkpoint.json'), 'utf8')) : null;
      if (previous && p.number.toBigInt() <= BigInt(previous.paraNumber)) { await sleep(); continue; }
      const pHash = p.hash.toHex();
      let cursor = h, inclusion = null;
      const ancestry = [];
      for (let i = 0; i < 100; i++) {
        const cursorHeader = await relay.rpc.chain.getHeader(cursor);
        ancestry.push({ hash: cursor.toHex(), header: cursorHeader.toJSON() });
        const historical = await relay.at(cursor);
        const events = await historical.query.system.events();
        for (const { event } of events) {
          if (event.section === 'paraInclusion' && event.method === 'CandidateIncluded' && event.data[0].descriptor.paraId.toNumber() === 2000) {
            const included = primary.registry.createType('Header', event.data[1].toU8a(true));
            if (included.hash.toHex() === pHash) inclusion = { relayHash: cursor.toHex(), event: event.toJSON() };
          }
        }
        if (inclusion || cursorHeader.number.toNumber() === 0) break;
        cursor = cursorHeader.parentHash;
      }
      if (!inclusion) { await sleep(); continue; }
      record('finalized-relay-ancestry', ancestry);
      record('inclusion', inclusion);
      const results = [];
      let synced = true;
      for (const [index, api] of [primary, secondary].entries()) {
        const local = await api.rpc.chain.getHeader(pHash);
        if (local.hash.toHex() !== pHash || JSON.stringify(local.toJSON()) !== JSON.stringify(p.toJSON())) { synced = false; break; }
        const canonical = await api.rpc.chain.getBlockHash(p.number);
        if (canonical.toHex() !== pHash) { synced = false; break; }
        const pairs = topPairs(await providers[index + 1].send('state_getPairs', ['0x', pHash]));
        results.push({ header: local.toJSON(), pairs });
      }
      if (!synced) { await sleep(); continue; }
      assert.deepEqual(results[0], results[1], 'Independent peers differ at relay-finalized checkpoint');
      const canonicalRelay = await relay.rpc.chain.getBlockHash(header.number);
      assert.equal(canonicalRelay.toHex(), h.toHex());
      accepted = { acceptedAt: new Date().toISOString(), relayHash: h.toHex(), relayHeader: header.toJSON(), inclusion, paraHash: pHash, paraNumber: p.number.toString(), paraHeader: p.toJSON(), genesis: genesis.map(x => x.toHex()), topPairs: results[0].pairs.length };
      save(`${label}-primary.top.json`, results[0].pairs);
      save(`${label}-secondary.top.json`, results[1].pairs);
      save(`${label}.json`, accepted);
      console.log(`Relay-finalized inclusion and independent complete TOP equality: ${h.toHex()} -> ${pHash}`);
      break;
    }
    assert(accepted, 'No relay-finalized included nonzero para checkpoint within 300s');
  } finally {
    await Promise.allSettled(apis.map(api => api.disconnect()));
  }
}
function diagnosticLaunchArguments(args) {
  assert.deepEqual(args, ['spawn', '--provider', 'native'], 'Only fixed native spawn route accepted');
}
function profileSelfTest() {
  const fixture = fs.mkdtempSync('/tmp/deos-tool-profile-test-');
  try {
    const root = path.join(fixture, 'cli');
    fs.mkdirSync(root);
    const sources = ['configGenerator.js', 'keys.js', 'mocha.js', 'reader.js', 'bootnode.js'];
    fs.writeFileSync(path.join(root, 'package.json'), JSON.stringify({ name: '@zombienet/cli', version: '1.3.138' }));
    for (const name of sources) fs.writeFileSync(path.join(root, name), 'throw new Error("must never import fixture");');
    fs.symlinkSync('keys.js', path.join(root, 'link'));
    const runtime = path.join(fixture, 'node');
    fs.writeFileSync(runtime, 'reviewed runtime');
    const profile = { tree: treeDigest(root), node: fileDigest(runtime), expires: diagnosticProfile.expires };
    const check = (env = {}, now = Date.parse('2026-11-14'), platform = 'linux', arch = 'x64') => installedProfile(root, runtime, env, now, profile, platform, arch);
    check();
    for (const name of sources) {
      const file = path.join(root, name), original = fs.readFileSync(file);
      fs.appendFileSync(file, 'mutation');
      assert.throws(() => check(), /source drift/);
      fs.writeFileSync(file, original);
    }
    fs.writeFileSync(path.join(root, 'unexpected.cjs'), 'unexpected loadable code');
    assert.throws(() => check(), /source drift/);
    fs.unlinkSync(path.join(root, 'unexpected.cjs'));
    fs.unlinkSync(path.join(root, 'link'));
    fs.symlinkSync('reader.js', path.join(root, 'link'));
    assert.throws(() => check(), /source drift/);
    fs.unlinkSync(path.join(root, 'link'));
    fs.symlinkSync(runtime, path.join(root, 'link'));
    assert.throws(() => check(), /External/);
    fs.unlinkSync(path.join(root, 'link'));
    fs.symlinkSync('keys.js', path.join(root, 'link'));
    const missing = fs.readFileSync(path.join(root, 'keys.js'));
    fs.unlinkSync(path.join(root, 'keys.js'));
    assert.throws(() => check());
    fs.writeFileSync(path.join(root, 'keys.js'), missing);
    fs.appendFileSync(runtime, 'drift');
    assert.throws(() => check(), /Node executable drift/);
    fs.writeFileSync(runtime, 'reviewed runtime');
    const targets = path.join(fixture, 'targets');
    fs.mkdirSync(targets);
    const accepted = {};
    for (const name of Object.keys(diagnosticProfile.binaries)) {
      fs.writeFileSync(path.join(targets, name), 'reviewed target');
      accepted[name] = fileDigest(path.join(targets, name));
    }
    diagnosticBinaries(targets, accepted);
    for (const name of Object.keys(accepted)) {
      const target = path.join(targets, name);
      fs.appendFileSync(target, 'drift');
      assert.throws(() => diagnosticBinaries(targets, accepted), /drift/);
      fs.unlinkSync(target);
      assert.throws(() => diagnosticBinaries(targets, accepted), /ENOENT/);
      fs.writeFileSync(target, 'reviewed target');
    }
    for (const key of ['NODE_OPTIONS', 'NODE_PATH', 'NODE_EXTRA_CA_CERTS', 'LD_PRELOAD', 'LD_LIBRARY_PATH', 'BASH_ENV', 'ENV', 'BASH_FUNC_polkadot%%', 'ZOMBIE_JAEGER_URL', 'RUN_IN_CONTAINER', 'PUSHGATEWAY_URL']) assert.throws(() => check({ [key]: 'injected' }), /injection/);
    assert.throws(() => check({}, Date.parse(profile.expires)), /expired/);
    assert.throws(() => check({}, undefined, 'darwin'), /Linux/);
    assert.throws(() => check({}, undefined, 'linux', 'arm64'), /x86_64/);
    check();
    const config = structuredClone(reviewedTopology);
    config.relaychain.default_args[config.relaychain.default_args.indexOf('--workers-path') + 1] = '/different/owned/workdir';
    config.parachains[0].chain_spec_path = '/different/owned/spec.json';
    const file = path.join(fixture, 'config.json');
    const write = value => fs.writeFileSync(file, JSON.stringify(value));
    write(config);
    reviewedInput(file);
    for (const suffix of ['.toml', '.JSON', '.js']) {
      fs.writeFileSync(file + suffix, JSON.stringify(config));
      assert.throws(() => reviewedInput(file + suffix), /suffix/);
    }
    for (const payload of ['key = "value"', '{"settings":', ...['{{', '{%', '{#'].map(delimiter => JSON.stringify({ ...config, template: delimiter }))]) {
      fs.writeFileSync(file, payload);
      assert.throws(() => reviewedInput(file));
    }
    for (const mutate of [
      value => { value.action = 'test'; },
      value => { value.settings.reporter = 'custom.js'; },
      value => { value.relaychain.nodes[0].key = 'imported'; },
      value => { value.settings.env = ['NODE_OPTIONS=--require=custom']; },
      value => { value.parachains[0].collators[0].args.push('--rpc-external'); },
      value => { value.relaychain.default_args.push('--workers-path', '/other'); },
      value => { value.parachains[0].id = 2001; },
    ]) {
      const changed = structuredClone(config); mutate(changed); write(changed);
      assert.throws(() => reviewedInput(file));
    }
    write(config);
    reviewedInput(file);
    diagnosticLaunchArguments(['spawn', '--provider', 'native']);
    for (const command of ['custom.js', 'bash', '/unknown/polkadot']) assert.throws(() => diagnosticCommand(`${command} --no-mdns --no-telemetry --no-prometheus --listen-addr /ip4/127.0.0.1/tcp/30333`), /Unknown diagnostic executable/);
    for (const args of [['test', '--provider', 'native'], ['spawn', '--provider', 'native', '--provider', 'podman'], ['setup'], ['spawn', '--provider', 'native', '--unknown']]) assert.throws(() => diagnosticLaunchArguments(args));
    assert.throws(() => verifyFrozenDiagnostic(file, fixture), /ENOENT/);
    const spec = path.join(fixture, 'spec.json'), binary = path.join(fixture, 'relay');
    fs.writeFileSync(spec, 'reviewed spec');
    fs.writeFileSync(binary, 'reviewed target');
    const identify = () => ({ config: fileDigest(file), spec: fileDigest(spec), target: fileDigest(binary) });
    fs.writeFileSync(path.join(fixture, 'diagnostic-inputs.json'), JSON.stringify(identify()));
    verifyFrozenDiagnostic(file, fixture, identify);
    for (const changed of [file, spec, binary]) {
      const original = fs.readFileSync(changed);
      fs.appendFileSync(changed, 'drift');
      assert.throws(() => verifyFrozenDiagnostic(file, fixture, identify), /Frozen diagnostic input drift/);
      fs.writeFileSync(changed, original);
    }
  } finally { fs.rmSync(fixture, { recursive: true }); }
  console.log('Diagnostic profile/source/runtime/environment/config/route negative tests passed (no tool imports)');
}
function launchBoundarySelfTest() {
  const scripts = path.dirname(fileURLToPath(import.meta.url));
  const fixture = fs.mkdtempSync('/tmp/deos-launch-boundary-test-');
  try {
    const marker = path.join(fixture, 'unexpected-node-start');
    const preload = path.join(fixture, 'preload.cjs');
    fs.writeFileSync(preload, `require('node:fs').writeFileSync(${JSON.stringify(marker)}, 'injected'); throw new Error('unexpected Node startup');`);
    for (const [script, args] of [['05-spawn-zombienet.sh', []], ['network-assurance-local.sh', ['--prepared-diagnostic']], ['network-assurance-local.sh', ['--self-test']]]) {
      const result = spawnSync('bash', [path.join(scripts, script), ...args], {
        env: { ...process.env, DEOS_NETWORK_DIAGNOSTIC: '1', NODE_OPTIONS: `--require=${preload}` }, encoding: 'utf8', timeout: 10000,
      });
      assert.equal(result.status, 1, result.stderr);
      assert.match(result.stderr, /Diagnostic injection environment: NODE_OPTIONS/);
      assert(!fs.existsSync(marker), 'Node preload executed before Bash admission');
    }
    // Exercise the actual startup owner with a rejecting verifier and an exec
    // sentinel. No CLI import, child node or socket belongs to this fixture.
    const result = spawnSync('bash', ['-c', `
      source "$1/05-spawn-zombienet.sh"
      diagnostic_environment() { :; }
      node() { [[ "$2" != launch-check ]]; }
      exec() { echo unexpected > "$DEOS_NETWORK_EVIDENCE_DIR/unexpected-final-exec"; }
      export DEOS_NETWORK_DIAGNOSTIC=1 DEOS_NETWORK_EVIDENCE_DIR="$2"
      NETWORK_WORK_DIR="$2" ZOMBIENET_CONFIG="$2/config.json"
      spawn_network
    `, 'boundary-test', scripts, fixture], { encoding: 'utf8', timeout: 10000 });
    assert.equal(result.status, 1, result.stderr);
    assert(!fs.existsSync(path.join(fixture, 'unexpected-final-exec')));
  } finally { fs.rmSync(fixture, { recursive: true }); }
  console.log('Direct/wrapper pre-Node refusal and sole startup boundary self-tests passed');
}
function selfTest() {
  profileSelfTest();
  launchBoundarySelfTest();
  assert.deepEqual(topPairs([['0x3a636f6465', '0xff'], ['0x01', '0x']]), [['0x01', '0x'], ['0x3a636f6465', '0xff']]);
  for (const value of [[], null, [['0x3a636f6465', '0xf']], [['0x01', '0xff']], [['0x3a636f6465', '0xff'], ['0x3a636f6465', '0x']]]) assert.throws(() => topPairs(value));
  const safe = 'node --no-mdns --no-telemetry --no-prometheus --listen-addr /ip4/127.0.0.1/tcp/3333/ws';
  safeCommand(safe);
  for (const extra of [' --insecure-validator-i-know-what-i-do', ' --rpc-external', ' --prometheus-external', ' --listen-addr /ip4/0.0.0.0/tcp/3', ' --bootnodes /ip4/8.8.8.8/tcp/3', ' --no-mdns', ' --port 3333', '; other-command', ' --relay-chain-rpc-url ws://8.8.8.8:9944', ' --experimental-rpc-endpoint anything', '\n injected', '\r injected', ' --reserved-nodes /dns6/example.org/tcp/30333/p2p/12D3KooWFixture', ' --bootnodes /dns6/example.org/tcp/30333/p2p/12D3KooWFixture', ' --bootnodes /dnsaddr/example.org', ' --rpc-addr 8.8.8.8']) assert.throws(() => safeCommand(safe + extra));
  for (const escaped of ['\\n', '\\r']) assert.throws(() => configStrings(JSON.parse(`{"args":["--name${escaped}injected"]}`)));
  safeCommand(safe + ' --bootnodes /ip4/127.0.0.1/tcp/4444/ws/p2p/12D3KooWFixture');
  safeCommand(safe + ' --bootnodes=/ip4/127.0.0.1/tcp/4444/ws/p2p/12D3KooWFixture');
  for (const suffix of [' # --rpc-external', ' --name "#"', ' --name wildcard*', ' --name escaped\\ value', ' --name (nested)', ' --bootnodes=/garlic64/QUJD', ' --bootnodes /garlic64/QUJD', ' --bootnodes', ' --listen-addr=/garlic64/QUJD', ' --bootnodes=/ip4/127.0.0.1/tcp/65536', ' --no-mdns=false']) assert.throws(() => safeCommand(safe + suffix), `Unsafe suffix accepted: ${suffix}`);
  const config = { settings: { provider: 'native', bootnode: false }, relaychain: { chain: 'rococo-local', default_command: 'polkadot', nodes: ['alice', 'bob'].map(name => ({ name, validator: true })) }, parachains: [{ id: 2000, collators: ['alice', 'bob'].map(name => ({ name, command: 'polkadot-omni-node-deos' })) }] };
  diagnosticConfig(config);
  for (const node of [config.relaychain.nodes[0], config.parachains[0].collators[0]]) {
    node.overrides = [{ local_path: '/uninspected/raw.json', remote_name: 'rococo-local.json' }];
    assert.throws(() => diagnosticConfig(config), /Unsupported diagnostic override/);
    delete node.overrides;
  }
  config.relaychain.chain_spec_path = '/uninspected/raw.json';
  assert.throws(() => diagnosticConfig(config), /relay spec must be generated/);
  delete config.relaychain.chain_spec_path;
  const fixture = fs.mkdtempSync('/tmp/deos-fresh-path-test-');
  try {
    const command = `${safe} --collator --force-authoring --base-path ${fixture}/bob-1/data -- ${safe} --base-path ${fixture}/bob-1/relay-data`;
    const fresh = freshCommand(command, fixture);
    assert(!fresh.command.includes('--collator'));
    assert(!fresh.command.includes('--force-authoring'));
    assert(fresh.command.includes(`${fresh.basePath}/relay-data`));
    assert(!fs.existsSync(fresh.basePath), 'Command preparation must not create database');
    fs.mkdirSync(fresh.basePath);
    assert.throws(() => freshCommand(command, fixture));
    fs.rmdirSync(fresh.basePath);
    fs.symlinkSync('/missing-fresh-path-test-target', fresh.basePath);
    assert.throws(() => freshCommand(command, fixture));
    assert.throws(() => freshCommand(command.replace('/bob-1/data', '/elsewhere/data'), fixture));
    const output = path.join(fixture, 'existing.json');
    fs.writeFileSync(output, 'preserve');
    assert.throws(() => fs.writeFileSync(output, 'replace', { flag: 'wx' }));
    assert.equal(fs.readFileSync(output, 'utf8'), 'preserve');
  } finally { fs.rmSync(fixture, { recursive: true }); }
  console.log('Network structural leaf self-test passed');
}
if (['preflight', 'profile-check'].includes(mode)) assert.equal(process.argv.length, 6, 'Unexpected diagnostic arguments');
if (mode === 'preflight') await preflight();
else if (mode === 'profile-check') console.log(JSON.stringify(diagnosticIdentity(input), null, 2));
else if (mode === 'launch-check') {
  diagnosticLaunchArguments(process.argv.slice(6));
  verifyFrozenDiagnostic();
}
else if (mode === 'checkpoint') await checkpoint();
else if (mode === 'self-test') selfTest();
else if (mode === 'command-preflight') {
  if (process.env.DEOS_NETWORK_DIAGNOSTIC === '1') diagnosticCommand(executable);
  else safeCommand(executable);
}
else if (mode === 'fixed-history') await fixedHistory();
else if (mode === 'fresh-command') {
  const checkpoint = JSON.parse(fs.readFileSync(path.join(directory, 'checkpoint.json'), 'utf8'));
  assert(BigInt(checkpoint.paraNumber) > 0n && checkpoint.acceptedAt, 'Accepted checkpoint must precede fresh database');
  const network = JSON.parse(fs.readFileSync(path.join(input, 'zombie.json'), 'utf8'));
  save('fresh-instance.json', { ...freshCommand(network.client.processMap['bob-1'].cmd[0], input), checkpoint });
}
else throw new Error(`Unknown structural mode: ${mode}`);
