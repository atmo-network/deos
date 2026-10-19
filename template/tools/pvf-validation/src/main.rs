mod cpu_trace;

use codec::{DecodeAll, Encode};
use cpu_trace::CpuTrace;
use polkadot_sdk::{
  cumulus_primitives_core::ParachainBlockData,
  polkadot_node_core_pvf::{
    self as pvf, InvalidCandidate, PrepareJobKind, Priority, PvfPrepData, ValidationError,
    ValidationHost,
  },
  polkadot_node_core_pvf_common::execute::ValidationContext,
  polkadot_node_primitives::{NODE_VERSION, PoV},
  polkadot_node_subsystem::messages::PvfExecKind,
  polkadot_parachain_primitives::primitives::{ValidationCode, ValidationParams, ValidationResult},
  polkadot_primitives::{
    CandidateCommitments, CandidateDescriptorV2, CandidateReceiptV2, CoreIndex, ExecutorParams,
    MAX_CODE_SIZE, PersistedValidationData,
    executor_params::{DEFAULT_BACKING_EXECUTION_TIMEOUT, DEFAULT_LENIENT_PREPARATION_TIMEOUT},
  },
  sp_core::H256,
  sp_runtime::{
    DigestItem, OpaqueExtrinsic, generic,
    traits::{BlakeTwo256, Hash, Header as HeaderT},
  },
};
use serde_json::json;
use std::{fs, path::Path, sync::Arc, time::Duration};

type Header = generic::Header<u32, BlakeTwo256>;
type Block = generic::Block<Header, OpaqueExtrinsic>;
type Result<T> = std::result::Result<T, String>;

const USAGE: &str = "Usage: deos-pvf-validation WASM FIXTURES WORKERS PARA_ID [--insecure-local] [--cpu-trace FILE] [--positive-only]\n\nValidates every exported *.params.scale fixture with the actual SDK ValidationHost,\npinned-version prepare/execute workers and default SDK execution parameters. Checks\nall expected ValidationResult fields, corrupt Aura seals, parent roots and compact proofs.\n--positive-only requires exactly one fixture and runs only its complete positive check;\nit constructs no corrupt cases and does not establish negative-suite acceptance.\nSecure validator mode is required unless --insecure-local is explicitly supplied for\ntrusted local code. Checksums belong to scripts/01-download-binaries.sh --check.\nReceipts are locally constructed V2 inputs, not relay-authenticated provenance.\n--cpu-trace creates a new JSONL file with source-bound SDK events and accepted whole-job\nchild CPU, not wall time, root-only cost or a calibrated tariff. Missing or ambiguous\ntrace data fails closed; partial files without a complete record are not assurance.\nNo runtime, network, account, production weight or resource maximum is selected.\n";

#[derive(Clone)]
struct Fixture {
  params: ValidationParams,
  pvd: PersistedValidationData,
  expected: ValidationResult,
}

fn read_scale<T: DecodeAll>(path: &Path) -> Result<T> {
  let bytes = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
  T::decode_all(&mut bytes.as_slice()).map_err(|e| format!("decode {}: {e}", path.display()))
}

impl Fixture {
  fn read(root: &Path, preset: &str) -> Result<Self> {
    let params: ValidationParams = read_scale(&root.join(format!("{preset}.params.scale")))?;
    let pvd: PersistedValidationData = read_scale(&root.join(format!("{preset}.pvd.scale")))?;
    let expected: ValidationResult =
      read_scale(&root.join(format!("{preset}.expected-result.scale")))?;
    let head = fs::read(root.join(format!("{preset}.expected-head.scale")))
      .map_err(|e| format!("read expected head: {e}"))?;
    let block: Block = read_scale(&root.join(format!("{preset}.sealed-block.scale")))?;
    let data = ParachainBlockData::<Block>::decode_all(&mut params.block_data.0.as_slice())
      .map_err(|e| format!("decode block data: {e}"))?;
    if params.parent_head != pvd.parent_head
      || params.relay_parent_number != pvd.relay_parent_number
      || params.relay_parent_storage_root != pvd.relay_parent_storage_root
      || expected.head_data.0 != head
      || block.header.encode() != head
      || data.blocks() != [block]
      || params.block_data.0.len() > pvd.max_pov_size as usize
    {
      return Err(format!("{preset}: inconsistent fixture bindings"));
    }
    Ok(Self {
      params,
      pvd,
      expected,
    })
  }

