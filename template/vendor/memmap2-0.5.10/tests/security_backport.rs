#[test]
fn mapped_range_rejects_overflow_and_out_of_bounds_before_offset() {
    let m = memmap2::MmapMut::map_anon(4096).unwrap();
    for (offset, len) in [(usize::MAX, 1), (1, usize::MAX), (4097, 0), (4096, 1), (4095, 2)] {
        assert_eq!(m.flush_range(offset, len).unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
        assert_eq!(m.flush_async_range(offset, len).unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
        #[cfg(unix)]
        assert_eq!(m.advise_range(memmap2::Advice::Normal, offset, len).unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    }
    assert!(m.flush_range(4096, 0).is_ok());
    assert!(m.flush_range(0, 4096).is_ok());
    assert!(m.flush_async_range(4095, 1).is_ok());
    let m = m.make_read_only().unwrap();
    #[cfg(unix)]
    for (offset, len) in [(usize::MAX, 1), (1, usize::MAX), (4097, 0), (4096, 1)] {
        assert_eq!(m.advise_range(memmap2::Advice::Normal, offset, len).unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    }
}
