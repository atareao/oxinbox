use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use oxinbox_core::Task;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Blob, MediaStream, window};

use crate::storage;

#[wasm_bindgen]
extern "C" {
    type BlobEvent;
    #[wasm_bindgen(method, getter)]
    fn data(this: &BlobEvent) -> Blob;

    type JsMediaRecorder;
    #[wasm_bindgen(constructor)]
    fn new(stream: &MediaStream) -> JsMediaRecorder;
    #[wasm_bindgen(method)]
    fn start(this: &JsMediaRecorder);
    #[wasm_bindgen(method)]
    fn stop(this: &JsMediaRecorder);
    #[wasm_bindgen(method, setter = ondataavailable)]
    fn set_ondataavailable(this: &JsMediaRecorder, callback: &js_sys::Function);
    #[wasm_bindgen(method, setter = onstop)]
    fn set_onstop(this: &JsMediaRecorder, callback: &js_sys::Function);
}

#[component]
pub fn VoiceInput(on_task: EventHandler<Task>) -> Element {
    let mut state = use_signal(|| VoiceState::Idle);
    let mut error = use_signal(|| None::<String>);
    let mut preview = use_signal(|| None::<String>);
    let recorder: Rc<RefCell<Option<JsMediaRecorder>>> = use_hook(|| Rc::new(RefCell::new(None)));

    let rec_start = recorder.clone();
    #[allow(clippy::redundant_clone)]
    let rec_stop = recorder.clone();

    let start_recording = move |_| {
        if !state.read().is_idle() {
            return;
        }
        error.set(None);
        preview.set(None);
        state.set(VoiceState::RequestingMic);

        let mut state_c = state;
        let mut error_c = error;
        let preview_c = preview;
        let on_task_c = on_task;
        let recorder_c = rec_start.clone();

        spawn(async move {
            let nav = js_sys::Reflect::get(&window().unwrap(), &JsValue::from_str("navigator"))
                .ok()
                .unwrap();
            let media_devices = js_sys::Reflect::get(&nav, &JsValue::from_str("mediaDevices"))
                .ok()
                .unwrap();
            let get_user_media =
                js_sys::Reflect::get(&media_devices, &JsValue::from_str("getUserMedia"))
                    .ok()
                    .unwrap()
                    .dyn_into::<js_sys::Function>()
                    .unwrap();

            let constraints =
                serde_wasm_bindgen::to_value(&serde_json::json!({"audio": true})).unwrap();
            let promise = get_user_media
                .call1(&JsValue::null(), &constraints)
                .unwrap();
            let stream_val = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise))
                .await
                .ok();

            let Some(stream_val) = stream_val else {
                error_c.set(Some("Micrófono no disponible".into()));
                state_c.set(VoiceState::Idle);
                return;
            };

            let stream: MediaStream = stream_val.unchecked_into();
            let r = JsMediaRecorder::new(&stream);
            *recorder_c.borrow_mut() = Some(r);
            let chunks: Rc<RefCell<Vec<Blob>>> = Rc::new(RefCell::new(Vec::new()));

            let ch = chunks.clone();
            let ondataavailable = Closure::<dyn FnMut(JsValue)>::new(move |event_val: JsValue| {
                let event = event_val.unchecked_ref::<BlobEvent>();
                let blob = event.data();
                ch.borrow_mut().push(blob);
            });
            let r_ref = recorder_c.borrow();
            let r = r_ref.as_ref().unwrap();
            r.set_ondataavailable(ondataavailable.as_ref().unchecked_ref());
            drop(r_ref);
            ondataavailable.forget();

            let mut state_c2 = state_c;
            let mut error_c2 = error_c;
            let mut preview_c2 = preview_c;
            let on_task_c2 = on_task_c;
            let onstop = Closure::<dyn FnMut()>::new(move || {
                let chunks = chunks.clone();
                spawn(async move {
                    state_c2.set(VoiceState::Processing);
                    let token = storage::get_token();
                    let Some(token) = token else {
                        state_c2.set(VoiceState::Idle);
                        return;
                    };

                    let ws_url = format!("ws://localhost:3300/api/voice?token={token}");
                    let Some(ws) = web_sys::WebSocket::new(&ws_url).ok() else {
                        state_c2.set(VoiceState::Idle);
                        return;
                    };
                    ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

                    let ws_send = ws.clone();
                    let onopen = Closure::<dyn FnMut()>::new(move || {
                        let task_chunks = chunks.borrow().clone();
                        let wss = ws_send.clone();
                        spawn(async move {
                            let merged = Blob::new_with_blob_sequence(&task_chunks.into()).unwrap();
                            let promise = merged.array_buffer();
                            let buf = wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
                            let _ = wss.send_with_array_buffer(buf.unchecked_ref());
                            let _ = wss.send_with_str(r#"{"type":"transcribe"}"#);
                        });
                    });
                    ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
                    onopen.forget();

                    let ws_msg = ws.clone();
                    let onmessage = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(
                        move |event: web_sys::MessageEvent| {
                            let text_opt = event.data().as_string();
                            let text = match text_opt {
                                Some(ref t) => t.clone(),
                                None => return,
                            };
                            let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) else {
                                return;
                            };
                            match val["type"].as_str() {
                                Some("transcription") => {
                                    if let Some(t) = val["text"].as_str() {
                                        preview_c2.set(Some(t.to_string()));
                                    }
                                }
                                Some("task") => {
                                    if let Some(task_val) = val.get("task")
                                        && let Ok(task) =
                                            serde_json::from_value::<Task>(task_val.clone())
                                    {
                                        storage::save_task(&task);
                                        on_task_c2.call(task);
                                    }
                                    let _ = ws_msg.close();
                                    state_c2.set(VoiceState::Idle);
                                }
                                Some("parse_error") => {
                                    error_c2.set(Some("Error al parsear la tarea".into()));
                                    state_c2.set(VoiceState::Idle);
                                }
                                Some("error") => {
                                    if let Some(msg) = val["message"].as_str() {
                                        error_c2.set(Some(msg.to_string()));
                                    }
                                    state_c2.set(VoiceState::Idle);
                                }
                                _ => {}
                            }
                        },
                    );
                    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
                    onmessage.forget();

                    let mut state_c3 = state_c2;
                    let onerror = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                        state_c3.set(VoiceState::Idle);
                    });
                    ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
                    onerror.forget();
                });
            });

            let r_ref2 = recorder_c.borrow();
            let r2 = r_ref2.as_ref().unwrap();
            r2.set_onstop(onstop.as_ref().unchecked_ref());
            drop(r_ref2);
            onstop.forget();

            let r_ref3 = recorder_c.borrow();
            let r3 = r_ref3.as_ref().unwrap();
            r3.start();
            drop(r_ref3);

            state_c.set(VoiceState::Recording);
        });
    };

    let stop_recording = move |_| {
        if let Some(r) = rec_stop.borrow_mut().take() {
            r.stop();
        }
        state.set(VoiceState::Stopping);
    };

    rsx! {
        div { class: "voice-input mb-3",
            if matches!(*state.read(), VoiceState::Recording | VoiceState::Stopping) {
                div { class: "flex gap-2 items-center",
                    span { class: "recording-indicator", "🔴 Grabando..." }
                    button { onclick: stop_recording, "✓ Procesar" }
                }
            } else if matches!(*state.read(), VoiceState::Processing) {
                div { class: "flex gap-2 items-center",
                    span { "⏳ Procesando..." }
                }
            } else {
                button { onclick: start_recording, "🎤 Grabar tarea por voz" }
            }
            if let Some(text) = preview() {
                p { class: "text-sm text-muted", "Transcripción: {text}" }
            }
            if let Some(msg) = error() {
                p { class: "text-sm", style: "color: var(--danger)", "{msg}" }
            }
        }
    }
}

enum VoiceState {
    Idle,
    RequestingMic,
    Recording,
    Stopping,
    Processing,
}

impl VoiceState {
    const fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }
}
