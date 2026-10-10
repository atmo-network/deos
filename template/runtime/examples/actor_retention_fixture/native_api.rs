//! Executive replay through the SDK's generated Core client and StateMachine.
//! The selected executor uses compiled Rust or an explicitly supplied Wasm artifact.

use codec::{Decode, Encode};
use deos_runtime::{Block, Executive, Runtime};
use polkadot_sdk::{
  sp_api::{
    self, ApiError, ApiExt, ApiRef, CallApiAt, CallApiAtParams, ConstructRuntimeApi, Core,
    ProvideRuntimeApi,
  },
  sp_block_builder::{self, runtime_decl_for_block_builder::BlockBuilder},
  sp_core::{
    H256,
    storage::Storage,
    traits::{
      CallContext, CodeExecutor, ReadRuntimeVersion, ReadRuntimeVersionExt, RuntimeCode,
      WrappedRuntimeCode,
    },
  },
  sp_externalities::{
    Extension, Extensions, Externalities, TransactionType, set_and_run_with_externalities,
  },
  sp_inherents::{CheckInherentsResult, InherentData},
  sp_runtime::{
    ApplyExtrinsicResult, ExtrinsicInclusionMode,
    traits::{BlakeTwo256, Block as BlockT, Hash, Header as HeaderT},
  },
  sp_state_machine::{InMemoryBackend, OverlayedChanges, StateMachine, TrieBackendBuilder},
  sp_trie::{
    ProofSizeProvider,
    proof_size_extension::{
      ProofSizeExt, RecordedProofSizeEstimations, RecordingProofSizeProvider,
      ReplayProofSizeProvider,
    },
  },
  sp_version::RuntimeVersion,
};
use std::{
  any::{Any, TypeId},
  borrow::Cow,
  collections::BTreeMap,
  panic::AssertUnwindSafe,
  sync::{Arc, Mutex},
};

struct NativeRuntime;

#[cfg(not(feature = "runtime-benchmarks"))]
type WasmHostFunctions = (
  polkadot_sdk::sp_io::SubstrateHostFunctions,
  cumulus_primitives_proof_size_hostfunction::storage_proof_size::HostFunctions,
);

#[cfg(feature = "runtime-benchmarks")]
type WasmHostFunctions = (
  polkadot_sdk::sp_io::SubstrateHostFunctions,
  cumulus_primitives_proof_size_hostfunction::storage_proof_size::HostFunctions,
  polkadot_sdk::frame_benchmarking::benchmarking::HostFunctions,
);

#[derive(Clone)]
enum RuntimeExecution {
  Native,
  Wasm {
    code: Arc<[u8]>,
    executor: sc_executor::WasmExecutor<WasmHostFunctions>,
    version: RuntimeVersion,
  },
}

impl RuntimeExecution {
  fn wasm(code: Vec<u8>) -> Self {
    let executor = sc_executor::WasmExecutor::<WasmHostFunctions>::builder().build();
    let mut ext = polkadot_sdk::sp_io::TestExternalities::default();
    let bytes = executor
      .read_runtime_version(&code, &mut ext.ext())
      .unwrap();
    let code_fetcher = WrappedRuntimeCode(Cow::Borrowed(&code));
    let runtime_code = RuntimeCode {
      code_fetcher: &code_fetcher,
      heap_pages: Some(8192),
      hash: BlakeTwo256::hash(&code).as_bytes().to_vec(),
    };
    let (result, native) = executor.call(
      &mut ext.ext(),
      &runtime_code,
      "Core_version",
      &[],
      CallContext::Onchain { import: false },
    );
    assert!(!native, "runtime version must come from actual Wasm");
    let actual = RuntimeVersion::decode(&mut result.unwrap().as_slice()).unwrap();
    let mut normalized = actual.clone();
    let mut embedded = RuntimeVersion::decode(&mut bytes.as_slice()).unwrap();
    let mut expected = deos_runtime::VERSION;
    normalized.apis.to_mut().sort_unstable();
    embedded.apis.to_mut().sort_unstable();
    expected.apis.to_mut().sort_unstable();
    assert_eq!(
      normalized, embedded,
      "Wasm Core and embedded versions agree"
    );
    assert_eq!(
      normalized, expected,
      "Wasm and reference runtime versions agree"
    );
    Self::Wasm {
      code: code.into(),
      executor,
      version: actual,
    }
  }

