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
    let mut click_count = use_signal(|| 0u32);
    let mut easter_egg = use_signal(|| false);
    // Quantos caracteres de " e Renato" já foram revelados (0..=9)
    let mut chars_shown = use_signal(|| 0usize);

    // Quando o easter egg dispara: revela um char a cada 80ms, segura 10s, apaga e reseta
    use_effect(move || {
        let triggered = *easter_egg.read();
        let already_started = *chars_shown.read() > 0;
        if triggered && !already_started {
            spawn(async move {
                // Reveal: um caractere a cada 80ms
                for i in 1usize..=9 {
                    #[cfg(target_arch = "wasm32")]
                    {
                        let mut ev = eval(
                            "await new Promise(r => setTimeout(r, 80)); dioxus.send(true);"
                        );
                        let _ = ev.recv::<bool>().await;
                    }
                    chars_shown.set(i);
                }
                // Segura por 10s
                #[cfg(target_arch = "wasm32")]
                {
                    let mut ev = eval(
                        "await new Promise(r => setTimeout(r, 5000)); dioxus.send(true);"
                    );
                    let _ = ev.recv::<bool>().await;
                }
                // Apaga: um caractere a cada 60ms (ligeiramente mais rápido)
                for i in (0usize..9).rev() {
                    #[cfg(target_arch = "wasm32")]
                    {
                        let mut ev = eval(
                            "await new Promise(r => setTimeout(r, 60)); dioxus.send(true);"
                        );
                        let _ = ev.recv::<bool>().await;
                    }
                    if i == 0 {
                        // Reseta antes de zerar chars para evitar re-trigger
                        easter_egg.set(false);
                        click_count.set(0);
                    }
                    chars_shown.set(i);
                }
            });
        }
    });

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

    // Texto visível do easter egg (" e Renato", um char por vez)
    let egg_on = *easter_egg.read();
    let egg_text = &" e Renato"[..*chars_shown.read()];

    rsx! {
        div { class: "max-w-[680px] mx-auto mt-10 px-8 pb-16 [animation:fade-up_0.4s_ease_both]",

            // ── Header ─────────────────────────────────────────────────────
            div { class: "mb-10",
                h1 {
                    class: if egg_on { "home-title home-title-egg" } else { "home-title" },
                    style: "cursor: default; user-select: none;",
                    onclick: move |_| {
                        let n = *click_count.read() + 1;
                        click_count.set(n);
                        if n >= 10 {
                            easter_egg.set(true);
                        }
                    },
                    "HERMES"
                    if egg_on {
                        span { class: "easter-egg-suffix", "{egg_text}" }
                    }
                    span { class: "home-cursor", "_" }
                }
                p { class: "tagline", "point-to-point · server-cached · encrypted" }
            }

            // ── Mode Selector ──────────────────────────────────────────────
            div { class: "flex mb-7 border border-[var(--border)] rounded-[var(--radius)] overflow-hidden",
                label {
                    class: if *mode.read() == TransferMode::ServerUpload { "mode-btn active" } else { "mode-btn" },
                    input {
                        r#type: "radio",
                        name: "mode",
                        checked: *mode.read() == TransferMode::ServerUpload,
                        onchange: move |_| mode.set(TransferMode::ServerUpload),
                    }
                    svg {
                        class: "mode-icon",
                        fill: "none",
                        stroke: "currentColor",
                        view_box: "0 0 24 24",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        path { d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" }
                        path { d: "M17 8l-5-5-5 5" }
                        path { d: "M12 3v12" }
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
                    svg {
                        class: "mode-icon",
                        fill: "none",
                        stroke: "currentColor",
                        view_box: "0 0 24 24",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        path { d: "M5 12.55a11 11 0 0 1 14.08 0" }
                        path { d: "M1.42 9a16 16 0 0 1 21.16 0" }
                        path { d: "M8.53 16.11a6 6 0 0 1 6.95 0" }
                        path { d: "M12 20h.01", stroke_width: "3" }
                    }
                    span { "P2P Direct" }
                }
            }

            // ── Content ────────────────────────────────────────────────────
            if *mode.read() == TransferMode::ServerUpload {
                FileUploader { on_uploaded }

                if let Some(ref resp) = *upload_result.read() {
                    div { class: "mt-6 [animation:fade-up_0.35s_ease_both]",
                        div { class: "bg-[var(--surface)] border border-[var(--border)] rounded-[var(--radius-lg)] overflow-hidden",
                            div { class: "px-5 py-3 border-b border-[var(--border)] flex items-center gap-2 text-[0.8rem] uppercase tracking-[0.12em] text-[var(--accent)] bg-[rgba(110,114,251,0.05)]",
                                div { class: "upload-result-dot" }
                                "transfer complete"
                            }
                            div { class: "p-5 flex flex-col gap-4",
                                ShareLinkWidget {
                                    label: "direct link".to_string(),
                                    url: resp.download_url.clone(),
                                }
                                hr { class: "h-px bg-[var(--border)] border-none" }
                                if share_url.read().is_none() {
                                    button {
                                        class: "btn btn-ghost btn-w-full",
                                        onclick: on_generate_link,
                                        "Generate 10-min share link"
                                    }
                                }
                                if let Some(ref url) = *share_url.read() {
                                    ShareLinkWidget {
                                        label: "expires in 10 min".to_string(),
                                        url: url.clone(),
                                    }
                                }
                                if let Some(ref err) = *share_error.read() {
                                    p { class: "text-[0.875rem] text-[var(--error)] px-[0.8rem] py-[0.6rem] border-l-2 border-[var(--error)] bg-[var(--error-dim)] rounded-r-[var(--radius)] tracking-[0.02em]", "{err}" }
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

// ── Share Link Widget ─────────────────────────────────────────────────────────

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
        div { class: "flex flex-col gap-[0.4rem]",
            p { class: "text-[0.75rem] uppercase tracking-[0.1em] text-[var(--text-muted)]", "{props.label}" }
            div { class: "flex items-center gap-2 bg-[var(--bg)] border border-[var(--border)] rounded-[var(--radius)] px-4 py-[0.7rem] transition-[border-color] duration-[var(--transition)] hover:border-[rgba(110,114,251,0.25)]",
                code { class: "flex-1 font-[var(--font)] text-[0.875rem] text-[var(--info)] overflow-hidden text-ellipsis whitespace-nowrap tracking-[0.02em]", "{current_url}" }
                button {
                    class: if *copied.read() { "share-link-copy copied" } else { "share-link-copy" },
                    title: "Copy to clipboard",
                    onclick: move |_| {
                        let to_copy = current_url.clone();
                        copied.set(true);
                        spawn(async move {
                            let _ = eval(&format!(r#"
                                navigator.clipboard.writeText("{to_copy}").catch(err => {{
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
                            fill: "none",
                            stroke: "currentColor",
                            view_box: "0 0 24 24",
                            stroke_width: "2.5",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            style: "width:13px;height:13px",
                            path { d: "M20 6L9 17l-5-5" }
                        }
                    } else {
                        svg {
                            fill: "none",
                            stroke: "currentColor",
                            view_box: "0 0 24 24",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            style: "width:13px;height:13px",
                            path { d: "M8 17H6a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v2" }
                            path { d: "M10 9h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-8a2 2 0 0 1-2-2v-8a2 2 0 0 1 2-2z" }
                        }
                    }
                }
            }
        }
    }
}

// ── P2P Panel ─────────────────────────────────────────────────────────────────

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
        div { class: "flex flex-col gap-5",
            if let Some(ref err) = *error.read() {
                p { class: "text-[0.875rem] text-[var(--error)] px-[0.8rem] py-[0.6rem] border-l-2 border-[var(--error)] bg-[var(--error-dim)] rounded-r-[var(--radius)] tracking-[0.02em]", "Signaling error: {err}" }
            } else if let Some(ref id) = *session_id.read() {
                WebRtcWidget { session_id: id.clone() }
            } else {
                p { class: "p2p-status-connecting", "Connecting to signaling server" }
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
            let mut ev = eval(&format!(r#"
                (async () => {{
                    while (typeof window.startP2pSender !== 'function') {{
                        await new Promise(r => setTimeout(r, 50));
                    }}
                    await window.startP2pSender('{}');
                    dioxus.send("connected");
                }})();
            "#, sid));
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
                        "left"     => is_dragging.set(false),
                        "dropped"  => is_dragging.set(false),
                        "done"     => file_selected.set(true),
                        _          => {}
                    }
                }
            }
        });
    });

    rsx! {
        div { class: "flex flex-col gap-5",
            // Always show the share link so the user can send it to the receiver.
            if !full_receive_url_clone.is_empty() {
                div { class: "flex flex-col gap-3 [animation:fade-up_0.3s_ease_both]",
                    div { class: "mt-3",
                        ShareLinkWidget {
                            label: "share with receiver".to_string(),
                            url: full_receive_url_clone,
                        }
                    }
                }
            }

            if *sender_connected.read() {
                div { class: "p2p-connected-badge", "signaling connected" }
                p { class: "p2p-instructions", "Waiting for receiver — once connected, select a file below." }
            } else {
                p { class: "p2p-status-connecting", "Connecting to signaling server…" }
            }

            div { class: "w-full",
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

                    // Upload icon
                    svg {
                        class: "drop-zone-icon",
                        fill: "none",
                        stroke: "currentColor",
                        view_box: "0 0 24 24",
                        stroke_width: "1.5",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        path { d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" }
                        path { d: "M17 8l-5-5-5 5" }
                        path { d: "M12 3v12" }
                    }

                    if *file_selected.read() {
                        span { class: "drop-zone-hint drop-zone-uploading", "transfer initiated" }
                        span { class: "drop-zone-hint-sub", "keep this tab open" }
                    } else {
                        span { class: "drop-zone-hint", "drop file or click to select" }
                        span { class: "drop-zone-hint-sub", "direct P2P — no server storage" }
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
