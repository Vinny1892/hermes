//! Home / upload page.
//!
//! Presents the mode selector (server upload vs P2P) and the [`FileUploader`]
//! widget. After upload, shows the shareable link and optional P2P status.

use dioxus::prelude::*;
use dioxus::document::eval;
use crate::app::Route;

use crate::{
    api::{create_p2p_session, generate_share_link},
    components::FileUploader,
    models::UploadResponse,
};

/// Home page component.
#[component]
pub fn Home() -> Element {
    let mut mode = use_signal(|| TransferMode::ServerUpload);
    let mut upload_result = use_signal(|| Option::<UploadResponse>::None);
    let mut share_url = use_signal(|| Option::<String>::None);
    let mut share_error = use_signal(|| Option::<String>::None);

    let on_uploaded = move |resp: UploadResponse| {
        upload_result.set(Some(resp));
        share_url.set(None);
        share_error.set(None);
    };

    let on_generate_link = move |_| {
        if let Some(ref resp) = *upload_result.read() {
            let file_id = resp.file_id.clone();
            spawn(async move {
                match generate_share_link(file_id.to_string()).await {
                    Ok(res) => share_url.set(Some(res.share_url)),
                    Err(e) => share_error.set(Some(e.to_string())),
                }
            });
        }
    };

    rsx! {
        div { class: "page home-page",
            h1 { "hermes" }
            p { class: "tagline", "Share files securely via server or P2P." }

            div { class: "mode-selector",
                label { 
                    class: if *mode.read() == TransferMode::ServerUpload { "mode-btn active" } else { "mode-btn" },
                    input {
                        r#type: "radio",
                        name: "mode",
                        checked: *mode.read() == TransferMode::ServerUpload,
                        onchange: move |_| mode.set(TransferMode::ServerUpload),
                    }
                    span { "Server Upload" }
                }
                label { 
                    class: if *mode.read() == TransferMode::P2P { "mode-btn active" } else { "mode-btn" },
                    input {
                        r#type: "radio",
                        name: "mode",
                        checked: *mode.read() == TransferMode::P2P,
                        onchange: move |_| mode.set(TransferMode::P2P),
                    }
                    span { "Direct P2P" }
                }
            }

            if *mode.read() == TransferMode::ServerUpload {
                FileUploader { on_uploaded }

                if let Some(ref resp) = *upload_result.read() {
                    div { class: "upload-result mt-8 animate-in fade-in slide-in-from-bottom-4 duration-500",
                        div { class: "flex flex-col gap-4 bg-slate-900/40 p-6 rounded-2xl border border-white/5 backdrop-blur-sm shadow-xl",
                            div {
                                p { class: "text-sm text-gray-400 mb-1 font-medium", "File uploaded successfully!" }
                                ShareLinkWidget { 
                                    label: "Direct download link:", 
                                    url: resp.download_url.clone() 
                                }
                            }
                            
                            div { class: "border-t border-white/5 pt-4 flex flex-col gap-3",
                                if share_url.read().is_none() {
                                    button {
                                        class: "btn w-full bg-blue-600/20 hover:bg-blue-600/30 text-blue-400 border border-blue-500/20 py-2 rounded-xl transition-all active:scale-[0.98]",
                                        onclick: on_generate_link,
                                        "Generate 10-min share link"
                                    }
                                }
                                if let Some(ref url) = *share_url.read() {
                                    ShareLinkWidget { 
                                        label: "Temporary share link (expires in 10 min):", 
                                        url: url.clone() 
                                    }
                                }
                                if let Some(ref err) = *share_error.read() {
                                    p { class: "text-red-400 text-xs mt-1", "{err}" }
                                }
                            }
                        }
                    }
                }
            } else {
                P2pPanel {}
            }
        }
    }
}

// ── Shared UI Components ──────────────────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
struct ShareLinkWidgetProps {
    label: String,
    url: String,
}