  fn name(&self) -> &'static str {
    match self {
      Self::Native => "native",
      Self::Wasm { .. } => "wasm",
    }
  }

  fn code(&self) -> &[u8] {
    match self {
      Self::Native => &[],
      Self::Wasm { code, .. } => code,
    }
  }

  fn version(&self) -> RuntimeVersion {
    match self {
      Self::Native => version(),
      Self::Wasm { version, .. } => version.clone(),
    }
  }
}

fn version() -> RuntimeVersion {
  RuntimeVersion {
    apis: RUNTIME_API_VERSIONS,
    ..deos_runtime::VERSION
  }
}

sp_api::impl_runtime_apis! {
  impl sp_api::Core<Block> for NativeRuntime {
    fn version() -> RuntimeVersion { version() }
    fn execute_block(block: <Block as BlockT>::LazyBlock) {
      Executive::execute_block(block)
    }
    fn initialize_block(header: &<Block as BlockT>::Header) -> ExtrinsicInclusionMode {
      Executive::initialize_block(header)
    }
  }

  impl sp_block_builder::BlockBuilder<Block> for NativeRuntime {
    fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
      Executive::apply_extrinsic(extrinsic)
    }
    fn finalize_block() -> <Block as BlockT>::Header { Executive::finalize_block() }
    fn inherent_extrinsics(data: InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
      <Runtime as BlockBuilder<Block>>::inherent_extrinsics(data)
    }
    fn check_inherents(block: <Block as BlockT>::LazyBlock, data: InherentData) -> CheckInherentsResult {
      <Runtime as BlockBuilder<Block>>::check_inherents(block, data)
    }
  }
}

#[derive(Clone, Copy, Debug)]
enum ResponseFailure {
  Executor,
  Decode,
}

#[derive(Clone)]
struct CloseFailure {
  method: &'static str,
  key: Vec<u8>,
  kind: ResponseFailure,
}

#[derive(Clone)]
struct NativeExecutor {
  fail_after_close: Option<CloseFailure>,
  finalization_close: Option<Vec<u8>>,
  execution: RuntimeExecution,
}

impl ReadRuntimeVersion for NativeExecutor {
  fn read_runtime_version(
    &self,
    code: &[u8],
    ext: &mut dyn Externalities,
  ) -> Result<Vec<u8>, String> {
    match &self.execution {
      RuntimeExecution::Native => Ok(version().encode()),
      RuntimeExecution::Wasm { executor, .. } => executor.read_runtime_version(code, ext),
    }
  }
}

impl CodeExecutor for NativeExecutor {
  type Error = std::io::Error;

  fn call(
    &self,
    ext: &mut dyn Externalities,
    code: &RuntimeCode,
    method: &str,
    data: &[u8],
    context: CallContext,
  ) -> (Result<Vec<u8>, Self::Error>, bool) {
    assert!(
      ext
        .extension_by_type_id(TypeId::of::<ReadRuntimeVersionExt>())
        .is_some(),
      "the default SDK client registers its version extension before dispatch"
    );
    assert!(
      ext
        .extension_by_type_id(TypeId::of::<ProofSizeExt>())
        .is_some(),
      "the selected author/import paths carry their shared proof-size extension"
    );
    let finalization_key = (method == "BlockBuilder_finalize_block")
      .then_some(self.finalization_close.as_ref())
      .flatten();
    if let Some(key) = finalization_key {
      assert!(
        ext.storage(key).is_some(),
        "terminal Actor must survive until finalization"
      );
    }
    let result = match &self.execution {
      RuntimeExecution::Native => std::panic::catch_unwind(AssertUnwindSafe(|| {
        set_and_run_with_externalities(ext, || api::dispatch(method, data))
      }))
      .map_err(|_| std::io::Error::other("native runtime trapped"))
      .and_then(|output| output.ok_or_else(|| std::io::Error::other("unknown native API"))),
      RuntimeExecution::Wasm { executor, .. } => {
        let (result, native) = executor.call(ext, code, method, data, context);
        assert!(
          !native,
          "the selected Wasm path cannot use a native fallback"
        );
        result.map_err(|error| std::io::Error::other(error.to_string()))
      }
    }
    .and_then(|output| {
      if let Some(key) = finalization_key {
        assert!(
          ext.storage(key).is_none(),
          "finalization must execute the terminal Close"
        );
      }
      if let Some(failure) = &self.fail_after_close {
        assert_eq!(method, failure.method);
        match method {
          "BlockBuilder_apply_extrinsic" => assert_eq!(
            ApplyExtrinsicResult::decode(&mut output.as_slice()).unwrap(),
            Ok(Ok(()))
          ),
          "BlockBuilder_finalize_block" => {
            <Block as BlockT>::Header::decode(&mut output.as_slice()).unwrap();
          }
          _ => panic!("unsupported post-Close failure phase"),
        }
        assert!(
          ext.storage(&failure.key).is_none(),
          "Close must execute before the injected error"
        );
        return match failure.kind {
          ResponseFailure::Executor => Err(std::io::Error::other(
            "deliberate post-Close executor failure",
          )),
          ResponseFailure::Decode => Ok(Vec::new()),
        };
      }
      Ok(output)
    });
    (result, matches!(self.execution, RuntimeExecution::Native))
  }
}

