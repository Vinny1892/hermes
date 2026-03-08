//! P2P receive page — shown when a user opens `/receive/{session_id}`.
//!
//! Connects to the signaling WebSocket as Peer B and waits for the sender.
//! All WebRTC logic (answer creation, ICE handling, DataChannel receive) runs
//! in `assets/webrtc.js`; this Rust component boots the JS via `eval`.

use dioxus::prelude::*;
#[cfg(target_arch = "wasm32")]
use dioxus::document::eval;

use crate::app::Route;

/// P2P receive page at `/receive/{session_id}`.
#[component]
pub fn Receive(session_id: String) -> Element {
    let sid = session_id.clone();

    use_effect(move || {
        let id = sid.clone();
        #[cfg(target_arch = "wasm32")]
        {
            spawn(async move {
                let _ = eval(&format!(r#"
                    (async () => {{
                        while (typeof window.startP2pReceiver !== 'function') {{
                            await new Promise(r => setTimeout(r, 50));
                        }}
                        window.startP2pReceiver({id:?});
                    }})();
                "#));
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = id;
    });

    rsx! {
        div { class: "page receive-page",
            h2 { "Receiving file" }
            p { "Keep this tab open — the sender is transferring directly to you." }

            div { class: "receive-status-card",
                // Signal/antenna icon
                svg {
                    class: "receive-icon",
                    fill: "none",
                    stroke: "currentColor",
                    view_box: "0 0 24 24",
                    stroke_width: "1.5",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    path { d: "M5 12.55a11 11 0 0 1 14.08 0" }
                    path { d: "M1.42 9a16 16 0 0 1 21.16 0" }
                    path { d: "M8.53 16.11a6 6 0 0 1 6.95 0" }
                    path { d: "M12 20h.01", stroke_width: "3" }
                }

                span { class: "receive-label", "waiting for connection" }

                div { id: "p2p-status" }
                div { id: "p2p-progress" }
                div { id: "p2p-download" }
            }

            Link { to: Route::Home {}, class: "btn btn-secondary", "Cancel" }
        }
    }
}