  fn corrupt_seal(&self) -> Result<Self> {
    let mut altered = self.clone();
    let mut data =
      ParachainBlockData::<Block>::decode_all(&mut altered.params.block_data.0.as_slice())
        .map_err(|e| format!("decode negative block: {e}"))?;
    let block = data
      .blocks_mut()
      .last_mut()
      .ok_or("missing candidate block")?;
    let Some(DigestItem::Seal(engine, signature)) = block.header.digest_mut().logs.last_mut()
    else {
      return Err("missing final Aura seal".into());
    };
    if *engine != polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID {
      return Err("unexpected seal engine".into());
    }
    let byte = signature.first_mut().ok_or("empty seal")?;
    *byte ^= 1;
    altered.params.block_data.0 = data.encode();
    Ok(altered)
  }

  fn corrupt_proof(&self) -> Result<Self> {
    let mut altered = self.clone();
    let mut data =
      ParachainBlockData::<Block>::decode_all(&mut altered.params.block_data.0.as_slice())
        .map_err(|e| format!("decode proof input: {e}"))?;
    let proof = match &mut data {
      ParachainBlockData::V0 { proof, .. }
      | ParachainBlockData::V1 { proof, .. }
      | ParachainBlockData::V2 { proof, .. } => proof,
    };
    let byte = proof
      .encoded_nodes
      .first_mut()
      .and_then(|node| node.first_mut())
      .ok_or("empty compact proof")?;
    *byte ^= 1;
    altered.params.block_data.0 = data.encode();
    Ok(altered)
  }

  fn corrupt_parent(&self) -> Result<Self> {
    let mut altered = self.clone();
    let mut header = Header::decode_all(&mut altered.pvd.parent_head.0.as_slice())
      .map_err(|e| format!("decode parent: {e}"))?;
    let mut root = *header.state_root();
    root.as_bytes_mut()[0] ^= 1;
    header.set_state_root(root);
    altered.pvd.parent_head.0 = header.encode();
    altered.params.parent_head = altered.pvd.parent_head.clone();
    Ok(altered)
  }
}

fn commitments(expected: &ValidationResult) -> CandidateCommitments {
  CandidateCommitments {
    head_data: expected.head_data.clone(),
    new_validation_code: expected.new_validation_code.clone(),
    upward_messages: expected.upward_messages.clone(),
    horizontal_messages: expected.horizontal_messages.clone(),
    processed_downward_messages: expected.processed_downward_messages,
    hrmp_watermark: expected.hrmp_watermark,
  }
}

fn check_result(
  result: std::result::Result<ValidationResult, ValidationError>,
  expected: &ValidationResult,
  negative: bool,
) -> Result<()> {
  match (negative, result) {
    (false, Ok(actual)) if actual.encode() == expected.encode() => Ok(()),
    (true, Err(ValidationError::Invalid(InvalidCandidate::WorkerReportedInvalid(_)))) => Ok(()),
    (false, Ok(_)) => Err("ValidationResult differs from the complete native oracle".into()),
    (_, other) => Err(format!(
      "unexpected outcome (negative={negative}): {other:?}"
    )),
  }
}

async fn execute(
  host: &mut ValidationHost,
  code: &[u8],
  fixture: &Fixture,
  para_id: u32,
  negative: bool,
  trace: Option<&CpuTrace>,
  label: &str,
) -> Result<()> {
  let executor_params = ExecutorParams::default();
  let pov = PoV {
    block_data: fixture.params.block_data.clone(),
  };
  // The exported input has no authenticated relay receipt. These scheduling fields are synthetic.
  let relay_parent = H256::zero();
  let descriptor = CandidateDescriptorV2::new(
    para_id.into(),
    relay_parent,
    CoreIndex(0),
    0,
    fixture.pvd.hash(),
    pov.hash(),
    H256::zero(),
    BlakeTwo256::hash(&fixture.expected.head_data.0),
    ValidationCode(code.to_vec()).hash(),
  );
  let context = ValidationContext {
    candidate_receipt: CandidateReceiptV2 {
      descriptor,
      commitments_hash: commitments(&fixture.expected).hash(),
    },
    pvd: Arc::new(fixture.pvd.clone()),
    pov: Arc::new(pov),
    executor_params: executor_params.clone(),
    exec_timeout: DEFAULT_BACKING_EXECUTION_TIMEOUT,
    v3_seen: false,
  };
  let preparation = PvfPrepData::from_code(
    code.to_vec().into(),
    executor_params,
    DEFAULT_LENIENT_PREPARATION_TIMEOUT,
    PrepareJobKind::Compilation,
    MAX_CODE_SIZE.saturating_mul(4),
  );
  if let Some(trace) = trace {
    // ArtifactId is private SDK implementation; match its source-bound Debug event, not IPC.
    let artifact = format!(
      "ArtifactId {{ code_hash: {:?}, executor_params_prep_hash: {:?} }}",
      preparation.code_hash(),
      preparation.executor_params().prep_hash()
    );
    trace.begin(
      label,
      artifact,
      &sha256(&(&fixture.params, &fixture.pvd, &fixture.expected).encode()),
      negative,
    )?;
  }
  let (tx, rx) = futures::channel::oneshot::channel();
  host
    .execute_pvf(
      preparation,
      context,
      Priority::Normal,
      PvfExecKind::Backing(relay_parent),
      tx,
    )
    .await
    .map_err(|e| format!("submit PVF: {e}"))?;
  let result = tokio::time::timeout(
    DEFAULT_LENIENT_PREPARATION_TIMEOUT
      + DEFAULT_BACKING_EXECUTION_TIMEOUT
      + Duration::from_secs(30),
    rx,
  )
  .await
  .map_err(|_| "host result timeout; not a negative witness")?
  .map_err(|e| format!("host result canceled: {e}"))?;
  check_result(result, &fixture.expected, negative)?;
  if let Some(trace) = trace {
    trace.finish()?;
  }
  Ok(())
}