#[derive(Default)]
struct TransactionTrace {
  value: u32,
  frames: Vec<(TransactionType, u32)>,
  host_outcomes: Vec<bool>,
}

struct TransactionTraceExt(Arc<Mutex<TransactionTrace>>);

impl TransactionTraceExt {
  fn finish(&mut self, ty: TransactionType, commit: bool) {
    let mut trace = self.0.lock().unwrap();
    let (opened, value) = trace.frames.pop().unwrap();
    assert_eq!(opened, ty);
    if !commit {
      trace.value = value;
    }
    if ty.is_host() {
      assert!(trace.host_outcomes.len() < 32);
      trace.host_outcomes.push(commit);
    }
  }
}

impl Extension for TransactionTraceExt {
  fn as_mut_any(&mut self) -> &mut dyn Any {
    self
  }
  fn type_id(&self) -> TypeId {
    TypeId::of::<Self>()
  }
  fn start_transaction(&mut self, ty: TransactionType) {
    let mut trace = self.0.lock().unwrap();
    assert!(trace.frames.len() < 64);
    let value = trace.value;
    trace.frames.push((ty, value));
  }
  fn commit_transaction(&mut self, ty: TransactionType) {
    self.finish(ty, true);
  }
  fn rollback_transaction(&mut self, ty: TransactionType) {
    self.finish(ty, false);
  }
}

fn overlay_snapshot(overlay: &mut OverlayedChanges<BlakeTwo256>) -> Vec<u8> {
  let changes = overlay
    .changes_mut()
    .map(|(key, entry)| {
      let value = entry.value().cloned();
      (key.clone(), (value, entry.extrinsics()))
    })
    .collect::<BTreeMap<_, _>>();
  assert!(
    changes.len() < 10_000,
    "this finite block's dirty overlay is bounded"
  );
  changes.encode()
}

#[derive(Clone, Copy)]
enum ProofProvider<'a> {
  Plain,
  Recording,
  Replay(&'a [usize]),
}

impl ProofProvider<'_> {
  fn name(self) -> &'static str {
    match self {
      Self::Plain => "plain",
      Self::Recording => "recording",
      Self::Replay(_) => "replay",
    }
  }
}

struct ObservedReplay {
  provider: ReplayProofSizeProvider,
  observations: Arc<Mutex<Vec<usize>>>,
}

impl ProofSizeProvider for ObservedReplay {
  fn estimate_encoded_size(&self) -> usize {
    let value = self.provider.estimate_encoded_size();
    let mut observations = self.observations.lock().unwrap();
    assert!(observations.len() < 10_000, "finite fixture query bound");
    observations.push(value);
    value
  }
  // ReplayProofSizeProvider uses the trait's default no-op transaction callbacks.
}

struct ProofContext {
  recorder: sp_api::ProofRecorder<Block>,
  recordings: Option<RecordingProofSizeProvider>,
  replay_observations: Option<Arc<Mutex<Vec<usize>>>>,
}

impl ProofContext {
  fn new(provider: ProofProvider<'_>) -> (Self, ProofSizeExt) {
    let recorder = sp_api::ProofRecorder::<Block>::default();
    let mut context = Self {
      recorder,
      recordings: None,
      replay_observations: None,
    };
    let extension = match provider {
      ProofProvider::Plain => ProofSizeExt::new(context.recorder.clone()),
      ProofProvider::Recording => {
        let recording = RecordingProofSizeProvider::new(context.recorder.clone());
        context.recordings = Some(recording.clone());
        ProofSizeExt::new(recording)
      }
      ProofProvider::Replay(recorded) => {
        assert!(recorded.len() < 10_000, "finite fixture query bound");
        let observations = Arc::new(Mutex::new(Vec::new()));
        context.replay_observations = Some(observations.clone());
        ProofSizeExt::new(ObservedReplay {
          provider: ReplayProofSizeProvider::from_recorded(RecordedProofSizeEstimations(
            recorded.iter(/* deos-bypass: bounded-iter -- replay input is checked below 10,000 immediately above. */).copied().collect(),
          )),
          observations,
        })
      }
    };
    (context, extension)
  }

