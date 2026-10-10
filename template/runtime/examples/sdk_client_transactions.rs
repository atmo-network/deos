//! Generated SDK client transaction-depth and failed-call rollback regression.

use codec::Encode;
use polkadot_sdk::{
  sp_api::{self, ApiError, ApiExt, CallApiAt, CallApiAtParams, ConstructRuntimeApi},
  sp_core::{H256, traits::CallContext},
  sp_externalities::{Extension, Extensions, TransactionType},
  sp_runtime::{
    ExtrinsicInclusionMode, OpaqueExtrinsic, generic,
    traits::{BlakeTwo256, Block as BlockT},
  },
  sp_state_machine::InMemoryBackend,
  sp_version::RuntimeVersion,
};
use std::{
  any::{Any, TypeId},
  sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU32, Ordering},
  },
};

type Block = generic::Block<generic::Header<u32, BlakeTwo256>, OpaqueExtrinsic>;
const FAILED_CALL_KEY: &[u8] = b"sdk-client-failed-call";

sp_api::decl_runtime_apis! {
  pub trait ClientTransactionsApi {
    fn depth() -> u32;
    fn fail() -> u32;
    fn leaked() -> bool;
  }
}

struct Runtime;

fn version() -> RuntimeVersion {
  RuntimeVersion {
    spec_name: "client-transactions".into(),
    impl_name: "client-transactions".into(),
    authoring_version: 1,
    spec_version: 1,
    impl_version: 1,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 1,
    system_version: 1,
  }
}

sp_api::impl_runtime_apis! {
  impl sp_api::Core<Block> for Runtime {
    fn version() -> RuntimeVersion { version() }
    fn execute_block(_block: <Block as BlockT>::LazyBlock) {}
    fn initialize_block(_header: &<Block as BlockT>::Header) -> ExtrinsicInclusionMode {
      ExtrinsicInclusionMode::AllExtrinsics
    }
  }

  impl self::ClientTransactionsApi<Block> for Runtime {
    fn depth() -> u32 { unreachable!("client-only regression") }
    fn fail() -> u32 { unreachable!("client-only regression") }
    fn leaked() -> bool { unreachable!("client-only regression") }
  }
}

#[derive(Default)]
struct Client {
  dispatches: AtomicU32,
  fail_version_lookup: AtomicBool,
  fail_extension_lookup: AtomicBool,
}

impl CallApiAt<Block> for Client {
  type StateBackend = InMemoryBackend<BlakeTwo256>;

  fn call_api_at(&self, params: CallApiAtParams<Block>) -> Result<Vec<u8>, ApiError> {
    self.dispatches.fetch_add(1, Ordering::Relaxed);
    let mut overlay = params.overlayed_changes.borrow_mut();
    match params.function {
      "ClientTransactionsApi_depth" => Ok((overlay.transaction_depth() as u32).encode()),
      "ClientTransactionsApi_fail" => {
        overlay.set_storage(FAILED_CALL_KEY.to_vec(), Some(vec![1]));
        Err(ApiError::Application(
          std::io::Error::other("deliberate API failure").into(),
        ))
      }
      "ClientTransactionsApi_leaked" => Ok(
        overlay
          .storage(FAILED_CALL_KEY)
          .flatten()
          .is_some()
          .encode(),
      ),
      name => panic!("unexpected API: {name}"),
    }
  }

  fn runtime_version_at(&self, _: H256, _: CallContext) -> Result<RuntimeVersion, ApiError> {
    if self.fail_version_lookup.swap(false, Ordering::Relaxed) {
      return Err(ApiError::UnknownBlock(
        "deliberate version lookup failure".into(),
      ));
    }
    Ok(version())
  }

  fn state_at(&self, _: H256) -> Result<Self::StateBackend, ApiError> {
    Ok(Default::default())
  }

  fn initialize_extensions(&self, _: H256, _: &mut Extensions) -> Result<(), ApiError> {
    if self.fail_extension_lookup.swap(false, Ordering::Relaxed) {
      return Err(ApiError::UnknownBlock(
        "deliberate extension lookup failure".into(),
      ));
    }
    Ok(())
  }
}

fn verify() {
  for mode in ["commit", "rollback", "nested"] {
    let client = Client::default();
    let api = RuntimeApi::construct_runtime_api(&client);
    let at = H256::zero();
    let first = api.depth(at).unwrap();
    let inside = api.execute_in_transaction(|api| {
      let depth = if mode == "nested" {
        api.execute_in_transaction(|api| sp_api::TransactionOutcome::Commit(api.depth(at).unwrap()))
      } else {
        api.depth(at).unwrap()
      };
      if mode == "rollback" {
        sp_api::TransactionOutcome::Rollback(depth)
      } else {
        sp_api::TransactionOutcome::Commit(depth)
      }
    });
    let after = api.depth(at).unwrap();
    assert!(api.fail(at).is_err());
    let leaked = api.leaked(at).unwrap();
    println!(
      "{{\"mode\":\"{mode}\",\"first_api_depth\":{first},\"inside_explicit_depth\":{inside},\"after_explicit_depth\":{after},\"failed_call_leaked\":{leaked}}}"
    );
    assert_eq!(first, 1);
    assert_eq!(inside, if mode == "nested" { 2 } else { 1 });
    assert_eq!(after, 1, "API re-entry must open its implicit host layer");
    assert!(
      !leaked,
      "failed API calls must restore the previous overlay"
    );
  }
}

