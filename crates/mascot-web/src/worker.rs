//! Main-thread handle to the graph-building Web Worker.
//!
//! The heavy build (parse, FLASH neighbour search, Louvain and Leiden, and the
//! ForceAtlas2 layout) runs in `mascot-web-worker`, compiled to wasm by
//! `build.rs` and bundled as manganis assets. The worker is spawned from a
//! generated loader (`/assets/generated/worker-loader.js`) that imports the
//! glue and wasm by the hashed URLs it receives. Requests and responses are
//! serde types from `mascot-web-core`, passed via `postMessage`. The main thread
//! keeps its own parsed records for the panels and export, so only the build is
//! offloaded.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use dioxus::prelude::*;
use futures::channel::oneshot;
use futures::future::{FutureExt, Shared};
use js_sys::{Object, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{ErrorEvent, MessageEvent, Worker, WorkerOptions, WorkerType};

use mascot_web_core::message::{WorkerRequest, WorkerResponse};
use mascot_web_core::similarity::{GraphParams, SimilarityGraph};

/// Resolved once the worker has posted `Ready`.
type Ready = Shared<oneshot::Receiver<()>>;

/// Pending build requests, keyed by id, awaiting their worker response.
type Pending = Rc<RefCell<HashMap<u64, oneshot::Sender<Result<SimilarityGraph, String>>>>>;

/// Set to the error message once the worker has died, so later builds fail fast.
type Failed = Rc<RefCell<Option<String>>>;

/// A cloneable handle to the graph-building worker, suitable for context.
#[derive(Clone)]
pub struct WorkerClient {
    worker: Worker,
    pending: Pending,
    next_id: Rc<Cell<u64>>,
    ready: Ready,
    failed: Failed,
}

impl WorkerClient {
    /// Spawns the worker. Returns `None` if the browser cannot create it, in
    /// which case the caller falls back to building on the main thread.
    #[must_use]
    pub fn spawn() -> Option<Self> {
        // All three assets are bundled by manganis (built by `build.rs`), so they
        // are served from `/assets/` with a JavaScript `Content-Type`. The loader
        // is generic; it receives the hashed glue and wasm URLs by message.
        let loader = asset!("/assets/generated/worker-loader.js").to_string();
        let glue = asset!("/assets/generated/mascot_web_worker.js").to_string();
        let wasm = asset!("/assets/generated/mascot_web_worker_bg.wasm").to_string();

        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        let worker = Worker::new_with_options(&loader, &options).ok()?;

        let pending: Pending = Rc::new(RefCell::new(HashMap::new()));
        let failed: Failed = Rc::new(RefCell::new(None));
        let (ready_tx, ready_rx) = oneshot::channel::<()>();
        let ready_tx = Rc::new(RefCell::new(Some(ready_tx)));

        let pending_for_cb = pending.clone();
        let ready_for_cb = ready_tx.clone();
        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let Ok(response) = serde_wasm_bindgen::from_value::<WorkerResponse>(event.data())
            else {
                return;
            };
            match response {
                WorkerResponse::Ready => {
                    if let Some(sender) = ready_for_cb.borrow_mut().take() {
                        let _ = sender.send(());
                    }
                }
                WorkerResponse::Built { id, graph } => {
                    if let Some(sender) = pending_for_cb.borrow_mut().remove(&id) {
                        let _ = sender.send(Ok(graph));
                    }
                }
                WorkerResponse::Error { id, message } => {
                    if let Some(sender) = pending_for_cb.borrow_mut().remove(&id) {
                        let _ = sender.send(Err(message));
                    }
                }
            }
        });
        worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        // The worker lives for the app's lifetime, so leak the callback.
        on_message.forget();

        // If the worker fails to load or crashes, it fires `error` rather than a
        // message. Record the failure, wake any build still waiting for `Ready`,
        // and fail every in-flight build so nothing hangs forever.
        let pending_for_err = pending.clone();
        let ready_for_err = ready_tx.clone();
        let failed_for_err = failed.clone();
        let on_error = Closure::<dyn FnMut(ErrorEvent)>::new(move |event: ErrorEvent| {
            let text = event.message();
            let message = if text.is_empty() {
                "graph worker stopped unexpectedly".to_string()
            } else {
                format!("graph worker error: {text}")
            };
            failed_for_err.borrow_mut().replace(message.clone());
            if let Some(sender) = ready_for_err.borrow_mut().take() {
                let _ = sender.send(());
            }
            let stranded: Vec<_> = pending_for_err
                .borrow_mut()
                .drain()
                .map(|(_, sender)| sender)
                .collect();
            for sender in stranded {
                let _ = sender.send(Err(message.clone()));
            }
        });
        worker.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        on_error.forget();

        // Tell the loader where to import the glue and wasm from. Once it inits,
        // the worker's `start` takes over the message loop and posts `Ready`.
        let init = Object::new();
        let _ = Reflect::set(&init, &"kind".into(), &"init".into());
        let _ = Reflect::set(&init, &"glue".into(), &JsValue::from_str(&glue));
        let _ = Reflect::set(&init, &"wasm".into(), &JsValue::from_str(&wasm));
        worker.post_message(&init).ok()?;

        Some(Self {
            worker,
            pending,
            next_id: Rc::new(Cell::new(0)),
            ready: ready_rx.shared(),
            failed,
        })
    }

    /// Builds a graph from `text` and `params` in the worker. Dropping the
    /// returned future (for example when `use_resource` debounces) discards the
    /// response.
    pub async fn build(&self, text: &str, params: &GraphParams) -> Result<SimilarityGraph, String> {
        // Fail fast if the worker has already died.
        if let Some(message) = self.failed.borrow().clone() {
            return Err(message);
        }
        // Don't send work until the worker has initialised.
        let _ = self.ready.clone().await;
        // The worker may have failed to load while we were waiting for `Ready`.
        if let Some(message) = self.failed.borrow().clone() {
            return Err(message);
        }

        let id = self.next_id.get();
        self.next_id.set(id.wrapping_add(1));

        let (sender, receiver) = oneshot::channel();
        self.pending.borrow_mut().insert(id, sender);

        let request = WorkerRequest::Build {
            id,
            text: text.to_string(),
            params: *params,
        };
        let value = serde_wasm_bindgen::to_value(&request).map_err(|error| error.to_string())?;
        self.worker
            .post_message(&value)
            .map_err(|_| "failed to message the worker".to_string())?;

        receiver
            .await
            .unwrap_or_else(|_| Err("worker request was dropped".to_string()))
    }
}
