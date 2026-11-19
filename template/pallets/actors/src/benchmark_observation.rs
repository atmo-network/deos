//! Passive native benchmark/test notices. No state, execution policy or production host interface.

use std::cell::Cell;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Phase {
  PrepassBody,
  DrainBody,
  Pass,
  Encounter,
  Preflight,
  HeadAdmission {
    actor: u64,
    meter_remaining: [u64; 2],
    caller_control: Option<[u64; 2]>,
    required_control: [u64; 2],
    required_effect: [u64; 2],
  },
  Booking {
    actor: u64,
    ledger_control: [u64; 2],
    caller_control: [u64; 2],
    ledger_effect: [u64; 2],
    ledger_user: [u64; 2],
    base_effect: bool,
    required_effect: [u64; 2],
  },
  Task,
  Retirement,
  Settlement,
  Collection,
  Authentication,
}

type Observer = fn(Phase, bool);

std::thread_local! {
  static OBSERVER: Cell<Option<Observer>> = const { Cell::new(None) };
}

/// Refuses capture inside another observer, including callback-free consumers.
pub fn assert_unobserved() {
  assert!(
    OBSERVER.get().is_none(),
    "Actor observation scopes cannot nest"
  );
}

/// Installs one synchronous native consumer; unwinding cannot leave it installed.
pub fn with_observer<R>(observer: Observer, run: impl FnOnce() -> R) -> R {
  assert_unobserved();
  struct Reset;
  impl Drop for Reset {
    fn drop(&mut self) {
      OBSERVER.set(None);
    }
  }
  OBSERVER.set(Some(observer));
  let _reset = Reset;
  run()
}

pub(crate) struct Scope {
  phase: Phase,
  observer: Option<Observer>,
}

pub(crate) fn enter(phase: Phase) -> Scope {
  let observer = OBSERVER.get();
  if let Some(observer) = observer {
    observer(phase, true);
  }
  Scope { phase, observer }
}

impl Drop for Scope {
  fn drop(&mut self) {
    if let Some(observer) = self.observer {
      observer(self.phase, false);
    }
  }
}
