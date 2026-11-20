use super::{TriKlopsApp, decode_reference};
use crate::web::{RunRequest, WorkerMessage};
use eframe::egui;
use image::{DynamicImage, ImageReader};
use js_sys::{Promise, Uint8Array};
use std::{cell::RefCell, io::Cursor, path::PathBuf, rc::Rc};
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{ErrorEvent, File, MessageEvent, WorkerOptions, WorkerType};

#[wasm_bindgen(module = "/web/browser.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = pickReference)]
    fn pick_reference() -> Result<Promise, JsValue>;

    #[wasm_bindgen(catch, js_name = downloadFile)]
    fn download_file(name: &str, mime_type: &str, contents: &JsValue) -> Result<(), JsValue>;

    #[wasm_bindgen(js_name = renderJpg)]
    fn render_jpg(svg: &str) -> Promise;
}

type PickedImage = Result<Option<(PathBuf, DynamicImage)>, String>;

struct CompletedDrawing {
    svg: String,
    jpg: Result<JsValue, JsValue>,
}

pub(super) enum DownloadFormat {
    Svg,
    Jpg,
}

#[derive(Default)]
pub(super) struct Browser {
    picker: Option<Rc<RefCell<Option<PickedImage>>>>,
    output: Option<CompletedDrawing>,
}

impl Browser {
    pub(super) fn is_loading(&self) -> bool {
        self.picker.is_some()
    }

    pub(super) fn has_output(&self) -> bool {
        self.output.is_some()
    }
}

pub(super) struct Worker {
    handle: web_sys::Worker,
    result: Rc<RefCell<Option<Result<CompletedDrawing, String>>>>,
    _message: Closure<dyn FnMut(MessageEvent)>,
    _error: Closure<dyn FnMut(ErrorEvent)>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.handle.set_onmessage(None);
        self.handle.set_onerror(None);
        self.handle.terminate();
    }
}

impl TriKlopsApp {
    pub(super) fn load_reference(&mut self, ctx: &egui::Context) {
        let promise = match pick_reference() {
            Ok(promise) => promise,
            Err(error) => {
                self.error = Some(format!("Could not open the file picker: {error:?}"));
                return;
            }
        };
        let picked = Rc::new(RefCell::new(None));
        self.browser.picker = Some(picked.clone());
        let ctx = ctx.clone();
        spawn_local(async move {
            *picked.borrow_mut() = Some(read_reference(promise).await);
            ctx.request_repaint();
        });
    }

    pub(super) fn start_worker(&mut self, ctx: egui::Context) {
        self.browser.output = None;
        match self.launch_worker(ctx) {
            Ok(worker) => self.worker = Some(worker),
            Err(error) => self.error = Some(format!("Could not start image generation: {error:?}")),
        }
    }

