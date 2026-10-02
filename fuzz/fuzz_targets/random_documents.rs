//! Fuzzing harness for arbitrary MGF document parsing.
#![no_main]

use mascot_rs::prelude::*;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|document: &str| {
    let _ = document.parse::<MGFVec<f32>>();
});
