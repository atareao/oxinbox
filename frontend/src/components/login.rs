use dioxus::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::window;

use crate::http;
use crate::storage;

#[wasm_bindgen]
extern "C" {
    fn isUserVerifyingPlatformAuthenticatorAvailable() -> js_sys::Promise;
}

async fn auth_available() -> bool {
    let promise = isUserVerifyingPlatformAuthenticatorAvailable();
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

async fn webauthn_create(challenge: &serde_json::Value) -> Result<serde_json::Value, String> {
    let options = serde_wasm_bindgen::to_value(challenge).map_err(|e| format!("serialize: {e}"))?;

    let nav = js_sys::Reflect::get(&window().unwrap(), &JsValue::from_str("navigator"))
        .map_err(|_| "no navigator".to_string())?;
    let creds = js_sys::Reflect::get(&nav, &JsValue::from_str("credentials"))
        .map_err(|_| "no credentials".to_string())?;
    let create_fn = js_sys::Reflect::get(&creds, &JsValue::from_str("create"))
        .map_err(|_| "no create".to_string())?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| "not a function".to_string())?;

    let promise = create_fn
        .call1(&JsValue::null(), &options)
        .map_err(|e| format!("create call: {e:?}"))?;

    let cred = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise))
        .await
        .map_err(|e| format!("webauthn: {e:?}"))?;

    serde_wasm_bindgen::from_value::<serde_json::Value>(cred)
        .map_err(|e| format!("cred parse: {e}"))
}

async fn webauthn_get(challenge: &serde_json::Value) -> Result<serde_json::Value, String> {
    let options = serde_wasm_bindgen::to_value(challenge).map_err(|e| format!("serialize: {e}"))?;

    let nav = js_sys::Reflect::get(&window().unwrap(), &JsValue::from_str("navigator"))
        .map_err(|_| "no navigator".to_string())?;
    let creds = js_sys::Reflect::get(&nav, &JsValue::from_str("credentials"))
        .map_err(|_| "no credentials".to_string())?;
    let get_fn = js_sys::Reflect::get(&creds, &JsValue::from_str("get"))
        .map_err(|_| "no get".to_string())?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| "not a function".to_string())?;

    let promise = get_fn
        .call1(&JsValue::null(), &options)
        .map_err(|e| format!("get call: {e:?}"))?;

    let cred = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise))
        .await
        .map_err(|e| format!("webauthn: {e:?}"))?;

    serde_wasm_bindgen::from_value::<serde_json::Value>(cred)
        .map_err(|e| format!("cred parse: {e}"))
}

async fn register(email: &str) -> Result<String, String> {
    let start = http::api_post(
        "/auth/register/start",
        &serde_json::json!({"email": email}),
        None,
    )
    .await?;
    let challenge = start.get("challenge").ok_or("no challenge")?.clone();
    let state_id = start
        .get("state_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let credential = webauthn_create(&challenge).await?;

    let finish = http::api_post(
        "/auth/register/finish",
        &serde_json::json!({
            "state_id": state_id,
            "credential": credential,
        }),
        None,
    )
    .await?;

    finish
        .get("token")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| "no token".into())
}

async fn login(email: &str) -> Result<String, String> {
    let start = http::api_post(
        "/auth/login/start",
        &serde_json::json!({"email": email}),
        None,
    )
    .await?;
    let challenge = start.get("challenge").ok_or("no challenge")?.clone();
    let state_id = start
        .get("state_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let credential = webauthn_get(&challenge).await?;

    let finish = http::api_post(
        "/auth/login/finish",
        &serde_json::json!({
            "state_id": state_id,
            "credential": credential,
        }),
        None,
    )
    .await?;

    finish
        .get("token")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| "no token".into())
}

#[component]
pub fn LoginButton(on_login: Option<EventHandler<String>>) -> Element {
    let mut email = use_signal(|| "user@oxinbox.app".to_string());
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let onclick = move |_| {
        spawn(async move {
            loading.set(true);
            error.set(None);

            if !auth_available().await {
                error.set(Some("WebAuthn no disponible".into()));
                loading.set(false);
                return;
            }

            let email_val = email.read().trim().to_string();
            if email_val.is_empty() {
                error.set(Some("Introduce un email".into()));
                loading.set(false);
                return;
            }

            if let Ok(token) = register(&email_val).await {
                storage::set_token(&token);
                if let Some(cb) = &on_login {
                    cb.call(token);
                }
            } else if let Ok(token) = login(&email_val).await {
                storage::set_token(&token);
                if let Some(cb) = &on_login {
                    cb.call(token);
                }
            } else {
                error.set(Some("Autenticación fallida".into()));
            }
            loading.set(false);
        });
    };

    rsx! {
        div { class: "flex flex-col gap-2",
            input {
                class: "input",
                placeholder: "tu@email.com",
                value: email(),
                oninput: move |e| email.set(e.value()),
            }
            button { onclick, disabled: loading(),
                if loading() { "Conectando..." } else { "Comenzar con Passkey" }
            }
            if let Some(msg) = error() {
                p { class: "text-sm", style: "color: var(--danger)", "{msg}" }
            }
        }
    }
}
