use crate::gui::TriKlopsApp;
use image::RgbImage;
use js_sys::{Function, Promise};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use triklops::algo::{AlgorithmParams, Drawing, Progress, Triangle, approximate};
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::JsFuture;
use web_sys::{DedicatedWorkerGlobalScope, HtmlCanvasElement};

#[derive(Serialize, Deserialize)]
pub(crate) struct RunRequest {
    pub params: AlgorithmParams,
    pub dimensions: (u32, u32),
    #[serde(with = "serde_bytes")]
    pub reference: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum WorkerMessage {
    Preview(Preview),
    Done { svg: String },
    Error { message: String },
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Preview {
    pub triangle_index: usize,
    pub triangle_count: usize,
    pub generation_index: usize,
    pub fitness: f64,
    pub revision: usize,
    pub drawing: Option<Arc<Drawing>>,
    pub candidate: Option<Triangle>,
}

#[wasm_bindgen]
pub async fn start_app(canvas: HtmlCanvasElement) -> Result<(), JsValue> {
    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|_| Ok(Box::<TriKlopsApp>::default())),
        )
        .await
}

#[wasm_bindgen]
pub async fn generate(request: JsValue, check_stop: Function) -> Result<String, JsValue> {
    let request: RunRequest = serde_wasm_bindgen::from_value(request)?;
    let (width, height) = request.dimensions;
    let expected_bytes = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(3));
    if width == 0
        || height == 0
        || width.max(height) != request.params.image_size
        || expected_bytes.map(|bytes| bytes as usize) != Some(request.reference.len())
    {
        return Err(JsValue::from_str("Invalid reference dimensions."));
    }
    let reference =
        RgbImage::from_raw(width, height, request.reference).expect("validated RGB buffer");
    let worker: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let clock = worker.performance().unwrap();
    let progress = Mutex::new(Progress::default());
    let mut next_update = 0.0;
    let mut revision = 0;

    let drawing = approximate(&request.params, &reference, &progress, async |progress| {
        let now = clock.now();
        if now >= next_update {
            next_update = now + 50.0;
            // A timer gives the worker a chance to receive the Stop Processing message.
            let result = match send_preview(&worker, &progress.lock().unwrap(), &mut revision) {
                Ok(()) => check_stop.call0(&JsValue::UNDEFINED),
                Err(error) => Err(error),
            };
            let stopped = match result {
                Ok(promise) => JsFuture::from(Promise::from(promise))
                    .await
                    .map(|value| value.is_truthy())
                    .unwrap_or(true),
                Err(_) => true,
            };
            progress.lock().unwrap().should_stop = stopped;
        }
    })
    .await;

    send_preview(&worker, &progress.into_inner().unwrap(), &mut revision)?;
    Ok(drawing.to_string())
}

fn send_preview(
    worker: &DedicatedWorkerGlobalScope,
    progress: &Progress,
    revision: &mut usize,
) -> Result<(), JsValue> {
    let drawing = if progress.revision == *revision {
        None
    } else {
        *revision = progress.revision;
        progress.drawing.clone()
    };
    let message = WorkerMessage::Preview(Preview {
        triangle_index: progress.triangle_index,
        triangle_count: progress.triangle_count,
        generation_index: progress.generation_index,
        fitness: progress.current_fitness,
        revision: progress.revision,
        drawing,
        candidate: progress.candidate,
    });
    worker.post_message(&serde_wasm_bindgen::to_value(&message)?)
}
