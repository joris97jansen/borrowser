//! Qualified, thread-local allocator instrumentation; never engine evidence.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
thread_local! {
    pub(super) static REFUSE_SESSION: Cell<bool> = const { Cell::new(false) };
    pub(super) static REFUSE_OUTPUT: Cell<bool> = const { Cell::new(false) };
    static REFUSE: Cell<bool> = const { Cell::new(false) };
    static WORK: Cell<Option<[usize; 3]>> = const { Cell::new(None) };
}
pub(super) fn before_session_reservation() {
    REFUSE_SESSION.with(|v| {
        if v.replace(false) {
            REFUSE.with(|r| r.set(true));
        }
    });
}
pub(super) fn before_output_reservation() {
    REFUSE_OUTPUT.with(|v| {
        if v.replace(false) {
            REFUSE.with(|r| r.set(true));
        }
    });
}
struct Allocator;
fn refused() -> bool {
    REFUSE.try_with(|v| v.replace(false)).unwrap_or(false)
}
fn record(n: usize, realloc: bool) {
    let _ = WORK.try_with(|v| {
        if let Some(mut c) = v.get() {
            c[usize::from(realloc) * 2] += 1;
            c[1] += n;
            v.set(Some(c));
        }
    });
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if refused() {
            return std::ptr::null_mut();
        }
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            record(l.size(), false);
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        if refused() {
            return std::ptr::null_mut();
        }
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            record(l.size(), false);
        }
        p
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if refused() {
            return std::ptr::null_mut();
        }
        let q = unsafe { System.realloc(p, l, n) };
        if !q.is_null() {
            record(n.saturating_sub(l.size()), true);
        }
        q
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

#[test]
fn aj8_worker_decoder_allocation_and_admitted_output_refusal() {
    // This allocation measurement runs on a separate worker, independently of
    // Browser's caller-thread allocator. Input/decision/thread setup are excluded.
    let counts = std::thread::spawn(|| {
        let bytes = vec![0xff; tools::common::MAX_STYLESHEET_BYTES];
        let decision = crate::decoding::select_stylesheet_decoding(&bytes, None, None).unwrap();
        WORK.with(|v| v.set(Some([0; 3])));
        let decoded = crate::decoding::decode_stylesheet(&bytes, decision).unwrap();
        let counts = WORK.with(|v| v.replace(None).unwrap());
        assert_eq!(decoded.text().unwrap().len(), 3 * bytes.len());
        counts
    })
    .join()
    .unwrap();
    assert_eq!(counts, [1, 3 * tools::common::MAX_STYLESHEET_BYTES, 0]);
    let (tx, rx) = std::sync::mpsc::channel();
    let (events, received) = std::sync::mpsc::channel();
    tx.send(super::tests::chunk(1, b"abc")).unwrap();
    tx.send(super::tests::done(1)).unwrap();
    drop(tx);
    REFUSE_OUTPUT.with(|v| v.set(true));
    crate::run(rx, events);
    let events: Vec<_> = received.into_iter().collect();
    assert!(
        matches!(&events[0],bus::CoreEvent::CssDecodedBlock{decoded,..} if decoded.text()==Err(bus::css_decode::CssDecodeFailure::OutputReservation) && decoded.report()==bus::css_decode::CssDecodeReport::NotStarted)
    );
    assert!(matches!(&events[1], bus::CoreEvent::CssSheetDone { .. }));
    assert_eq!(events.len(), 2);
    println!(
        "worker_decode_allocations=1 bytes={} reallocations=0 parameter_rows=2",
        counts[1]
    );
}

#[test]
fn aj8_selection_and_report_validation_allocate_nothing() {
    // Only these two production regions are measured, independently of decode,
    // fixture construction, thread startup and Browser's allocator.
    std::thread::spawn(|| {
        for body in [
            b"".as_slice(),
            b"\xef\xbb\xbfp{}".as_slice(),
            b"p{\xff}\xe2\x82".as_slice(),
        ] {
            WORK.with(|v| v.set(Some([0; 3])));
            let decision = crate::decoding::select_stylesheet_decoding(body, Some("UTF-8"), None);
            let selection = WORK.with(|v| v.replace(None).unwrap());
            assert_eq!(selection, [0; 3]);
            let decision = decision.unwrap();
            let decoded = crate::decoding::decode_stylesheet(body, decision).unwrap();
            WORK.with(|v| v.set(Some([0; 3])));
            let valid = crate::decoding::validate_decoded_stylesheet(&decision, body, &decoded);
            let validation = WORK.with(|v| v.replace(None).unwrap());
            assert_eq!(validation, [0; 3]);
            valid.unwrap();
        }
    })
    .join()
    .unwrap();
    println!("AJ8 selection validation rows=3");
}
