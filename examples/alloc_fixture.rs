// SPDX-License-Identifier: MIT

//! Integration-test fixture for `run_peak_rss_matches_known_allocation_within_5pct`.
//!
//! Allocates `mb` megabytes of resident memory, then sleeps for
//! `seconds`. Built as a Cargo example so the production `rprof`
//! binary doesn't ship a test scaffold; the integration test reaches
//! it via the `alloc_fixture_bin()` helper, which builds the example
//! on demand and points at `target/<profile>/examples/alloc_fixture`.
//!
//! `vec![0u8; n]` is *not* enough on Linux: zeroed u8 allocations are
//! backed by the kernel's shared zero page (CoW), so RSS stays tiny.
//! The dirtying loop writes one non-zero byte per 4 KiB page to force
//! real page allocation.

use std::time::Duration;

fn main() {
    let mut args = std::env::args().skip(1);
    let mb: usize = args
        .next()
        .expect("usage: alloc_fixture <mb> <seconds>")
        .parse()
        .expect("mb must be a non-negative integer");
    let seconds: f64 = args
        .next()
        .expect("usage: alloc_fixture <mb> <seconds>")
        .parse()
        .expect("seconds must be a number");

    let bytes = mb.saturating_mul(1024 * 1024);
    let mut buf: Vec<u8> = vec![0u8; bytes];
    const PAGE: usize = 4096;
    let mut i = 0;
    while i < bytes {
        buf[i] = 0xa5;
        i += PAGE;
    }
    std::hint::black_box(&buf);
    std::thread::sleep(Duration::from_secs_f64(seconds.max(0.0)));
    drop(buf);
}
