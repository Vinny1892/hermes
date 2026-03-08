//! Home / upload page.
//!
//! Presents the mode selector (server upload vs P2P) and the [`FileUploader`]
//! widget. After upload, shows the shareable link and optional P2P status.

use dioxus::prelude::*;
#[cfg(target_arch = "wasm32")]
use dioxus::document::eval;

use crate::{
    api::{create_p2p_session, generate_share_link},
    components::FileUploader,
    models::UploadResponse,
};

/// Transfer mode chosen by the user.
#[derive(Clone, PartialEq)]
enum TransferMode {
    /// Store on the server; allow multiple downloads for 7 days.
    ServerUpload,
    /// Direct P2P via WebRTC DataChannel.
    P2p,
}

/// Home page — file upload and link generation.
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
        let upload_result = upload_result.clone();
        async move {
            if let Some(ref resp) = *upload_result.read() {
                let file_id = resp.file_id.to_string();
                match generate_share_link(file_id).await {
                    Ok(link) => share_url.set(Some(link.share_url)),
                    Err(e) => share_error.set(Some(e.to_string())),
                }
            }
        }
    };

    rsx! {
        div { class: "page home-page",
            h1 { "hermes" }
            p { class: "tagline", "Fast file sharing between friends" }

            // Mode selector
            div { class: "mode-selector",
                label {
                    class: if *mode.read() == TransferMode::ServerUpload { "mode-btn active" } else { "mode-btn" },
                    input {
                        r#type: "radio",
                        name: "mode",
                        checked: *mode.read() == TransferMode::ServerUpload,
                        onchange: move |_| mode.set(TransferMode::ServerUpload),
                    }
                    "Save on server"
                }
                label {
                    class: if *mode.read() == TransferMode::P2p { "mode-btn active" } else { "mode-btn" },
                    input {
                        r#type: "radio",
                        name: "mode",
                        checked: *mode.read() == TransferMode::P2p,
                        onchange: move |_| mode.set(TransferMode::P2p),
                    }
                    "Direct P2P transfer"
                }
            }

            if *mode.read() == TransferMode::ServerUpload {
                FileUploader { on_uploaded }

                if let Some(ref resp) = *upload_result.read() {
                    div { class: "upload-result",
                        p {
                            "Uploaded! Direct link: "
                            a { href: "{resp.download_url}", "{resp.download_url}" }
                        }
                        button {
                            class: "btn",
                            onclick: on_generate_link,
                            "Generate 10-min share link"
                        }
                        if let Some(ref url) = *share_url.read() {
                            div { class: "share-link",
                                p { "Share this link (expires in 10 min):" }
                                code { "{url}" }
                            }
                        }
                        if let Some(ref err) = *share_error.read() {
                            p { class: "error", "{err}" }
                        }
                    }
                }
            } else {
                P2pPanel {}
            }
        }
    }
}

// ── P2P panel ─────────────────────────────────────────────────────────────────

/// Panel shown when the user selects P2P mode.
#[component]
fn P2pPanel() -> Element {
    let mut session_info = use_signal(|| Option::<(String, uuid::Uuid)>::None);
    let mut error = use_signal(|| Option::<String>::None);

    let on_start = move |_| async move {
        match create_p2p_session().await {
            Ok(resp) => session_info.set(Some((resp.signal_url, resp.session_id))),
            Err(e) => error.set(Some(e.to_string())),
        }
    };

    rsx! {
        div { class: "p2p-panel",
            if session_info.read().is_none() {
                button { class: "btn", onclick: on_start, "Start P2P transfer" }
            }
            if let Some((ref url, ref id)) = *session_info.read() {
                WebRtcWidget { signal_url: url.clone(), session_id: id.clone() }
            }
            if let Some(ref e) = *error.read() {
                p { class: "error", "{e}" }
            }
        }
    }
}

// ── WebRTC widget ─────────────────────────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
struct WebRtcWidgetProps {
    signal_url: String,
    session_id: uuid::Uuid,
}

/// Renders the P2P send widget and boots the WebRTC JS.
#[component]
fn WebRtcWidget(props: WebRtcWidgetProps) -> Element {
    let signal_url = props.signal_url.clone();
    let session_id = props.session_id.clone();
    let mut file_selected = use_signal(|| false);
    // Only show the receive link after the sender's WebSocket is open.
    // This prevents the race condition where the receiver connects first
    // and accidentally takes slot 'a' (the sender's slot) in the registry.
    #[allow(unused_mut)] // mutated only on wasm32 inside the cfg block
    let mut sender_connected = use_signal(|| false);

    use_effect(move || {
        let url = signal_url.clone();
        #[cfg(target_arch = "wasm32")]
        {
            // Await startP2pSender so we know the WebSocket is open before
            // revealing the receive link.
            spawn(async move {
                let mut ev = eval(&format!(r#"
                    (async () => {{
                        // Wait for webrtc.js to load before calling startP2pSender.
                        while (typeof window.startP2pSender !== 'function') {{
                            await new Promise(r => setTimeout(r, 50));
                        }}
                        await window.startP2pSender({url:?});
                        dioxus.send(true);
                    }})();
                "#));
                if ev.recv::<bool>().await.is_ok() {
                    sender_connected.set(true);
                }
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = url;
    });

    let receive_url = format!("/receive/{}", session_id);

    rsx! {
        div { class: "webrtc-widget",
            if *sender_connected.read() {
                div { class: "share-link p2p-link",
                    p { "Share this link with your friend to start the P2P transfer:" }
                    code { "{receive_url}" }
                }
                p { class: "p2p-instructions", "Waiting for receiver... once they connect, select a file below." }
            } else {
                p { class: "p2p-status-connecting", "Connecting to signaling server…" }
            }

            div { class: "uploader p2p-uploader",
                label { class: "drop-zone",
                    input {
                        id: "p2p-file-input",
                        r#type: "file",
                        style: "display:none",
                        onchange: move |_| {
                            file_selected.set(true);
                            #[cfg(target_arch = "wasm32")]
                            spawn(async move {
                                let _ = eval("if (typeof startP2pTransfer === 'function') startP2pTransfer();").await;
                            });
                        },
                    }
                    if *file_selected.read() {
                        span { class: "drop-zone-hint", "File selected! Starting transfer..." }
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
