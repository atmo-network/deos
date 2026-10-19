//! Opt-in guest call-site evidence at existing imports, not complete phase attribution.

use super::*;
use sc_executor::sp_wasm_interface::{
  HostFunctionRegistry, HostFunctions, Value, anyhow, wasmtime,
};
use std::cell::RefCell;

type OriginalProofHost =
  cumulus_primitives_proof_size_hostfunction::storage_proof_size::HostFunctions;
pub(super) type ObservedHostFunctions = (ObservedSubstrateHost, ObservedProofHost);
const MAX_SITES: usize = 16384;
const MAX_FRAMES: usize = 64;
const MAX_NAME_BYTES: usize = 2048;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GuestFrame {
  pub function: u32,
  /// Offsets refer to the SDK-prepared module, not an asserted raw-CODE offset map.
  pub module_offset: Option<usize>,
  pub function_offset: Option<usize>,
  pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GuestImportSite {
  pub import: &'static str,
  pub ordinal: u32,
  pub frames: Vec<GuestFrame>,
}

#[derive(Debug, Default)]
pub(crate) struct GuestImportTrace {
  pub code_sha256: Option<H256>,
  pub original_root: Option<H256>,
  pub call_data_hash: Option<H256>,
  pub compiled_module_sha256: Option<H256>,
  pub sites: Vec<GuestImportSite>,
  pub seen: u32,
  pub incomplete: bool,
  pub injected_at: Option<u32>,
  pub execution: Option<Result<(), String>>,
  pub backend_root_after_execution: Option<H256>,
  pub proof_observations: usize,
}

impl GuestImportTrace {
  /// Completeness only for the observed import prefix, never for all guest work/owners.
  pub fn has_complete_import_prefix(&self) -> bool {
    self.code_sha256.is_some()
      && self.original_root.is_some()
      && self.call_data_hash.is_some()
      && self.compiled_module_sha256.is_some()
      && self.execution.is_some()
      && !self.incomplete
      && self.seen > 0
      && usize::try_from(self.seen).ok() == Some(self.sites.len())
  }
}

struct Capture {
  limit: usize,
  interrupt_at: Option<u32>,
  active: bool,
  module_image: Option<std::ops::Range<*const u8>>,
  trace: GuestImportTrace,
}

thread_local! {
  static CAPTURE: RefCell<Option<Capture>> = const { RefCell::new(None) };
}

pub(super) fn capture(
  limit: usize,
  interrupt_at: Option<u32>,
  call: impl FnOnce() -> Result<VerifiedWasmReplay, String>,
) -> (Result<VerifiedWasmReplay, String>, GuestImportTrace) {
  assert!((1..=MAX_SITES).contains(&limit));
  assert!(interrupt_at.is_none_or(|n| n > 0 && n as usize <= limit));
  CAPTURE.with_borrow_mut(|capture| {
    assert!(capture.is_none(), "guest import capture cannot nest");
    *capture = Some(Capture {
      limit,
      interrupt_at,
      active: false,
      module_image: None,
      trace: GuestImportTrace::default(),
    });
  });
  let result = call();
  let trace = CAPTURE.with_borrow_mut(|capture| capture.take().unwrap().trace);
  (result, trace)
}

pub(super) fn begin(wasm: &[u8], root: H256, call_data: &[u8]) {
  CAPTURE.with_borrow_mut(|capture| {
    if let Some(capture) = capture {
      assert!(!capture.active && capture.trace.execution.is_none());
      capture.trace.code_sha256 = Some(H256::from(sp_io::hashing::sha2_256(wasm)));
      capture.trace.original_root = Some(root);
      capture.trace.call_data_hash = Some(H256::from(sp_io::hashing::blake2_256(call_data)));
      capture.active = true;
    }
  });
}

pub(super) fn finish(result: Result<(), String>, root: H256, proof_observations: usize) {
  CAPTURE.with_borrow_mut(|capture| {
    if let Some(capture) = capture {
      assert!(capture.active);
      capture.active = false;
      capture.trace.execution = Some(result);
      capture.trace.backend_root_after_execution = Some(root);
      capture.trace.proof_observations = proof_observations;
    }
  });
}

fn record_site<T: 'static>(caller: &wasmtime::Caller<'_, T>, import: &'static str) -> bool {
  if !CAPTURE.with_borrow(|capture| capture.as_ref().is_some_and(|capture| capture.active)) {
    return false;
  }
  let backtrace = wasmtime::WasmBacktrace::force_capture(caller);
  CAPTURE.with_borrow_mut(|capture| {
    let capture = capture.as_mut().unwrap();
    let trace = &mut capture.trace;
    trace.seen = match trace.seen.checked_add(1) {
      Some(seen) => seen,
      None => {
        trace.incomplete = true;
        return false;
      }
    };
    if trace.sites.len() < capture.limit {
      let source = backtrace.frames();
      trace.incomplete |= source.is_empty() || source.len() > MAX_FRAMES;
      if let Some(root) = source.last() {
        let image = root.module().image_range();
        trace.incomplete |= root.func_name() != Some("Core_execute_block");
        trace.incomplete |= source
          .iter()
          .any(|frame| frame.module().image_range() != image);
        if let Some(expected) = &capture.module_image {
          trace.incomplete |= *expected != image;
        } else {
          capture.module_image = Some(image);
          match root.module().serialize() {
            Ok(bytes) => {
              trace.compiled_module_sha256 = Some(H256::from(sp_io::hashing::sha2_256(&bytes)))
            }
            Err(_) => trace.incomplete = true,
          }
        }
      }
      let frames = source
        .iter()
        .take(MAX_FRAMES)
        .map(|frame| {
          trace.incomplete |= frame.module_offset().is_none() || frame.func_offset().is_none();
          let name = match frame.func_name() {
            Some(name) if name.len() <= MAX_NAME_BYTES => Some(name.to_owned()),
            Some(_) => {
              trace.incomplete = true;
              None
            }
            None => None,
          };
          GuestFrame {
            function: frame.func_index(),
            module_offset: frame.module_offset(),
            function_offset: frame.func_offset(),
            name,
          }
        })
        .collect();
      trace.sites.push(GuestImportSite {
        import,
        ordinal: trace.seen,
        frames,
      });
    } else {
      trace.incomplete = true;
    }
    if capture.interrupt_at == Some(trace.seen) {
      trace.injected_at = Some(trace.seen);
      true
    } else {
      false
    }
  })
}

