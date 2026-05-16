// SPDX-License-Identifier: MIT

//! `rprof` — process resource profiler.
//!
//! The library is intentionally exposed so integration tests and other
//! consumers can reach into the schema and parsers directly.

pub mod cli;
pub mod schema;

#[cfg(target_os = "linux")]
pub mod proc_parse;

pub mod runner;
pub mod sampler;
pub mod viewer;