#[derive(Default)]
struct HostTrace {
  depth: u32,
  outcomes: Vec<bool>,
}

struct HostTraceExt(Arc<Mutex<HostTrace>>);

impl HostTraceExt {
  fn finish(&mut self, ty: TransactionType, commit: bool) {
    assert!(ty.is_host());
    let mut trace = self.0.lock().unwrap();
    assert_eq!(trace.depth, 1);
    trace.depth = 0;
    assert!(trace.outcomes.len() < 8);
    trace.outcomes.push(commit);
  }
}

impl Extension for HostTraceExt {
  fn as_mut_any(&mut self) -> &mut dyn Any {
    self
  }
  fn type_id(&self) -> TypeId {
    TypeId::of::<Self>()
  }
  fn start_transaction(&mut self, ty: TransactionType) {
    assert!(ty.is_host());
    let mut trace = self.0.lock().unwrap();
    assert_eq!(trace.depth, 0);
    trace.depth = 1;
  }
  fn commit_transaction(&mut self, ty: TransactionType) {
    self.finish(ty, true);
  }
  fn rollback_transaction(&mut self, ty: TransactionType) {
    self.finish(ty, false);
  }
}

fn verify_early_errors() {
  for phase in ["version_lookup", "extension_lookup", "block_identity"] {
    let client = Client::default();
    let mut api = RuntimeApi::construct_runtime_api(&client);
    let trace = Arc::new(Mutex::new(HostTrace::default()));
    api.register_extension(HostTraceExt(trace.clone()));
    let at = H256::zero();
    api.execute_in_transaction(|_| sp_api::TransactionOutcome::Commit(()));
    let requested = match phase {
      "version_lookup" => {
        client.fail_version_lookup.store(true, Ordering::Relaxed);
        at
      }
      "extension_lookup" => {
        client.fail_extension_lookup.store(true, Ordering::Relaxed);
        at
      }
      "block_identity" => {
        assert_eq!(api.depth(at).unwrap(), 1);
        H256::repeat_byte(1)
      }
      _ => unreachable!("finite caller-guard inventory"),
    };
    let dispatches = client.dispatches.load(Ordering::Relaxed);
    let error = api.depth(requested).unwrap_err();
    if phase == "block_identity" {
      assert!(matches!(
        error,
        ApiError::UsingSameInstanceForDifferentBlocks
      ));
    } else {
      assert!(matches!(error, ApiError::UnknownBlock(_)));
    }
    assert_eq!(client.dispatches.load(Ordering::Relaxed), dispatches);
    {
      let trace = trace.lock().unwrap();
      assert_eq!(trace.depth, 0);
      assert_eq!(trace.outcomes.last(), Some(&false));
    }
    assert_eq!(api.depth(at).unwrap(), 1);
    assert_eq!(client.dispatches.load(Ordering::Relaxed), dispatches + 1);
    let trace = trace.lock().unwrap();
    let expected = if phase == "block_identity" {
      vec![true, true, false, true]
    } else {
      vec![true, false, true]
    };
    assert_eq!(trace.depth, 0);
    assert_eq!(trace.outcomes, expected);
    println!(
      "{{\"early_error\":\"{phase}\",\"requested_dispatch_skipped\":true,\"host_rollback\":true,\"retry_api_depth\":1}}"
    );
  }
}

fn main() {
  let args = std::env::args().collect::<Vec<_>>();
  if args.len() == 2 && args[1] == "--help" {
    println!(
      "SDK client transaction regression.\nUsage: sdk_client_transactions [--help]\nChecks generated API re-entry and failed-call overlay rollback after explicit commit, rollback and nesting. Early version/extension lookup and block-identity guards verify skipped requested dispatch, diagnostic host rollback and retry using the workspace SDK. The caller is diagnostic: this does not execute a runtime, Wasm or node, qualify proof recording or concrete Node extensions, or certify physical Weight."
    );
    return;
  }
  assert_eq!(args.len(), 1, "unexpected argument; see --help");
  verify();
  verify_early_errors();
}

#[cfg(test)]
mod tests {
  #[test]
  fn explicit_scopes_restore_api_reentry_and_failed_call_rollback() {
    super::verify();
  }

  #[test]
  fn early_client_guards_rollback_before_dispatch_and_allow_retry() {
    super::verify_early_errors();
  }
}