struct WithoutStorageSet<'a, T>(&'a mut T);
impl<T: HostFunctionRegistry> HostFunctionRegistry for WithoutStorageSet<'_, T> {
  type State = T::State;
  type Error = T::Error;
  type FunctionContext = T::FunctionContext;
  fn with_function_context<R>(
    caller: wasmtime::Caller<'_, Self::State>,
    callback: impl FnOnce(&mut dyn sc_executor::sp_wasm_interface::FunctionContext) -> R,
  ) -> R {
    T::with_function_context(caller, callback)
  }
  fn register_static<Params, Results>(
    &mut self,
    name: &str,
    function: impl wasmtime::IntoFunc<Self::State, Params, Results> + 'static,
  ) -> Result<(), Self::Error> {
    if name == "ext_storage_set_version_1" {
      Ok(())
    } else {
      self.0.register_static(name, function)
    }
  }
}

// Match the pinned runtime-interface generator's static host wrapper. Dynamic
// Function::execute alone does not convert delegated host panics into traps.
fn with_original_host_error_conversion<R>(
  call: impl FnOnce() -> Result<R, String>,
) -> anyhow::Result<R> {
  match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    call().map_err(anyhow::Error::msg)
  })) {
    Ok(result) => result,
    Err(panic) => {
      let message = if let Some(message) = panic.downcast_ref::<String>() {
        format!("host code panicked while being called by the runtime: {message}")
      } else if let Some(message) = panic.downcast_ref::<&'static str>() {
        format!("host code panicked while being called by the runtime: {message}")
      } else {
        "host code panicked while being called by the runtime".to_owned()
      };
      Err(anyhow::Error::msg(message))
    }
  }
}

