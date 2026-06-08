//! Dedicated wasm Web Worker that builds similarity graphs off the main thread.
//!
//! Built to wasm by the app's `build.rs` and loaded as a module worker. On
//! start it installs a message loop on the worker scope: each `Build` request
//! parses the MGF and builds the graph, then posts the result back. All logic is
//! Rust; the only generated glue is wasm-bindgen's own output.

#[cfg(target_arch = "wasm32")]
mod worker {
    use js_sys::global;
    use mascot_web_core::message::{WorkerRequest, WorkerResponse};
    use mascot_web_core::{parse_mgf, similarity::build_graph};
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::{DedicatedWorkerGlobalScope, MessageEvent};

    /// Worker entry point: installs the message loop and signals readiness.
    #[wasm_bindgen(start)]
    pub fn start() {
        let on_message = Closure::wrap(Box::new(move |event: MessageEvent| {
            let Ok(request) = serde_wasm_bindgen::from_value::<WorkerRequest>(event.data()) else {
                return;
            };
            match request {
                WorkerRequest::Build { id, text, params } => {
                    let (records, _skipped) = parse_mgf(&text);
                    let response = match build_graph(records.as_ref(), &params) {
                        Ok(graph) => WorkerResponse::Built { id, graph },
                        Err(message) => WorkerResponse::Error { id, message },
                    };
                    post(&response);
                }
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        worker_scope().set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();
        post(&WorkerResponse::Ready);
    }

    /// Serializes and posts a response back to the main thread.
    fn post(response: &WorkerResponse) {
        if let Ok(value) = serde_wasm_bindgen::to_value(response) {
            let _ = worker_scope().post_message(&value);
        }
    }

    /// The dedicated worker's global scope.
    fn worker_scope() -> DedicatedWorkerGlobalScope {
        global().unchecked_into::<DedicatedWorkerGlobalScope>()
    }
}
