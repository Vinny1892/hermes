//! Root component and application routes.
//!
//! # Route tree
//!
//! ```text
//! /                   → Home       (upload + mode selector)
//! /f/:file_id         → Download   (shows file info + download button)
//! /receive/:session_id → Receive   (P2P receiver, waits for sender)
//! ```
//!
//! The `Navbar` layout wraps all routes and injects global CSS/JS assets.

use dioxus::prelude::*;

use crate::pages::{Download, Home, Receive};

/// Application route enum.
///
/// Each variant maps to a URL pattern. Dynamic segments (`:file_id`,
/// `:session_id`) become fields in the variant and are passed as props to
/// the corresponding page component.
#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Navbar)]
        #[route("/")]
        Home {},
        #[route("/d/:file_id")]
        Download { file_id: String },
        #[route("/receive/:session_id")]
        Receive { session_id: String },
}

/// Root component.
///
/// Injects global assets (favicon, CSS, WebRTC JS) and mounts the router.
#[component]
pub fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: asset!("/assets/favicon.ico") }
        document::Link { rel: "stylesheet", href: asset!("/assets/main.css") }
        document::Link { rel: "stylesheet", href: asset!("/assets/tailwind.css") }
        document::Script { src: asset!("/assets/webrtc.js") }
        Router::<Route> {}
    }
}

/// Shared navigation bar rendered around every route via `#[layout(Navbar)]`.
#[component]
fn Navbar() -> Element {
    rsx! {
        nav { class: "navbar",
            Link { to: Route::Home {}, class: "navbar-brand", "hermes" }
        }
        Outlet::<Route> {}
    }
}
