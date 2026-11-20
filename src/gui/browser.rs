use super::TriKlopsApp;
use crate::web::{RunRequest, WorkerMessage};
use eframe::egui;
use image::{DynamicImage, RgbImage};
use js_sys::{Promise, Uint8Array};
use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc};
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{ErrorEvent, File, MessageEvent, WorkerOptions, WorkerType};

#[wasm_bindgen(module = "/web/browser.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = pickReference)]
    fn pick_reference() -> Result<Promise, JsValue>;

    #[wasm_bindgen(catch, js_name = downloadSvg)]
    fn download_svg(name: &str, svg: &str) -> Result<(), JsValue>;
}

type PickedImage = Result<Option<(PathBuf, DynamicImage)>, String>;

#[derive(Default)]
pub(super) struct Browser {
    picker: Option<Rc<RefCell<Option<PickedImage>>>>,
    svg: Option<String>,
}

impl Browser {
    pub(super) fn is_loading(&self) -> bool {
        self.picker.is_some()
    }
}

pub(super) struct Worker {
    handle: web_sys::Worker,
    result: Rc<RefCell<Option<Result<String, String>>>>,
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
        self.browser.svg = None;
        match self.launch_worker(ctx) {
            Ok(worker) => self.worker = Some(worker),
            Err(error) => self.error = Some(format!("Could not start image generation: {error:?}")),
        }
    }

    fn launch_worker(&self, ctx: egui::Context) -> Result<Worker, JsValue> {
        let reference = self.reference.as_ref().unwrap();
        let request = serde_wasm_bindgen::to_value(&RunRequest {
            params: self.params.clone(),
            reference: reference.image.as_raw().clone(),
        })?;
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        let handle = web_sys::Worker::new_with_options("./worker.js", &options)?;
        let result = Rc::new(RefCell::new(None));

        let progress = self.progress.clone();
        let size = reference.image.width();
        let message = Closure::new({
            let result = result.clone();
            let ctx = ctx.clone();
            move |event: MessageEvent| {
                match serde_wasm_bindgen::from_value(event.data()) {
                    Ok(WorkerMessage::Preview(preview)) => {
                        let mut progress = progress.lock().unwrap();
                        if let Some(pixels) = preview.pixels {
                            progress.canvas =
                                RgbImage::from_raw(size, size, pixels.into_vec()).map(Arc::new);
                        }
                        progress.triangle_index = preview.triangle_index;
                        progress.triangle_count = preview.triangle_count;
                        progress.generation_index = preview.generation_index;
                        progress.current_fitness = preview.fitness;
                        progress.revision = preview.revision;
                    }
                    Ok(WorkerMessage::Done { svg }) => *result.borrow_mut() = Some(Ok(svg)),
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

    pub(super) fn poll_platform(&mut self, ctx: &egui::Context) {
        let picked = self
            .browser
            .picker
            .as_ref()
            .and_then(|p| p.borrow_mut().take());
        if let Some(picked) = picked {
            self.browser.picker = None;
            match picked {
                Ok(Some((path, image))) => {
                    self.browser.svg = None;
                    self.set_reference(ctx, path, image);
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
                Ok(svg) => self.browser.svg = Some(svg),
                Err(error) => self.error = Some(format!("Image generation failed: {error}")),
            }
        }
    }

    pub(super) fn download_control(&mut self, ui: &mut egui::Ui) {
        let Some(svg) = self.browser.svg.as_deref() else {
            return;
        };
        if ui
            .add(super::controls::sidebar_button(ui, "Download SVG"))
            .clicked()
        {
            let path = self.reference.as_ref().unwrap().path.with_extension("svg");
            if let Err(error) = download_svg(&path.to_string_lossy(), svg) {
                self.error = Some(format!("Could not download SVG: {error:?}"));
            }
        }
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
    let image = image::load_from_memory(&Uint8Array::new(&buffer).to_vec())
        .map_err(|e| format!("Could not load {name}: {e}"))?;
    Ok(Some((PathBuf::from(name), image)))
}