fn discover_fixtures(root: &Path) -> Result<Vec<String>> {
  let mut presets = Vec::new();
  for entry in fs::read_dir(root).map_err(|e| format!("fixture directory: {e}"))? {
    let entry = entry.map_err(|e| e.to_string())?;
    let name = entry
      .file_name()
      .into_string()
      .map_err(|_| "non-UTF8 fixture name")?;
    if let Some(preset) = name.strip_suffix(".params.scale") {
      presets.push(preset.to_owned());
    }
  }
  presets.sort();
  if presets.is_empty() {
    return Err("no exported fixtures; zero cases cannot qualify acceptance".into());
  }
  Ok(presets)
}

#[derive(Debug, PartialEq, Eq)]
enum CaseSelection {
  Full,
  PositiveOnly(String),
}

impl CaseSelection {
  fn new(root: &Path, positive_only: bool) -> Result<Self> {
    if !positive_only {
      return Ok(Self::Full);
    }
    let mut presets = discover_fixtures(root)?;
    if presets.len() != 1 {
      return Err(format!(
        "--positive-only requires exactly one fixture; found {}",
        presets.len()
      ));
    }
    Ok(Self::PositiveOnly(presets.remove(0)))
  }

  fn presets(&self, root: &Path) -> Result<Vec<String>> {
    match self {
      Self::Full => discover_fixtures(root),
      Self::PositiveOnly(preset) => Ok(vec![preset.clone()]),
    }
  }

  fn cases(&self, fixture: Fixture) -> Result<Vec<(&'static str, Fixture, bool)>> {
    if matches!(self, Self::PositiveOnly(_)) {
      return Ok(vec![("valid", fixture, false)]);
    }
    Ok(vec![
      ("valid", fixture.clone(), false),
      ("corrupt_seal", fixture.corrupt_seal()?, true),
      ("parent_root", fixture.corrupt_parent()?, true),
      ("compact_proof", fixture.corrupt_proof()?, true),
    ])
  }

  fn annotate_scope(&self, mut record: serde_json::Value) -> serde_json::Value {
    if matches!(self, Self::PositiveOnly(_)) {
      record["case_selection"] = json!("positive_only_single_fixture");
    }
    record
  }
}

async fn run_cases(
  host: &mut ValidationHost,
  code: &[u8],
  root: &Path,
  para_id: u32,
  trace: Option<&CpuTrace>,
  selection: &CaseSelection,
) -> Result<()> {
  for preset in selection.presets(root)? {
    let fixture = Fixture::read(root, &preset)?;
    for (case, input, negative) in selection.cases(fixture)? {
      execute(
        host,
        code,
        &input,
        para_id,
        negative,
        trace,
        &format!("{preset}/{case}"),
      )
      .await?;
      println!(
        "PVF_ACCEPTANCE_V1 preset={preset} case={case} matched=true negative={negative} relay_authenticated=false"
      );
    }
  }
  Ok(())
}

fn sha256(bytes: &[u8]) -> String {
  format!(
    "{}",
    polkadot_sdk::sp_core::hexdisplay::HexDisplay::from(&polkadot_sdk::sp_io::hashing::sha2_256(
      bytes
    ))
  )
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Options<'a> {
  insecure: bool,
  trace_path: Option<&'a str>,
  positive_only: bool,
}

fn options(args: &[String]) -> Result<Options<'_>> {
  if args.len() < 4 {
    return Err(USAGE.into());
  }
  let mut options = Options::default();
  let mut remaining = &args[4..];
  while let Some((flag, rest)) = remaining.split_first() {
    match flag.as_str() {
      "--insecure-local" if !options.insecure => {
        options.insecure = true;
        remaining = rest;
      }
      "--positive-only" if !options.positive_only => {
        options.positive_only = true;
        remaining = rest;
      }
      "--cpu-trace" if options.trace_path.is_none() => {
        let (path, rest) = rest.split_first().ok_or(USAGE)?;
        if path.is_empty() || path.starts_with("--") {
          return Err(USAGE.into());
        }
        options.trace_path = Some(path.as_str());
        remaining = rest;
      }
      _ => return Err(USAGE.into()),
    }
  }
  Ok(options)
}

