extern crate lru;
use lru::LruCache;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};

#[test]
fn panicking_key_pop_detaches_before_unwind_and_later_eviction() {
    static ARMED: AtomicBool = AtomicBool::new(false);
    #[derive(PartialEq, Eq, Hash)]
    struct PanicKey(u32);
    impl Drop for PanicKey {
        fn drop(&mut self) {
            if ARMED.swap(false, Ordering::SeqCst) {
                panic!("key destructor");
            }
        }
    }
    let mut cache = LruCache::new(2);
    cache.put(PanicKey(1), 10);
    cache.put(PanicKey(2), 20);
    ARMED.store(true, Ordering::SeqCst);
    assert!(catch_unwind(AssertUnwindSafe(|| cache.pop(&PanicKey(1)))).is_err());
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.iter().map(|(k, v)| (k.0, *v)).collect::<Vec<_>>(), vec![(2, 20)]);
    cache.put(PanicKey(3), 30);
    cache.put(PanicKey(4), 40);
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.get(&PanicKey(2)), None);
    assert_eq!(cache.get(&PanicKey(3)), Some(&30));
    assert_eq!(cache.get(&PanicKey(4)), Some(&40));
}