#[test]
fn delegated_host_panics_match_sdk_error_conversion() {
  assert_eq!(with_original_host_error_conversion(|| Ok(42)).unwrap(), 42);
  assert_eq!(
    with_original_host_error_conversion::<()>(|| Err("ordinary host error".to_owned()))
      .unwrap_err()
      .to_string(),
    "ordinary host error"
  );
  for (panic, expected) in [
    (
      Box::new("borrowed") as Box<dyn std::any::Any + Send>,
      "host code panicked while being called by the runtime: borrowed",
    ),
    (
      Box::new("owned".to_owned()),
      "host code panicked while being called by the runtime: owned",
    ),
    (
      Box::new(17_u8),
      "host code panicked while being called by the runtime",
    ),
  ] {
    assert_eq!(
      with_original_host_error_conversion::<()>(|| std::panic::resume_unwind(panic))
        .unwrap_err()
        .to_string(),
      expected
    );
  }
}

pub(super) struct ObservedSubstrateHost;
impl HostFunctions for ObservedSubstrateHost {
  fn host_functions() -> Vec<&'static dyn sc_executor::sp_wasm_interface::Function> {
    sp_io::SubstrateHostFunctions::host_functions()
  }
  fn register_static<T: HostFunctionRegistry>(registry: &mut T) -> Result<(), T::Error> {
    sp_io::SubstrateHostFunctions::register_static(&mut WithoutStorageSet(registry))?;
    let original = sp_io::SubstrateHostFunctions::host_functions()
      .into_iter()
      .find(|function| function.name() == "ext_storage_set_version_1")
      .unwrap();
    registry.register_static(
      original.name(),
      move |caller: wasmtime::Caller<'_, T::State>, key: i64, value: i64| -> anyhow::Result<()> {
        if record_site(&caller, "ext_storage_set_version_1") {
          return Err(anyhow::anyhow!(
            "DEV_GUEST_IMPORT_INTERRUPT before original storage-set import"
          ));
        }
        let result = T::with_function_context(caller, |context| {
          with_original_host_error_conversion(|| {
            original.execute(
              context,
              &mut [Value::I64(key), Value::I64(value)].into_iter(),
            )
          })
        })?;
        match result {
          None => Ok(()),
          _ => Err(anyhow::anyhow!("storage-set host ABI changed")),
        }
      },
    )
  }
}

pub(super) struct ObservedProofHost;
impl HostFunctions for ObservedProofHost {
  fn host_functions() -> Vec<&'static dyn sc_executor::sp_wasm_interface::Function> {
    OriginalProofHost::host_functions()
  }

  fn register_static<T: HostFunctionRegistry>(registry: &mut T) -> Result<(), T::Error> {
    let functions = OriginalProofHost::host_functions();
    assert_eq!(functions.len(), 1);
    let original = functions[0];
    assert_eq!(
      original.name(),
      "ext_storage_proof_size_storage_proof_size_version_1"
    );
    registry.register_static(
      original.name(),
      move |caller: wasmtime::Caller<'_, T::State>| -> anyhow::Result<i64> {
        if record_site(
          &caller,
          "ext_storage_proof_size_storage_proof_size_version_1",
        ) {
          return Err(anyhow::anyhow!(
            "DEV_GUEST_IMPORT_INTERRUPT before original proof-size import"
          ));
        }
        let value = T::with_function_context(caller, |context| {
          with_original_host_error_conversion(|| original.execute(context, &mut std::iter::empty()))
        })?;
        match value {
          Some(Value::I64(value)) => Ok(value),
          _ => Err(anyhow::anyhow!("proof-size host ABI changed")),
        }
      },
    )
  }
}
