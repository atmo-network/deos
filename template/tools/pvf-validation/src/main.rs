use codec::{DecodeAll, Encode};
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
use std::{fs, path::Path, sync::Arc, time::Duration};

type Header = generic::Header<u32, BlakeTwo256>;
type Block = generic::Block<Header, OpaqueExtrinsic>;
type Result<T> = std::result::Result<T, String>;

const USAGE: &str = "Usage: deos-pvf-validation WASM FIXTURES WORKERS PARA_ID [--insecure-local]\n\nValidates every exported *.params.scale fixture with the actual SDK ValidationHost,\npinned-version prepare/execute workers and default SDK execution parameters. Checks\nall expected ValidationResult fields, corrupt Aura seals, parent roots and compact proofs.\nSecure validator mode is required unless --insecure-local is explicitly supplied for\ntrusted local code. Checksums belong to scripts/01-download-binaries.sh --check.\nReceipts are locally constructed V2 inputs, not relay-authenticated provenance.\nNo runtime, network, account, production weight or resource maximum is selected.\n";

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
  let (tx, rx) = futures::channel::oneshot::channel();
  host
    .execute_pvf(
      PvfPrepData::from_code(
        code.to_vec().into(),
        executor_params,
        DEFAULT_LENIENT_PREPARATION_TIMEOUT,
        PrepareJobKind::Compilation,
        MAX_CODE_SIZE.saturating_mul(4),
      ),
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
  check_result(result, &fixture.expected, negative)
}

async fn run_cases(
  host: &mut ValidationHost,
  code: &[u8],
  root: &Path,
  para_id: u32,
) -> Result<()> {
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
  for preset in presets {
    let fixture = Fixture::read(root, &preset)?;
    for (case, input, negative) in [
      ("valid", fixture.clone(), false),
      ("corrupt_seal", fixture.corrupt_seal()?, true),
      ("parent_root", fixture.corrupt_parent()?, true),
      ("compact_proof", fixture.corrupt_proof()?, true),
    ] {
      execute(host, code, &input, para_id, negative).await?;
      println!(
        "PVF_ACCEPTANCE_V1 preset={preset} case={case} matched=true negative={negative} relay_authenticated=false"
      );
    }
  }
  Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
  let args = std::env::args().skip(1).collect::<Vec<_>>();
  if args == ["--help"] || args == ["-h"] {
    print!("{USAGE}");
    return Ok(());
  }
  let insecure = args.len() == 5 && args[4] == "--insecure-local";
  if args.len() != 4 && !insecure {
    return Err(USAGE.into());
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
  let result = run_cases(&mut host, &code, root, para_id).await;
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
  result
}

#[cfg(test)]
mod tests {
  use super::*;
  use polkadot_sdk::polkadot_parachain_primitives::primitives::HeadData;

  fn expected() -> ValidationResult {
    ValidationResult {
      head_data: HeadData(vec![1]),
      new_validation_code: None,
      upward_messages: Default::default(),
      horizontal_messages: Default::default(),
      processed_downward_messages: 0,
      hrmp_watermark: 2,
    }
  }

  #[test]
  fn only_exact_result_and_typed_worker_invalid_are_accepted() {
    let expected = expected();
    assert!(check_result(Ok(expected.clone()), &expected, false).is_ok());
    let mut changed = expected.clone();
    changed.hrmp_watermark += 1;
    assert!(check_result(Ok(changed), &expected, false).is_err());
    assert!(check_result(Ok(expected.clone()), &expected, true).is_err());
    assert!(
      check_result(
        Err(ValidationError::Invalid(InvalidCandidate::HardTimeout)),
        &expected,
        true
      )
      .is_err()
    );
    assert!(check_result(Err(ValidationError::ExecutionDeadline), &expected, true).is_err());
    assert!(
      check_result(
        Err(ValidationError::Invalid(
          InvalidCandidate::WorkerReportedInvalid("seal".into())
        )),
        &expected,
        true
      )
      .is_ok()
    );
  }
}