#[tokio::main]
async fn main() -> Result<()> {
  let args = std::env::args().skip(1).collect::<Vec<_>>();
  if args == ["--help"] || args == ["-h"] {
    print!("{USAGE}");
    return Ok(());
  }
  let Options {
    insecure,
    trace_path,
    positive_only,
  } = options(&args)?;
  // Positive-only admission precedes every worker probe, trace and host side effect.
  // Full mode retains discovery at its original run_cases boundary.
  let selection = CaseSelection::new(Path::new(&args[1]), positive_only)?;
  if positive_only {
    println!("PVF_SELECTION_V1 mode=positive_only fixtures=1 cases=1 full_suite=false");
  }
  let code = fs::read(&args[0]).map_err(|e| format!("Wasm: {e}"))?;
  let root = Path::new(&args[1]);
  let workers = Path::new(&args[2]);
  let para_id = args[3]
    .parse::<u32>()
    .map_err(|e| format!("para ID: {e}"))?;
  let prepare = workers.join(pvf::PREPARE_BINARY_NAME);
  let execute = workers.join(pvf::EXECUTE_BINARY_NAME);
  for path in [&prepare, &execute] {
    let version = pvf::get_worker_version(path).map_err(|e| format!("worker version: {e}"))?;
    if version != NODE_VERSION {
      return Err(format!(
        "worker {} reports {version}; SDK requires {NODE_VERSION}",
        path.display()
      ));
    }
  }
  let trace = trace_path
    .map(|path| CpuTrace::install(Path::new(path)))
    .transpose()?;
  if let Some(trace) = &trace {
    let worker_sha = |path: &Path| {
      fs::read(path)
        .map(|bytes| sha256(&bytes))
        .map_err(|e| format!("executable identity {}: {e}", path.display()))
    };
    trace.record(selection.annotate_scope(json!({"kind":"session", "format":"DEOS_PVF_CPU_TRACE_V1", "node_version":NODE_VERSION,
      "timing_contract_sdk_commit":"7a8daa68e18718c18823c1109513fdc1c8aa71fe", "code_sha256":sha256(&code),
      "prepare_sha256":worker_sha(&prepare)?, "execute_sha256":worker_sha(&execute)?,
      "host_sha256":worker_sha(&std::env::current_exe().map_err(|e| format!("host identity: {e}"))?)?,
      "os":std::env::consts::OS, "arch":std::env::consts::ARCH,
      "para_id":para_id, "secure_required":!insecure, "executor_params_scale":format!("{:?}", ExecutorParams::default().encode()),
      "cache":"new temporary artifact directory; OS caches uncontrolled", "scope":"whole-job child user+system CPU; not root-only or calibrated tariff"})))?;
  }
  // Keep Unix socket paths short; SDK worker socket setup has a platform length limit.
  let cache = tempfile::Builder::new()
    .prefix("deos-pvf-")
    .tempdir()
    .map_err(|e| e.to_string())?;
  let config = pvf::Config::new(
    cache.path().into(),
    Some(NODE_VERSION.into()),
    !insecure,
    prepare,
    execute,
    1,
    1,
    1,
  );
  let (mut host, task) = pvf::start(config, pvf::Metrics::default())
    .await
    .map_err(|e| format!("ValidationHost startup: {e}"))?;
  println!(
    "PVF_HOST_V1 node_version={NODE_VERSION} code_sha256={} secure_required={} security={:?}",
    polkadot_sdk::sp_core::hexdisplay::HexDisplay::from(&polkadot_sdk::sp_io::hashing::sha2_256(
      &code
    )),
    !insecure,
    host.security_status
  );
  let task = tokio::spawn(task);
  let result = async {
    if let Some(trace) = &trace {
      trace.record(json!({"kind":"security", "status":format!("{:?}", host.security_status)}))?;
    }
    run_cases(&mut host, &code, root, para_id, trace.as_ref(), &selection).await
  }
  .await;
  drop(host);
  let mut task = task;
  match tokio::time::timeout(Duration::from_secs(10), &mut task).await {
    Ok(joined) => joined.map_err(|e| format!("ValidationHost task failed: {e}"))?,
    Err(_) => {
      task.abort();
      let _ = task.await;
      return Err("ValidationHost did not shut down gracefully".into());
    }
  }
  result?;
  if let Some(trace) = &trace {
    trace.record(selection.annotate_scope(json!({"kind":"complete", "acceptance":true})))?;
  }
  Ok(())
}

#[cfg(test)]
mod tests;
