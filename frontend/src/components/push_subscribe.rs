use dioxus::prelude::*;
use wasm_bindgen::prelude::*;
use web_sys::window;

use crate::http;

pub async fn request_push_subscription(token: &str) -> Result<(), String> {
    let win = window().ok_or("no window")?;
    let nav_val =
        js_sys::Reflect::get(&win, &JsValue::from_str("navigator")).map_err(|_| "no navigator")?;
    let sw_container = js_sys::Reflect::get(&nav_val, &JsValue::from_str("serviceWorker"))
        .map_err(|_| "no serviceWorker")?;
    let register_fn = js_sys::Reflect::get(&sw_container, &JsValue::from_str("register"))
        .map_err(|_| "no register")?;
    let register_fn = register_fn
        .dyn_into::<js_sys::Function>()
        .map_err(|_| "not a fn")?;

    let reg_promise = register_fn
        .call1(&JsValue::undefined(), &JsValue::from_str("/sw.js"))
        .map_err(|e| format!("register: {e:?}"))?;

    wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(reg_promise))
        .await
        .map_err(|e| format!("sw: {e:?}"))?;

    let perm = js_sys::Reflect::get(&win, &JsValue::from_str("Notification"))
        .and_then(|n| js_sys::Reflect::get(&n, &JsValue::from_str("requestPermission")));

    let Ok(perm_fn) = perm else {
        return Err("Notification API not available".into());
    };
    let perm_fn = perm_fn
        .dyn_into::<js_sys::Function>()
        .map_err(|_| "not a function")?;
    let perm_promise = perm_fn
        .call0(&JsValue::undefined())
        .map_err(|e| format!("perm: {e:?}"))?;
    let perm_val = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(perm_promise))
        .await
        .map_err(|e| format!("perm: {e:?}"))?;

    if perm_val.as_string().as_deref() != Some("granted") {
        return Err("Permission denied".into());
    }

    let push_manager = js_sys::Reflect::get(&sw_container, &JsValue::from_str("pushManager"))
        .map_err(|_| "no pushManager")?;

    let subscribe_fn = js_sys::Reflect::get(&push_manager, &JsValue::from_str("subscribe"))
        .map_err(|_| "no subscribe")?;
    let subscribe_fn = subscribe_fn
        .dyn_into::<js_sys::Function>()
        .map_err(|_| "not a function")?;

    let opts = serde_wasm_bindgen::to_value(&serde_json::json!({
        "userVisibleOnly": true,
        "applicationServerKey": std::env::var("VAPID_PUBLIC_KEY").unwrap_or_default(),
    }))
    .map_err(|e| e.to_string())?;

    let sub_promise = subscribe_fn
        .call1(&JsValue::undefined(), &opts)
        .map_err(|e| format!("subscribe: {e:?}"))?;

    let sub_val = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(sub_promise))
        .await
        .map_err(|e| format!("sub: {e:?}"))?;

    let to_json_fn =
        js_sys::Reflect::get(&sub_val, &JsValue::from_str("toJSON")).map_err(|_| "no toJSON")?;
    let to_json_fn = to_json_fn
        .dyn_into::<js_sys::Function>()
        .map_err(|_| "not a function")?;
    let json_val = to_json_fn
        .call0(&sub_val)
        .map_err(|e| format!("toJSON: {e:?}"))?;

    let sub_json: serde_json::Value =
        serde_wasm_bindgen::from_value(json_val).map_err(|e| format!("parse: {e}"))?;

    http::api_post("/api/push/subscribe", &sub_json, Some(token)).await?;
    Ok(())
}

#[component]
pub fn PushSubscribe() -> Element {
    let mut subscribed = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let subscribe = move |_| {
        spawn(async move {
            if let Some(token) = crate::storage::get_token() {
                match request_push_subscription(&token).await {
                    Ok(()) => {
                        subscribed.set(true);
                        error.set(None);
                    }
                    Err(e) => error.set(Some(e)),
                }
            }
        });
    };

    rsx! {
        div { class: "card",
            if subscribed() {
                p { class: "text-sm", "Notificaciones push activadas" }
            } else {
                button { onclick: subscribe, "Activar notificaciones" }
            }
            if let Some(msg) = error() {
                p { class: "text-sm", style: "color: var(--danger)", "{msg}" }
            }
        }
    }
}