  fn recorded(&self) -> Vec<usize> {
    let recorded = self
      .recordings
      .as_ref()
      .map_or_else(Vec::new, RecordingProofSizeProvider::recorded_estimations);
    assert!(recorded.len() < 10_000, "finite fixture query bound");
    recorded
  }
}

struct NativeClient {
  execution: RuntimeExecution,
  storage: Storage,
  backend: InMemoryBackend<BlakeTwo256>,
  depths: Mutex<Vec<(usize, usize)>>,
  fail_next_close: Mutex<Option<CloseFailure>>,
  expected_rollback: Mutex<Option<Vec<u8>>>,
  trace: Option<Arc<Mutex<TransactionTrace>>>,
  finalization_close: Option<Vec<u8>>,
  state_requests: Mutex<usize>,
  recordings: Option<RecordingProofSizeProvider>,
}

impl CallApiAt<Block> for NativeClient {
  type StateBackend = InMemoryBackend<BlakeTwo256>;

  fn call_api_at(&self, params: CallApiAtParams<Block>) -> Result<Vec<u8>, ApiError> {
    assert!(matches!(params.call_context, CallContext::Onchain { .. }));
    let mut overlay = params.overlayed_changes.borrow_mut();
    if let Some(expected) = self.expected_rollback.lock().unwrap().take() {
      assert_eq!(
        overlay_snapshot(&mut overlay),
        expected,
        "API error must restore dirty values and extrinsic provenance"
      );
    }
    let failed_close = self.fail_next_close.lock().unwrap().take();
    if failed_close
      .as_ref()
      .is_some_and(|failure| failure.method == "BlockBuilder_apply_extrinsic")
    {
      *self.expected_rollback.lock().unwrap() = Some(overlay_snapshot(&mut overlay));
    }
    let before = overlay.transaction_depth();
    assert_eq!(
      before, 1,
      "every API call must have one client transaction; explicit scopes must restore logical depth"
    );
    let mut extensions = params.extensions.borrow_mut();
    let code_fetcher = WrappedRuntimeCode(Cow::Borrowed(self.execution.code()));
    let code = RuntimeCode {
      code_fetcher: &code_fetcher,
      heap_pages: matches!(self.execution, RuntimeExecution::Wasm { .. }).then_some(8192),
      hash: match self.execution {
        RuntimeExecution::Native => Vec::new(),
        RuntimeExecution::Wasm { .. } => {
          BlakeTwo256::hash(self.execution.code()).as_bytes().to_vec()
        }
      },
    };
    let recorder = params
      .recorder
      .as_ref()
      .expect("import proof recording is enabled");
    let backend = TrieBackendBuilder::wrap(&self.backend)
      .with_recorder(recorder.clone())
      .build();
    let executor = NativeExecutor {
      fail_after_close: failed_close,
      finalization_close: self.finalization_close.clone(),
      execution: self.execution.clone(),
    };
    let recorded_before = self
      .recordings
      .as_ref()
      .map(|recording| recording.recorded_estimations().len());
    let result = StateMachine::new(
      &backend,
      &mut overlay,
      &executor,
      params.function,
      &params.arguments,
      &mut extensions,
      &code,
      params.call_context,
    )
    .set_parent_hash(params.at)
    .execute();
    if executor.fail_after_close.is_some() || result.is_err() {
      if let Some(recording) = &self.recordings {
        let provisional = recording.recorded_estimations().len();
        assert!(provisional >= recorded_before.unwrap());
        if params.function != "BlockBuilder_finalize_block" {
          assert!(provisional > recorded_before.unwrap());
        }
        println!(
          "native_sdk_recording_error function={} estimates_before={} estimates_provisional={provisional}",
          params.function,
          recorded_before.unwrap(),
        );
      }
    }
    if let Some(failure) = &executor.fail_after_close {
      match failure.kind {
        ResponseFailure::Executor => assert!(
          result.is_err(),
          "the executed Close must return an executor error"
        ),
        ResponseFailure::Decode => assert!(
          result.as_ref().unwrap().is_empty(),
          "the executed Close must return malformed response bytes"
        ),
      }
    }
    if let Some(trace) = &self.trace {
      trace.lock().unwrap().value += 1;
    }
    let after = overlay.transaction_depth();
    self.depths.lock().unwrap().push((before, after));
    assert_eq!(
      after, before,
      "runtime must preserve the protected client layer"
    );
    result.map_err(|error| ApiError::Application(std::io::Error::other(error.to_string()).into()))
  }

