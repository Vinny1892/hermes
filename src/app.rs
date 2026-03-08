//! Root component and application routes.

use dioxus::prelude::*;
use dioxus::document::eval;

use crate::pages::{Download, Home, Receive};

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

#[component]
fn Navbar() -> Element {
    let mut is_light = use_signal(|| false);

    // Restore theme from localStorage on mount
    use_effect(move || {
        spawn(async move {
            let mut ev = eval(r#"
                const saved = localStorage.getItem('hermes-theme');
                dioxus.send(saved === 'light');
            "#);
            if let Ok(light) = ev.recv::<bool>().await {
                is_light.set(light);
                if light {
                    let _ = eval(r#"document.documentElement.setAttribute('data-theme','light');"#);
                }
            }
        });
    });

    let toggle = move |_| {
        let next = !*is_light.read();
        is_light.set(next);
        spawn(async move {
            if next {
                let _ = eval(r#"
                    document.documentElement.setAttribute('data-theme','light');
                    localStorage.setItem('hermes-theme','light');
                "#);
            } else {
                let _ = eval(r#"
                    document.documentElement.removeAttribute('data-theme');
                    localStorage.setItem('hermes-theme','dark');
                "#);
            }
        });
    };

    rsx! {
        nav { class: "navbar",
            Link { to: Route::Home {}, class: "navbar-brand",
                "HERMES"
                span { class: "navbar-brand-cursor" }
            }
            div { class: "navbar-right",
                div { class: "navbar-meta",
                    div { class: "navbar-status-dot" }
                    span { "secure transfer" }
                }
                button {
                    class: "theme-toggle",
                    onclick: toggle,
                    title: if *is_light.read() { "Switch to dark mode" } else { "Switch to light mode" },
                    if *is_light.read() {
                        // Moon — switch to dark
                        svg {
                            fill: "none",
                            stroke: "currentColor",
                            view_box: "0 0 24 24",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            style: "width:17px;height:17px",
                            path { d: "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" }
                        }
                    } else {
                        // Sun — switch to light
                        svg {
                            fill: "none",
                            stroke: "currentColor",
                            view_box: "0 0 24 24",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
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
