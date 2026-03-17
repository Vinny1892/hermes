//! Root component and application routes.

use dioxus::prelude::*;
use dioxus::document::eval;

use crate::pages::{Download, Home, Login, Receive, Settings};

const THEME_INIT: &str = "(function(){\
    if(localStorage.getItem('hermes-theme')==='light')\
        document.documentElement.setAttribute('data-theme','light');\
})();";


#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    // ── Public ────────────────────────────────────────────────────────────────
    #[route("/login")]
    Login {},

    // ── Protected ─────────────────────────────────────────────────────────────
    // Every route below goes through Navbar → AuthGuard → page.
    #[layout(Navbar)]
        #[layout(AuthGuard)]
            #[route("/")]
            Home {},
            #[route("/d/:file_id")]
            Download { file_id: String },
            #[route("/receive/:session_id")]
            Receive { session_id: String },
            #[route("/settings")]
            Settings {},
}

#[component]
pub fn App() -> Element {
    // Set a sentinel on <html> after WASM hydrates. SSR never sets this.
    // Used by Playwright tests to reliably detect WASM readiness.
    use_effect(move || {
        #[cfg(target_arch = "wasm32")]
        spawn(async move {
            let _ = eval("document.documentElement.setAttribute('data-wasm-ready','true')");
        });
    });

    rsx! {
        document::Link { rel: "icon", r#type: "image/png", href: asset!("/assets/favicon.png") }
        document::Link { rel: "stylesheet", href: asset!("/assets/main.css") }
        document::Link { rel: "stylesheet", href: asset!("/assets/tailwind.css") }
        document::Script { "{THEME_INIT}" }
        document::Script { src: asset!("/assets/webrtc.js") }
        Router::<Route> {}
    }
}

// ── Auth guard ─────────────────────────────────────────────────────────────────

/// Checks localStorage synchronously at render time.
/// Server (SSR): cfg block is skipped, children render normally.
/// WASM: redirects to /login if no token is found.
#[component]
fn AuthGuard() -> Element {
    #[cfg(target_arch = "wasm32")]
    {
        let has_token = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item("hermes-token").ok().flatten())
            .is_some();

        if !has_token {
            if let Some(win) = web_sys::window() {
                let _ = win.location().replace("/login");
            }
            return rsx! {
                div { class: "flex items-center justify-center min-h-screen",
                    div { class: "auth-gate-spinner" }
                }
            };
        }
    }

    rsx! { Outlet::<Route> {} }
}

// ── Navbar ─────────────────────────────────────────────────────────────────────

#[component]
fn Navbar() -> Element {
    let mut is_light = use_signal(|| {
        #[cfg(target_arch = "wasm32")]
        {
            web_sys::window()
                .and_then(|w| w.local_storage().ok().flatten())
                .and_then(|s| s.get_item("hermes-theme").ok().flatten())
                .map(|v| v == "light")
                .unwrap_or(false)
        }
        #[cfg(not(target_arch = "wasm32"))]
        { false }
    });

    let is_admin = use_signal(|| {
        #[cfg(target_arch = "wasm32")]
        {
            web_sys::window()
                .and_then(|w| w.local_storage().ok().flatten())
                .and_then(|s| s.get_item("hermes-role").ok().flatten())
                .map(|r| r == "ADMIN")
                .unwrap_or(false)
        }
        #[cfg(not(target_arch = "wasm32"))]
        { false }
    });

    let logout = move |_| {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(win) = web_sys::window() {
                if let Some(storage) = win.local_storage().ok().flatten() {
                    let _ = storage.remove_item("hermes-token");
                    let _ = storage.remove_item("hermes-role");
                }
                let _ = win.location().replace("/login");
            }
        }
    };

    let toggle = move |_| {
        let next = !*is_light.read();
        is_light.set(next);
        spawn(async move {
            let (set_theme, store_val) = if next {
                ("document.documentElement.setAttribute('data-theme','light')", "light")
            } else {
                ("document.documentElement.removeAttribute('data-theme')", "dark")
            };
            let js = format!("{set_theme}; localStorage.setItem('hermes-theme','{store_val}');");
            let _ = eval(&js);
        });
    };

    rsx! {
        nav { class: "navbar",
            Link { to: Route::Home {}, class: "navbar-brand flex items-center no-underline",
                img { src: asset!("/assets/logo-dark.png"),  alt: "Hermes", class: "h-[52px] w-auto block rounded-md [[data-theme=light]_&]:hidden" }
                img { src: asset!("/assets/logo-light.png"), alt: "Hermes", class: "h-[52px] w-auto hidden rounded-md [[data-theme=light]_&]:block" }
            }
            div { class: "flex items-center gap-3",
                div { class: "flex items-center gap-2 text-[0.75rem] text-[var(--text-muted)] tracking-[0.1em] uppercase",
                    div { class: "w-[6px] h-[6px] rounded-full bg-[var(--accent)] shadow-[0_0_8px_var(--accent)] shrink-0" }
                    span { "secure transfer" }
                }
                Link {
                    to: Route::Settings {},
                    class: if *is_admin.read() { "navbar-logout" } else { "hidden" },
                    title: "Settings",
                    svg {
                            fill: "none", stroke: "currentColor",
                            view_box: "0 0 24 24", stroke_width: "2",
                            stroke_linecap: "round", stroke_linejoin: "round",
                            style: "width:17px;height:17px",
                            path { d: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z" }
                            path { d: "M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" }
                        }
                    }
                button {
                    class: "navbar-logout",
                    onclick: logout,
                    title: "Logout",
                    svg {
                        fill: "none", stroke: "currentColor",
                        view_box: "0 0 24 24", stroke_width: "2",
                        stroke_linecap: "round", stroke_linejoin: "round",
                        style: "width:17px;height:17px",
                        path { d: "M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" }
                        polyline { points: "16 17 21 12 16 7" }
                        line { x1: "21", y1: "12", x2: "9", y2: "12" }
                    }
                }
                button {
                    class: "theme-toggle",
                    onclick: toggle,
                    title: if *is_light.read() { "Switch to dark mode" } else { "Switch to light mode" },
                    if *is_light.read() {
                        svg {
                            fill: "none", stroke: "currentColor",
                            view_box: "0 0 24 24", stroke_width: "2",
                            stroke_linecap: "round", stroke_linejoin: "round",
                            style: "width:17px;height:17px",
                            path { d: "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" }
                        }
                    } else {
                        svg {
                            fill: "none", stroke: "currentColor",
                            view_box: "0 0 24 24", stroke_width: "2",
                            stroke_linecap: "round", stroke_linejoin: "round",
                            style: "width:17px;height:17px",
                            path { d: "M12 7a5 5 0 1 0 0 10 5 5 0 0 0 0-10z" }
                            path { d: "M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" }
                        }
                    }
                }
            }
        }
        Outlet::<Route> {}
    }
}
