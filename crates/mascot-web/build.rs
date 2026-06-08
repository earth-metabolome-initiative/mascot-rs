//! Builds the dedicated wasm Web Worker and emits its loader.
//!
//! Compiles `mascot-web-worker` to `wasm32-unknown-unknown`, runs `wasm-bindgen`
//! (web target) over it, and writes the glue, the wasm, and a generated loader
//! into `assets/generated/`. They are bundled as manganis assets (referenced via
//! `asset!` in `worker.rs`), so they are served from `/assets/` with the correct
//! `Content-Type` (the plain `public/` dir is served without one, which browsers
//! reject for worker scripts). The loader receives the hashed glue and wasm URLs
//! by message and dynamic-imports them, so nothing depends on the hashed names.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const WORKER_PACKAGE: &str = "mascot-web-worker";
const WORKER_STEM: &str = "mascot_web_worker";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../mascot-web-worker/src");
    println!("cargo:rerun-if-changed=../mascot-web-worker/Cargo.toml");
    println!("cargo:rerun-if-changed=../mascot-web-core/src");
    println!("cargo:rerun-if-changed=../mascot-web-core/Cargo.toml");

    // Building the worker invokes a nested cargo build, which breaks coverage
    // instrumentation; allow skipping it explicitly.
    if env::var_os("MASCOT_SKIP_WORKER_BUILD").is_some()
        || env::var_os("CARGO_CFG_COVERAGE").is_some()
    {
        println!("cargo:warning=skipping worker build");
        return;
    }

    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").ok_or("cargo did not provide CARGO_MANIFEST_DIR")?,
    );
    let workspace_root = manifest_dir
        .join("../..")
        .canonicalize()
        .map_err(|error| format!("failed to resolve workspace root: {error}"))?;
    let generated_dir = manifest_dir.join("assets/generated");
    fs::create_dir_all(&generated_dir)
        .map_err(|error| format!("failed to create generated directory: {error}"))?;

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").ok_or("cargo did not provide OUT_DIR")?);
    let bindgen_dir = out_dir.join("mascot-worker-bindgen");
    let target_dir = out_dir.join("mascot-worker-target");
    let _ = fs::remove_dir_all(&bindgen_dir);
    fs::create_dir_all(&bindgen_dir)
        .map_err(|error| format!("failed to create bindgen directory: {error}"))?;

    let cargo = env::var("CARGO").unwrap_or_else(|_| String::from("cargo"));

    // The worker is a compute kernel, so always build it optimized regardless of
    // the app profile. This also keeps the shared `assets/generated/` output
    // consistent (a debug app build would otherwise leave a huge debug worker).
    let mut build = Command::new(cargo);
    build
        .current_dir(&workspace_root)
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .args([
            "build",
            "--package",
            WORKER_PACKAGE,
            "--lib",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "--target-dir",
        ])
        .arg(&target_dir);

    let status = build
        .status()
        .map_err(|error| format!("failed to launch worker cargo build: {error}"))?;
    if !status.success() {
        return Err(format!("worker cargo build failed with status {status}"));
    }

    let worker_wasm = target_dir
        .join("wasm32-unknown-unknown")
        .join("release")
        .join(format!("{WORKER_STEM}.wasm"));
    if !worker_wasm.exists() {
        return Err(format!("expected worker wasm at {}", worker_wasm.display()));
    }

    let mut bindgen = wasm_bindgen_cli_support::Bindgen::new();
    bindgen
        .input_path(&worker_wasm)
        .out_name(WORKER_STEM)
        .typescript(false)
        .web(true)
        .map_err(|error| format!("failed to configure worker bindgen: {error}"))?
        .generate(&bindgen_dir)
        .map_err(|error| format!("worker bindgen generation failed: {error}"))?;

    copy_file(
        &bindgen_dir.join(format!("{WORKER_STEM}.js")),
        &generated_dir.join(format!("{WORKER_STEM}.js")),
    )?;
    copy_file(
        &bindgen_dir.join(format!("{WORKER_STEM}_bg.wasm")),
        &generated_dir.join(format!("{WORKER_STEM}_bg.wasm")),
    )?;
    fs::write(generated_dir.join("worker-loader.js"), loader_script())
        .map_err(|error| format!("failed to write worker loader: {error}"))?;
    Ok(())
}

fn copy_file(source: &Path, destination: &Path) -> Result<(), String> {
    fs::copy(source, destination).map(|_| ()).map_err(|error| {
        format!(
            "failed to copy {} to {}: {error}",
            source.display(),
            destination.display()
        )
    })
}

/// The generated module-worker loader. On the `init` message it dynamic-imports
/// the wasm-bindgen glue (by its hashed manganis URL) and initialises it, which
/// runs the worker's `start`. Emitted by the build, not hand-authored, and free
/// of any hashed filenames so it survives asset bundling.
fn loader_script() -> &'static str {
    "self.onmessage = async (event) => {\n\
    \x20   const data = event.data;\n\
    \x20   if (data && data.kind === \"init\") {\n\
    \x20       const wasm = await import(data.glue);\n\
    \x20       await wasm.default({ module_or_path: data.wasm });\n\
    \x20   }\n\
     };\n"
}
