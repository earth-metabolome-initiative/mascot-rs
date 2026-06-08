//! Re-export of the shared graph-building core, so the app can keep referring to
//! `crate::similarity::*`. The implementation lives in `mascot-web-core` so the
//! same code runs in the Web Worker (see [`crate::worker`]).

pub use mascot_web_core::similarity::*;
