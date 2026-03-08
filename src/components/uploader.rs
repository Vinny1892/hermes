//! File uploader component.
//!
//! Renders a drag-and-drop / click-to-browse file input. When the user selects
//! files, they are uploaded to `POST /api/upload` via the browser's native
//! `fetch` API (called through [`eval`]). For each file successfully uploaded
//! the `on_uploaded` callback is called with an [`UploadResponse`].

use dioxus::prelude::*;
#[cfg(target_arch = "wasm32")]
use dioxus::document::eval;

use crate::models::UploadResponse;

/// Props for [`FileUploader`].
#[derive(Props, Clone, PartialEq)]
pub struct FileUploaderProps {
    /// Called once per successfully uploaded file.
    pub on_uploaded: EventHandler<UploadResponse>,
}

/// Interactive file upload widget.
///
/// Selecting one or more files triggers a JS `fetch` call to `/api/upload` for
/// each file. Progress is shown inline; errors are displayed below the zone.
#[component]
pub fn FileUploader(props: FileUploaderProps) -> Element {
    let mut uploading = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);

    rsx! {
        div { class: "uploader",
            if let Some(ref err) = *error_msg.read() {
                div { class: "uploader-error", "{err}" }
            }

            label { class: "drop-zone",
                input {
                    r#type: "file",
                    multiple: true,
                    style: "display:none",
                    onchange: move |_event| {
                        let _on_uploaded = props.on_uploaded.clone();
                        async move {
                            uploading.set(true);
                            error_msg.set(None);

                            // The actual upload runs in JS; cfg-gated so the
                            // server build (which can't use eval) still compiles.
                            #[cfg(target_arch = "wasm32")]
                            match js_upload_all().await {
                                Ok(responses) => {
                                    for r in responses {
                                        _on_uploaded.call(r);
                                    }
                                }
                                Err(e) => error_msg.set(Some(e)),
                            }

                            uploading.set(false);
                        }
                    },
                }
                if *uploading.read() {
                    span { class: "drop-zone-hint", "Uploading..." }
                } else {
                    span { class: "drop-zone-hint", "Drop files here or click to browse" }
                }
            }
        }
    }
}

/// Uploads all files currently in the `<input type="file">` via browser `fetch`.
///
/// Returns a [`Vec<UploadResponse>`] — one entry per file. The JS snippet
/// iterates `input.files`, POSTs each as `multipart/form-data`, and calls
/// `dioxus.send(results)` once all uploads complete (or fail).
///
/// This function is only compiled for the WASM target; the server build gets
/// the unreachable stub below.
#[cfg(target_arch = "wasm32")]
async fn js_upload_all() -> Result<Vec<UploadResponse>, String> {
    let script = r#"
    (async () => {
        const input = document.querySelector('input[type="file"]');
        if (!input || !input.files || input.files.length === 0) {
            dioxus.send({ ok: [] });
            return;
        }
        const results = [];
        for (const file of input.files) {
            const fd = new FormData();
            fd.append("file", file);
            const resp = await fetch("/api/upload", { method: "POST", body: fd });
            if (!resp.ok) {
                const text = await resp.text();
                dioxus.send({ error: `${file.name}: HTTP ${resp.status} — ${text}` });
                return;
            }
            results.push(await resp.json());
        }
        dioxus.send({ ok: results });
    })();
    "#;

    let mut ev = eval(script);

    let raw: serde_json::Value = ev
        .recv()
        .await
        .map_err(|e| format!("eval channel error: {e}"))?;

    if let Some(err) = raw.get("error").and_then(|v| v.as_str()) {
        return Err(err.to_owned());
    }

    let responses: Vec<UploadResponse> = serde_json::from_value(
        raw.get("ok").cloned().unwrap_or(serde_json::Value::Array(vec![])),
    )
    .map_err(|e| format!("JSON parse error: {e}"))?;

    Ok(responses)
}