#[component]
fn ShareLinkWidget(props: ShareLinkWidgetProps) -> Element {
    let mut copied = use_signal(|| false);
    let mut full_url = use_signal(|| props.url.clone());
    let url_prop = props.url.clone();

    use_effect(move || {
        let url = url_prop.clone();
        if url.starts_with('/') {
            spawn(async move {
                let mut ev = eval(r#"dioxus.send(window.location.origin);"#);
                if let Ok(origin) = ev.recv::<String>().await {
                    full_url.set(format!("{}{}", origin, url));
                }
            });
        } else {
            full_url.set(url);
        }
    });

    let current_url = full_url.read().clone();

    rsx! {
        div { class: "share-link-widget",
            p { class: "text-[11px] uppercase tracking-wider text-gray-500 mb-1.5 ml-1 font-bold", "{props.label}" }
            div { class: "flex items-center gap-3 bg-slate-900/80 p-3 rounded-xl border border-white/10 shadow-inner group",
                code { class: "flex-1 font-mono text-xs truncate text-blue-300/90 selection:bg-blue-500/30", "{current_url}" }
                button {
                    class: "p-1.5 hover:bg-white/5 rounded-lg transition-all active:scale-90 relative",
                    title: "Copy to clipboard",
                    onclick: move |_| {
                        let to_copy = current_url.clone();
                        copied.set(true);
                        spawn(async move {
                            let _ = eval(&format!(r#"
                                navigator.clipboard.writeText("{to_copy}").then(() => {{
                                    dioxus.send("copied");
                                }}).catch(err => {{
                                    console.error("Failed to copy:", err);
                                }});
                            "#));
                            let mut _ev = eval(r#"await new Promise(r => setTimeout(r, 2000)); dioxus.send(true);"#);
                            let _ = _ev.recv::<bool>().await;
                            copied.set(false);
                        });
                    },
                    if *copied.read() {
                        svg {
                            class: "w-4 h-4 text-green-400 animate-in zoom-in-50 duration-300",
                            fill: "none",
                            stroke: "currentColor",
                            view_box: "0 0 24 24",
                            path {
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                stroke_width: "3",
                                d: "M5 13l4 4L19 7"
                            }
                        }
                    } else {
                        svg {
                            class: "w-4 h-4 text-gray-500 group-hover:text-blue-400 transition-colors",
                            fill: "none",
                            stroke: "currentColor",
                            view_box: "0 0 24 24",
                            path {
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                stroke_width: "2",
                                d: "M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── P2P panel ─────────────────────────────────────────────────────────────────

#[component]
fn P2pPanel() -> Element {
    let mut session_id = use_signal(|| Option::<String>::None);
    let mut error = use_signal(|| Option::<String>::None);

    use_effect(move || {
        if session_id.read().is_none() {
            spawn(async move {
                match create_p2p_session().await {
                    Ok(res) => session_id.set(Some(res.session_id.to_string())),
                    Err(e) => error.set(Some(e.to_string())),
                }
            });
        }
    });

    rsx! {
        div { class: "p2p-panel",
            h2 { "P2P Transfer" }
            if let Some(ref err) = *error.read() {
                p { class: "error", "Signaling error: {err}" }
            } else if let Some(ref id) = *session_id.read() {
                WebRtcWidget { session_id: id.clone() }
            } else {
                p { "Creating session..." }
            }
        }
    }
}

#[component]
fn WebRtcWidget(session_id: String) -> Element {
    let mut sender_connected = use_signal(|| false);
    let mut file_selected = use_signal(|| false);
    let mut full_receive_url = use_signal(|| "".to_string());
    let mut is_dragging = use_signal(|| false);

    let receive_url = Route::Receive { session_id: session_id.to_string() }.to_string();

    use_effect(move || {
        let receive_url = receive_url.clone();
        spawn(async move {
            let mut ev = eval(r#"dioxus.send(window.location.origin);"#);
            if let Ok(origin) = ev.recv::<String>().await {
                full_receive_url.set(format!("{}{}", origin, receive_url));
            }
        });
    });

    let full_receive_url_clone = (*full_receive_url.read()).clone();

    use_effect(move || {
        let sid = session_id.clone();
        spawn(async move {
            let mut ev = eval(&format!("startP2pSender('{}')", sid));
            while let Ok(msg) = ev.recv::<String>().await {
                if msg == "connected" {
                    sender_connected.set(true);
                }
            }
        });
    });

    // Native Drag & Drop listener for P2P
    use_effect(move || {
        spawn(async move {
            #[cfg(target_arch = "wasm32")]
            {
                let mut ev = eval(r#"
                    const el = document.getElementById("drop-zone-p2p");
                    if (!el) return;

                    el.addEventListener("dragover", e => { e.preventDefault(); e.stopPropagation(); dioxus.send("dragging"); });
                    el.addEventListener("dragleave", e => { e.preventDefault(); e.stopPropagation(); dioxus.send("left"); });
                    el.addEventListener("drop", async e => {
                        e.preventDefault();
                        e.stopPropagation();
                        dioxus.send("dropped");
                        const file = e.dataTransfer.files[0];
                        if (file && typeof window.startP2pTransferWithFile === 'function') {
                            window.startP2pTransferWithFile(file);
                            dioxus.send("done");
                        }
                    });
                "#);

                while let Ok(msg) = ev.recv::<String>().await {
                    match msg.as_str() {
                        "dragging" => is_dragging.set(true),
                        "left" => is_dragging.set(false),
                        "dropped" => is_dragging.set(false),
                        "done" => file_selected.set(true),
                        _ => {}
                    }
                }
            }
        });
    });

    rsx! {
        div { class: "webrtc-widget mt-6",
            if *sender_connected.read() {
                div { class: "p2p-share-container animate-in fade-in slide-in-from-bottom-4 duration-500",
                    ShareLinkWidget {
                        label: "Share this link with your friend:",
                        url: full_receive_url_clone
                    }
                }
                p { class: "p2p-instructions text-gray-400 text-xs mt-4 italic", "Waiting for receiver... once they connect, select a file below." }
            } else {
                p { class: "p2p-status-connecting", "Connecting to signaling server…" }
            }

            div { class: "uploader p2p-uploader",
                label { 
                    id: "drop-zone-p2p",
                    class: if *is_dragging.read() { "drop-zone dragging" } else { "drop-zone" },
                    
                    input {
                        id: "p2p-file-input",
                        r#type: "file",
                        style: "display:none",
                        onchange: move |_e| {
                            file_selected.set(true);
                            #[cfg(target_arch = "wasm32")]
                            spawn(async move {
                                let _ = eval("if (typeof startP2pTransfer === 'function') startP2pTransfer();").await;
                            });
                        },
                    }
                    if *file_selected.read() {
                        span { class: "drop-zone-hint text-green-400", "File selected! Starting transfer..." }
                    } else {
                        span { class: "drop-zone-hint", "Select file to send via P2P" }
                    }
                }
            }

            div { id: "p2p-status", class: "p2p-status" }
            div { id: "p2p-progress", class: "p2p-progress" }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum TransferMode {
    ServerUpload,
    P2P,
}
