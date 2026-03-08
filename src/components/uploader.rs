//! File uploader component.
//!
//! Renders a drag-and-drop / click-to-browse file input. When the user selects
//! files, they are uploaded to `POST /api/upload` via the browser's native
//! `fetch` API.

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
#[component]
pub fn FileUploader(props: FileUploaderProps) -> Element {
    let mut uploading = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut is_dragging = use_signal(|| false);

    // Injetamos o listener de Drag & Drop nativo
    use_effect(move || {
        let mut on_uploaded = props.on_uploaded.clone();
        spawn(async move {
            #[cfg(target_arch = "wasm32")]
            {
                let mut ev = eval(r#"
                    const el = document.getElementById("drop-zone-server");
                    if (!el) return;

                    el.addEventListener("dragover", e => {
                        e.preventDefault();
                        e.stopPropagation();
                        dioxus.send("dragging");
                    });

                    el.addEventListener("dragleave", e => {
                        e.preventDefault();
                        e.stopPropagation();
                        dioxus.send("left");
                    });

                    el.addEventListener("drop", async e => {
                        e.preventDefault();
                        e.stopPropagation();
                        dioxus.send("dropped");

                        const files = e.dataTransfer.files;
                        if (!files || files.length === 0) return;

                        for (const file of files) {
                            const fd = new FormData();
                            fd.append("file", file);
                            try {
                                const resp = await fetch("/api/upload", { method: "POST", body: fd });
                                if (resp.ok) {
                                    dioxus.send({ ok: await resp.json() });
                                } else {
                                    dioxus.send({ error: `Upload failed: ${resp.status}` });
                                }
                            } catch (err) {
                                dioxus.send({ error: err.message });
                            }
                        }
                        dioxus.send("done");
                    });
                "#);

                while let Ok(msg) = ev.recv::<serde_json::Value>().await {
                    match msg {
                        serde_json::Value::String(s) if s == "dragging" => is_dragging.set(true),
                        serde_json::Value::String(s) if s == "left" => is_dragging.set(false),
                        serde_json::Value::String(s) if s == "dropped" => {
                            is_dragging.set(false);
                            uploading.set(true);
                            error_msg.set(None);
                        },
                        serde_json::Value::String(s) if s == "done" => uploading.set(false),
                        serde_json::Value::Object(map) => {
                            if let Some(ok) = map.get("ok") {
                                if let Ok(resp) = serde_json::from_value::<UploadResponse>(ok.clone()) {
                                    on_uploaded.call(resp);
                                }
                            } else if let Some(err) = map.get("error").and_then(|v| v.as_str()) {
                                error_msg.set(Some(err.to_string()));
                                uploading.set(false);
                            }
                        },
                        _ => {}
                    }
                }
            }
        });
    });

    rsx! {
        div { class: "uploader",
            if let Some(ref err) = *error_msg.read() {
                div { class: "uploader-error mb-4", "{err}" }
            }

            label { 
                id: "drop-zone-server",
                class: if *is_dragging.read() { "drop-zone dragging" } else { "drop-zone" },
                
                input {
                    r#type: "file",
                    multiple: true,
                    style: "display:none",
                    onchange: move |_e| {
                        let on_uploaded = props.on_uploaded.clone();
                        async move {
                            uploading.set(true);
                            error_msg.set(None);
                            #[cfg(target_arch = "wasm32")]
                            {
                                let script = r#"
                                    (async () => {
                                        const input = event.target;
                                        for (const file of input.files) {
                                            const fd = new FormData();
                                            fd.append("file", file);
                                            const resp = await fetch("/api/upload", { method: "POST", body: fd });
                                            if (resp.ok) dioxus.send({ ok: await resp.json() });
                                        }
                                        dioxus.send("done");
                                    })();
                                "#;
                                let mut ev = eval(script);
                                while let Ok(msg) = ev.recv::<serde_json::Value>().await {
                                    if let serde_json::Value::Object(map) = msg {
                                        if let Some(ok) = map.get("ok") {
                                            if let Ok(resp) = serde_json::from_value::<UploadResponse>(ok.clone()) {
                                                on_uploaded.call(resp);
                                            }
                                        }
                                    } else if msg == "done" {
                                        break;
                                    }
                                }
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