  fn runtime_version_at(&self, _: H256, _: CallContext) -> Result<RuntimeVersion, ApiError> {
    Ok(self.execution.version())
  }

  fn state_at(&self, _: H256) -> Result<Self::StateBackend, ApiError> {
    *self.state_requests.lock().unwrap() += 1;
    Ok(
      (
        self.storage.clone(),
        self.execution.version().state_version(),
      )
        .into(),
    )
  }

  fn initialize_extensions(&self, _: H256, extensions: &mut Extensions) -> Result<(), ApiError> {
    // Match the selected SDK's empty factory plus default version extension.
    let mut initialized = Extensions::new();
    initialized.register(ReadRuntimeVersionExt::new(NativeExecutor {
      fail_after_close: None,
      finalization_close: None,
      execution: self.execution.clone(),
    }));
    extensions.merge(initialized);
    Ok(())
  }
}

impl ProvideRuntimeApi<Block> for NativeClient {
  type Api = <RuntimeApi as ConstructRuntimeApi<Block, Self>>::RuntimeApi;

  fn runtime_api(&self) -> ApiRef<'_, Self::Api> {
    RuntimeApi::construct_runtime_api(self)
  }
}

pub(super) fn assert_builder(
  storage: Storage,
  expected: Storage,
  block: &Block,
  data: InherentData,
  close_key: Vec<u8>,
  finalization_close: Option<Vec<u8>>,
  wasm: Option<&[u8]>,
) {
  let execution = wasm.map_or(RuntimeExecution::Native, |code| {
    RuntimeExecution::wasm(code.to_vec())
  });
  assert_builder_with_provider(
    storage.clone(),
    expected.clone(),
    block,
    data.clone(),
    close_key.clone(),
    finalization_close.clone(),
    (ProofProvider::Plain, execution.clone()),
  );
  let recorded = assert_builder_with_provider(
    storage.clone(),
    expected.clone(),
    block,
    data,
    close_key,
    finalization_close,
    (ProofProvider::Recording, execution.clone()),
  );
  assert!(!recorded.is_empty());
  let imported = assert_import_with_provider(
    storage.clone(),
    expected.clone(),
    block,
    ProofProvider::Recording,
    execution.clone(),
  );
  assert_eq!(imported, recorded, "author/import estimation order agrees");
  assert_import_with_provider(
    storage,
    expected,
    block,
    ProofProvider::Replay(&recorded),
    execution,
  );
}

