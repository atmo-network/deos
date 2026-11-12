use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use hickory_proto::{
    op::{Message, Query},
    rr::{Name, RecordType},
    serialize::binary::BinDecoder,
};

struct ObservedAllocator;
thread_local! {
    static MAX_ALLOCATION: Cell<Option<usize>> = const { Cell::new(None) };
}
unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        MAX_ALLOCATION.with(|max| {
            if let Some(previous) = max.get() {
                max.set(Some(previous.max(layout.size())));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        MAX_ALLOCATION.with(|max| {
            if let Some(previous) = max.get() {
                max.set(Some(previous.max(size)));
            }
        });
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

fn largest_allocation(f: impl FnOnce()) -> usize {
    MAX_ALLOCATION.with(|max| max.set(Some(0)));
    f();
    MAX_ALLOCATION.with(|max| max.replace(None).unwrap())
}

#[test]
fn security_wire_counts_do_not_preallocate_advertised_records() {
    for count in [9u16, u16::MAX] {
        for section in 0..4 {
            let mut bytes = [0u8; 12];
            bytes[4 + section * 2..6 + section * 2].copy_from_slice(&count.to_be_bytes());
            let largest = largest_allocation(|| assert!(Message::from_vec(&bytes).is_err()));
            bytes[4 + section * 2..6 + section * 2].copy_from_slice(&8u16.to_be_bytes());
            let bound = largest_allocation(|| assert!(Message::from_vec(&bytes).is_err()));
            assert_eq!(largest, bound, "section={section} count={count}");
        }
    }
    for records in [false, true] {
        let largest = largest_allocation(|| {
            let mut decoder = BinDecoder::new(&[]);
            if records {
                assert!(Message::read_records(&mut decoder, u16::MAX as usize, true).is_err());
            } else {
                assert!(Message::read_queries(&mut decoder, u16::MAX as usize).is_err());
            }
        });
        let baseline = largest_allocation(|| {
            let mut decoder = BinDecoder::new(&[]);
            if records {
                assert!(Message::read_records(&mut decoder, 8, true).is_err());
            } else {
                assert!(Message::read_queries(&mut decoder, 8).is_err());
            }
        });
        assert_eq!(largest, baseline);
    }
}

#[test]
fn security_wire_counts_allow_growth_beyond_initial_reservation() {
    for count in [0, 1, 8, 9, 121] {
        let mut message = Message::new();
        for i in 0..count {
            message.add_query(Query::query(
                Name::from_ascii(format!("n{i}.example.")).unwrap(),
                RecordType::A,
            ));
        }
        let bytes = message.to_vec().unwrap();
        assert_eq!(Message::from_vec(&bytes).unwrap().queries(), message.queries());
    }
}