    fn launch_worker(&self, ctx: egui::Context) -> Result<Worker, JsValue> {
        let reference = self.reference.as_ref().unwrap();
        let request = serde_wasm_bindgen::to_value(&RunRequest {
            params: self.params.clone(),
            dimensions: reference.image.dimensions(),
            reference: reference.image.as_raw().clone(),
        })?;
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        let handle = web_sys::Worker::new_with_options("./worker.js", &options)?;
        let result = Rc::new(RefCell::new(None));

        let progress = self.progress.clone();
        let message = Closure::new({
            let result = result.clone();
            let ctx = ctx.clone();
            move |event: MessageEvent| {
                match serde_wasm_bindgen::from_value(event.data()) {
                    Ok(WorkerMessage::Preview(preview)) => {
                        let mut progress = progress.lock().unwrap();
                        if let Some(drawing) = preview.drawing {
                            progress.drawing = Some(drawing);
                        }
                        progress.candidate = preview.candidate;
                        progress.triangle_index = preview.triangle_index;
                        progress.triangle_count = preview.triangle_count;
                        progress.generation_index = preview.generation_index;
                        progress.current_fitness = preview.fitness;
                        progress.revision = preview.revision;
                    }
                    Ok(WorkerMessage::Done { svg }) => {
                        let result = result.clone();
                        let ctx = ctx.clone();
                        spawn_local(async move {
                            // Prepare the JPG before the click so the download can start synchronously.
                            let jpg = JsFuture::from(render_jpg(&svg)).await;
                            *result.borrow_mut() = Some(Ok(CompletedDrawing { svg, jpg }));
                            ctx.request_repaint();
                        });
                    }
                    Ok(WorkerMessage::Error { message }) => {
                        *result.borrow_mut() = Some(Err(message));
                    }
                    Err(error) => *result.borrow_mut() = Some(Err(error.to_string())),
                }
                ctx.request_repaint();
            }
        });
        let error = Closure::new({
            let result = result.clone();
            move |event: ErrorEvent| {
                *result.borrow_mut() = Some(Err(event.message()));
                ctx.request_repaint();
            }
        });
        handle.set_onmessage(Some(message.as_ref().unchecked_ref()));
        handle.set_onerror(Some(error.as_ref().unchecked_ref()));
        let worker = Worker {
            handle,
            result,
            _message: message,
            _error: error,
        };
        worker.handle.post_message(&request)?;
        Ok(worker)
    }

    pub(super) fn stop_worker(&mut self) {
        let worker = self.worker.as_ref().unwrap();
        if let Err(error) = worker.handle.post_message(&JsValue::from_str("stop")) {
            self.error = Some(format!("Could not stop image generation: {error:?}"));
        }
    }

    pub(super) fn poll_platform(&mut self) {
        let picked = self
            .browser
            .picker
            .as_ref()
            .and_then(|p| p.borrow_mut().take());
        if let Some(picked) = picked {
            self.browser.picker = None;
            match picked {
                Ok(Some((path, image))) => {
                    self.browser.output = None;
                    self.set_reference(path, image);
                }
                Ok(None) => {}
                Err(error) => self.error = Some(error),
            }
        }
        let result = self
            .worker
            .as_ref()
            .and_then(|worker| worker.result.borrow_mut().take());
        if let Some(result) = result {
            self.worker = None;
            match result {
                Ok(output) => self.browser.output = Some(output),
                Err(error) => self.error = Some(format!("Image generation failed: {error}")),
            }
        }
    }

    pub(super) fn download_output(&mut self, format: DownloadFormat) {
        let output = self.browser.output.as_ref().unwrap();
        let (extension, mime_type, contents) = match format {
            DownloadFormat::Svg => ("svg", "image/svg+xml", JsValue::from_str(&output.svg)),
            DownloadFormat::Jpg => match &output.jpg {
                Ok(jpg) => ("jpg", "image/jpeg", jpg.clone()),
                Err(error) => {
                    self.error = Some(format!("Could not render JPG: {error:?}"));
                    return;
                }
            },
        };
        let path = self
            .reference
            .as_ref()
            .unwrap()
            .path
            .with_extension(extension);
        self.error = download_file(&path.to_string_lossy(), mime_type, &contents)
            .err()
            .map(|error| format!("Could not download {}: {error:?}", extension.to_uppercase()));
    }
}

async fn read_reference(promise: Promise) -> PickedImage {
    let value = JsFuture::from(promise)
        .await
        .map_err(|e| format!("Could not open the file picker: {e:?}"))?;
    let Some(file) = value.dyn_into::<File>().ok() else {
        return Ok(None);
    };
    let name = file.name();
    let buffer = JsFuture::from(file.array_buffer())
        .await
        .map_err(|e| format!("Could not read {name}: {e:?}"))?;
    let bytes = Uint8Array::new(&buffer).to_vec();
    let image = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(image::ImageError::from)
        .and_then(decode_reference)
        .map_err(|e| format!("Could not load {name}: {e}"))?;
    Ok(Some((PathBuf::from(name), image)))
}