fn assert_builder_with_provider(
  storage: Storage,
  expected: Storage,
  block: &Block,
  data: InherentData,
  close_key: Vec<u8>,
  finalization_close: Option<Vec<u8>>,
  (provider, execution): (ProofProvider<'_>, RuntimeExecution),
) -> Vec<usize> {
  assert!(storage.children_default.is_empty() && expected.children_default.is_empty());
  let finalization_inputs = finalization_close
    .clone()
    .map(|key| (storage.clone(), data.clone(), key));
  let trace = Arc::new(Mutex::new(TransactionTrace::default()));
  let (proof_context, proof_extension) = ProofContext::new(provider);
  let client = NativeClient {
    backend: (storage.clone(), execution.version().state_version()).into(),
    execution: execution.clone(),
    storage: storage.clone(),
    depths: Mutex::new(Vec::new()),
    fail_next_close: Mutex::new(None),
    expected_rollback: Mutex::new(None),
    trace: Some(trace.clone()),
    finalization_close,
    state_requests: Mutex::new(0),
    recordings: proof_context.recordings.clone(),
  };
  let recorder = &proof_context.recorder;
  let mut extensions = Extensions::new();
  extensions.register(proof_extension);
  extensions.register(TransactionTraceExt(trace.clone()));
  println!(
    "SDK BlockBuilder execution begins: execution={}",
    execution.name()
  );
  let mut builder = sc_block_builder::BlockBuilderBuilder::new(&client)
    .on_parent_block(*block.header.parent_hash())
    .with_parent_block_number(block.header.number() - 1)
    .with_inherent_digests(block.header.digest().clone())
    .with_proof_recorder(recorder.clone())
    .with_extra_extensions(extensions)
    .build()
    .unwrap();
  let inherents = builder.create_inherents(data).unwrap();
  assert!(inherents.len() >= 3 && inherents.len() + 1 == block.extrinsics.len());
  assert_eq!(
    inherents.encode(),
    block.extrinsics[..inherents.len()].encode()
  );
  for extrinsic in &block.extrinsics[..inherents.len()] {
    builder.push(extrinsic.clone()).unwrap();
  }
  let close = block.extrinsics.last().unwrap();
  assert!(client.storage.top.contains_key(&close_key));
  let proof_before_error = recorder.estimate_encoded_size();
  let nodes_before_error = recorder.to_storage_proof();
  let extension_before_error = trace.lock().unwrap().value;
  let recorded_before_error = proof_context.recorded();
  *client.fail_next_close.lock().unwrap() = Some(CloseFailure {
    method: "BlockBuilder_apply_extrinsic",
    key: close_key.clone(),
    kind: ResponseFailure::Executor,
  });
  assert!(builder.push(close.clone()).is_err());
  assert_eq!(recorder.estimate_encoded_size(), proof_before_error);
  assert_eq!(recorder.to_storage_proof(), nodes_before_error);
  assert_eq!(proof_context.recorded(), recorded_before_error);
  {
    let trace = trace.lock().unwrap();
    assert_eq!(trace.value, extension_before_error);
    assert!(trace.frames.is_empty());
    assert_eq!(trace.host_outcomes.last(), Some(&false));
  }
  *client.fail_next_close.lock().unwrap() = Some(CloseFailure {
    method: "BlockBuilder_apply_extrinsic",
    key: close_key,
    kind: ResponseFailure::Decode,
  });
  let decode_error: ApiError = builder.push(close.clone()).unwrap_err().into();
  assert!(matches!(decode_error, ApiError::FailedToDecodeReturnValue {
    function: "BlockBuilder_apply_extrinsic", raw, ..
  } if raw.is_empty()));
  assert_eq!(recorder.estimate_encoded_size(), proof_before_error);
  assert_eq!(recorder.to_storage_proof(), nodes_before_error);
  assert_eq!(proof_context.recorded(), recorded_before_error);
  {
    let trace = trace.lock().unwrap();
    assert_eq!(trace.value, extension_before_error);
    assert!(trace.frames.is_empty());
    assert_eq!(trace.host_outcomes.last(), Some(&false));
  }
  builder.push(close.clone()).unwrap();
  assert!(client.expected_rollback.lock().unwrap().is_none());
  let proof_before_invalid = recorder.estimate_encoded_size();
  let nodes_before_invalid = recorder.to_storage_proof();
  let recorded_before_invalid = proof_context.recorded();
  assert!(
    builder
      .push(block.extrinsics.last().unwrap().clone())
      .is_err()
  );
  assert_eq!(recorder.estimate_encoded_size(), proof_before_invalid);
  assert_eq!(recorder.to_storage_proof(), nodes_before_invalid);
  assert_eq!(proof_context.recorded(), recorded_before_invalid);
  let built = builder.build().unwrap();
  assert_eq!(*client.state_requests.lock().unwrap(), 1);
  assert_eq!(built.block.encode(), block.encode());
  assert!(built.storage_changes.child_storage_changes.is_empty());
  assert_eq!(
    built.storage_changes.transaction_storage_root,
    *block.header.state_root()
  );
  let changed_keys = built.storage_changes.main_storage_changes.len();
  let mut post = storage;
  for (key, value) in built.storage_changes.main_storage_changes {
    match value {
      Some(value) => {
        post.top.insert(key, value);
      }
      None => {
        post.top.remove(&key);
      }
    }
  }
  assert_eq!(post.top, expected.top);
  let depths = client.depths.lock().unwrap();
  let calls = block.extrinsics.len() + 6 + usize::from(cfg!(debug_assertions));
  assert_eq!(*depths, vec![(1, 1); calls]);
  let trace = trace.lock().unwrap();
  let mut outcomes = vec![true];
  outcomes.push(false);
  outcomes.extend(vec![true; inherents.len()]);
  outcomes.extend([false, false, true, false, true]);
  if cfg!(debug_assertions) {
    outcomes.push(true);
  }
  assert_eq!(trace.host_outcomes, outcomes);
  assert_eq!(
    trace.value as usize,
    block.extrinsics.len() + 2 + usize::from(cfg!(debug_assertions))
  );
  assert!(trace.frames.is_empty());
  let recorded = proof_context.recorded();
  println!(
    "native_sdk_builder execution={} api_calls={} host_depth=1 changed_keys={changed_keys} recorded_proof_bytes={} rejected_api_error=true rejected_decode_error=true extension_callbacks=true default_version_extension=true rejected_duplicate=true post_root={:?} proof_provider={} recorded_estimations={}",
    execution.name(),
    depths.len(),
    recorder.estimate_encoded_size(),
    block.header.state_root(),
    provider.name(),
    recorded.len(),
  );
  if let Some((storage, data, key)) = finalization_inputs {
    assert_finalization_errors(storage, block, data, key, provider, execution);
  }
  recorded
}

fn assert_finalization_errors(
  storage: Storage,
  block: &Block,
  data: InherentData,
  close_key: Vec<u8>,
  provider: ProofProvider<'_>,
  execution: RuntimeExecution,
) {
  for kind in [ResponseFailure::Executor, ResponseFailure::Decode] {
    let trace = Arc::new(Mutex::new(TransactionTrace::default()));
    let (proof_context, proof_extension) = ProofContext::new(provider);
    let client = NativeClient {
      backend: (storage.clone(), execution.version().state_version()).into(),
      execution: execution.clone(),
      storage: storage.clone(),
      depths: Mutex::new(Vec::new()),
      fail_next_close: Mutex::new(None),
      expected_rollback: Mutex::new(None),
      trace: Some(trace.clone()),
      finalization_close: Some(close_key.clone()),
      state_requests: Mutex::new(0),
      recordings: proof_context.recordings.clone(),
    };
    let recorder = &proof_context.recorder;
    let mut extensions = Extensions::new();
    extensions.register(proof_extension);
    extensions.register(TransactionTraceExt(trace.clone()));
    let mut builder = sc_block_builder::BlockBuilderBuilder::new(&client)
      .on_parent_block(*block.header.parent_hash())
      .with_parent_block_number(block.header.number() - 1)
      .with_inherent_digests(block.header.digest().clone())
      .with_proof_recorder(recorder.clone())
      .with_extra_extensions(extensions)
      .build()
      .unwrap();
    let inherents = builder.create_inherents(data.clone()).unwrap();
    assert_eq!(
      inherents.encode(),
      block.extrinsics[..inherents.len()].encode()
    );
    for extrinsic in &block.extrinsics {
      builder.push(extrinsic.clone()).unwrap();
    }
    let proof_before = recorder.estimate_encoded_size();
    let nodes_before = recorder.to_storage_proof();
    let extension_before = trace.lock().unwrap().value;
    let recorded_before = proof_context.recorded();
    *client.fail_next_close.lock().unwrap() = Some(CloseFailure {
      method: "BlockBuilder_finalize_block",
      key: close_key.clone(),
      kind,
    });
    let error: ApiError = match builder.build() {
      Ok(_) => panic!("failed finalization must not export a block or storage changes"),
      Err(error) => error.into(),
    };
    let committed = match kind {
      ResponseFailure::Executor => {
        assert!(matches!(error, ApiError::Application(_)));
        assert_eq!(recorder.estimate_encoded_size(), proof_before);
        assert_eq!(recorder.to_storage_proof(), nodes_before);
        false
      }
      ResponseFailure::Decode => {
        assert!(matches!(error, ApiError::FailedToDecodeReturnValue {
          function: "BlockBuilder_finalize_block", raw, ..
        } if raw.is_empty()));
        assert!(
          nodes_before
            .into_nodes()
            .is_subset(&recorder.to_storage_proof().into_nodes())
        );
        true
      }
    };
    let recorded_after = proof_context.recorded();
    if committed {
      assert!(recorded_after.starts_with(&recorded_before));
    } else {
      assert_eq!(recorded_after, recorded_before);
    }
    assert_eq!(*client.state_requests.lock().unwrap(), 0);
    assert!(client.expected_rollback.lock().unwrap().is_none());
    let depths = client.depths.lock().unwrap();
    assert_eq!(*depths, vec![(1, 1); block.extrinsics.len() + 3]);
    let trace = trace.lock().unwrap();
    assert!(trace.frames.is_empty());
    assert_eq!(trace.value, extension_before + u32::from(committed));
    let mut outcomes = vec![true, false];
    outcomes.extend(vec![true; block.extrinsics.len()]);
    outcomes.push(committed);
    assert_eq!(trace.host_outcomes, outcomes);
    println!(
      "native_sdk_finalization_error execution={} failure={kind:?} api_calls={} host_depth=1 close_executed=true block_exported=false state_requests=0 recorded_proof_bytes_before={proof_before} recorded_proof_bytes_after={} extension_committed={committed} proof_provider={} recorded_estimations_before={} recorded_estimations_after={}",
      execution.name(),
      depths.len(),
      recorder.estimate_encoded_size(),
      provider.name(),
      recorded_before.len(),
      recorded_after.len(),
    );
  }
}

pub(super) fn assert_import(storage: Storage, expected: Storage, block: &Block) {
  assert_import_with_provider(
    storage,
    expected,
    block,
    ProofProvider::Plain,
    RuntimeExecution::Native,
  );
}

fn assert_import_with_provider(
  storage: Storage,
  expected: Storage,
  block: &Block,
  provider: ProofProvider<'_>,
  execution: RuntimeExecution,
) -> Vec<usize> {
  assert!(storage.children_default.is_empty() && expected.children_default.is_empty());
  let mut recorded_output = Vec::new();
  println!("native API backend constructed");
  let parent = *block.header.parent_hash();
  for succeeds in [true, false] {
    let (proof_context, proof_extension) = ProofContext::new(provider);
    let client = NativeClient {
      backend: (storage.clone(), execution.version().state_version()).into(),
      execution: execution.clone(),
      storage: storage.clone(),
      depths: Mutex::new(Vec::new()),
      fail_next_close: Mutex::new(None),
      expected_rollback: Mutex::new(None),
      trace: None,
      finalization_close: None,
      state_requests: Mutex::new(0),
      recordings: proof_context.recordings.clone(),
    };
    println!(
      "API Core execution begins: success={succeeds} execution={}",
      execution.name()
    );
    let mut api = RuntimeApi::construct_runtime_api(&client);
    api.set_call_context(CallContext::Onchain { import: true });
    let recorder = &proof_context.recorder;
    api.record_proof_with_recorder(recorder.clone());
    api.register_extension(proof_extension);
    let mut input = block.clone();
    if !succeeds {
      input
        .header
        .set_state_root(*input.header.state_root() ^ H256::repeat_byte(1));
    }
    let result = api.execute_block(parent, input.into());
    assert_eq!(
      result.is_ok(),
      succeeds,
      "only the correct complete block may import"
    );
    let proof_bytes = recorder.estimate_encoded_size();
    let changes = api.into_storage_changes(&client.backend, parent).unwrap();
    assert!(changes.child_storage_changes.is_empty());
    let changed_keys = changes.main_storage_changes.len();
    let mut post = storage.clone();
    for (key, value) in changes.main_storage_changes {
      match value {
        Some(value) => {
          post.top.insert(key, value);
        }
        None => {
          post.top.remove(&key);
        }
      }
    }
    if succeeds {
      assert!(proof_bytes > 0 && changed_keys > 0);
      assert_eq!(post.top, expected.top);
      assert_eq!(changes.transaction_storage_root, *block.header.state_root());
      recorded_output = proof_context.recorded();
    } else {
      assert_eq!(
        post.top, storage.top,
        "late invalid-root failure must restore the complete pre-state"
      );
      assert_eq!(changed_keys, 0);
      assert_eq!(
        proof_bytes, 0,
        "host rollback must discard recorded execution nodes"
      );
      assert_eq!(changes.transaction_storage_root, *client.backend.root());
      assert!(proof_context.recorded().is_empty());
    }
    let replayed = proof_context
      .replay_observations
      .as_ref()
      .map(|observations| {
        let observations = observations.lock().unwrap();
        let ProofProvider::Replay(expected) = provider else {
          panic!("only the replay provider observes replay queries")
        };
        assert_eq!(observations.as_slice(), expected);
        observations.len()
      });
    println!(
      "native_api_import execution={} success={succeeds} host_depth=1 changed_keys={changed_keys} recorded_proof_bytes={proof_bytes} post_root={:?} proof_provider={} recorded_estimations={} replayed_estimations={}",
      execution.name(),
      changes.transaction_storage_root,
      provider.name(),
      proof_context.recorded().len(),
      replayed.unwrap_or_default(),
    );
    assert_eq!(*client.depths.lock().unwrap(), vec![(1, 1)]);
  }
  recorded_output
}
