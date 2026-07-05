use serde_json::Value;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Headers, Request, RequestInit, Response, window};

pub fn api_url(path: &str) -> String {
    let port = "3300";
    format!("http://localhost:{port}{path}")
}

fn headers_with_auth(token: &str) -> Headers {
    let h = Headers::new().unwrap();
    let _ = h.set("Authorization", &format!("Bearer {token}"));
    let _ = h.set("Content-Type", "application/json");
    h
}

pub async fn api_post(path: &str, json: &Value, token: Option<&str>) -> Result<Value, String> {
    let body = serde_json::to_string(json).map_err(|e| e.to_string())?;
    #[allow(unused_mut)]
    let mut opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_body(&JsValue::from_str(&body));

    if let Some(t) = token {
        opts.set_headers(&headers_with_auth(t));
    }

    let request = Request::new_with_str_and_init(&api_url(path), &opts)
        .map_err(|e| format!("request: {e:?}"))?;

    let promise = window().unwrap().fetch_with_request(&request);
    let value = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|e| format!("fetch: {e:?}"))?;

    let resp = value
        .dyn_into::<Response>()
        .map_err(|_| "type error".to_string())?;
    let text = js_text(&resp).await?;
    if !resp.ok() {
        return Err(format!("HTTP {} {}", resp.status(), text));
    }
    serde_json::from_str(&text).map_err(|e| format!("json: {e}"))
}

pub async fn api_get(path: &str, token: &str) -> Result<Value, String> {
    #[allow(unused_mut)]
    let mut opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_headers(&headers_with_auth(token));

    let request = Request::new_with_str_and_init(&api_url(path), &opts)
        .map_err(|e| format!("request: {e:?}"))?;

    let promise = window().unwrap().fetch_with_request(&request);
    let value = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|e| format!("fetch: {e:?}"))?;

    let resp = value
        .dyn_into::<Response>()
        .map_err(|_| "type error".to_string())?;
    let text = js_text(&resp).await?;
    if !resp.ok() {
        return Err(format!("HTTP {} {}", resp.status(), text));
    }
    serde_json::from_str(&text).map_err(|e| format!("json: {e}"))
}

pub async fn api_put(path: &str, json: &Value, token: &str) -> Result<Value, String> {
    let body = serde_json::to_string(json).map_err(|e| e.to_string())?;
    #[allow(unused_mut)]
    let mut opts = RequestInit::new();
    opts.set_method("PUT");
    opts.set_headers(&headers_with_auth(token));
    opts.set_body(&JsValue::from_str(&body));

    let request = Request::new_with_str_and_init(&api_url(path), &opts)
        .map_err(|e| format!("request: {e:?}"))?;

    let promise = window().unwrap().fetch_with_request(&request);
    let value = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|e| format!("fetch: {e:?}"))?;

    let resp = value
        .dyn_into::<Response>()
        .map_err(|_| "type error".to_string())?;
    let text = js_text(&resp).await?;
    if !resp.ok() {
        return Err(format!("HTTP {} {}", resp.status(), text));
    }
    serde_json::from_str(&text).map_err(|e| format!("json: {e}"))
}

pub async fn api_delete(path: &str, token: &str) -> Result<(), String> {
    #[allow(unused_mut)]
    let mut opts = RequestInit::new();
    opts.set_method("DELETE");
    opts.set_headers(&headers_with_auth(token));

    let request = Request::new_with_str_and_init(&api_url(path), &opts)
        .map_err(|e| format!("request: {e:?}"))?;

    let promise = window().unwrap().fetch_with_request(&request);
    let value = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|e| format!("fetch: {e:?}"))?;

    let resp = value
        .dyn_into::<Response>()
        .map_err(|_| "type error".to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    Ok(())
}

#[expect(dead_code)]
pub async fn api_patch(path: &str, json: &Value, token: &str) -> Result<Value, String> {
    let body = serde_json::to_string(json).map_err(|e| e.to_string())?;
    #[allow(unused_mut)]
    let mut opts = RequestInit::new();
    opts.set_method("PATCH");
    opts.set_headers(&headers_with_auth(token));
    opts.set_body(&JsValue::from_str(&body));

    let request = Request::new_with_str_and_init(&api_url(path), &opts)
        .map_err(|e| format!("request: {e:?}"))?;

    let promise = window().unwrap().fetch_with_request(&request);
    let value = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|e| format!("fetch: {e:?}"))?;

    let resp = value
        .dyn_into::<Response>()
        .map_err(|_| "type error".to_string())?;
    let text = js_text(&resp).await?;
    if !resp.ok() {
        return Err(format!("HTTP {} {}", resp.status(), text));
    }
    serde_json::from_str(&text).map_err(|e| format!("json: {e}"))
}

async fn js_text(resp: &Response) -> Result<String, String> {
    let text_promise = resp.text().map_err(|e| format!("text: {e:?}"))?;
    wasm_bindgen_futures::JsFuture::from(text_promise)
        .await
        .map_err(|e| format!("text: {e:?}"))?
        .as_string()
        .ok_or_else(|| "no text".to_string())
}
