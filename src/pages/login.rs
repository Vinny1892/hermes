//! Login page — standalone full-screen, no navbar.

use dioxus::prelude::*;
use dioxus::document::eval;

use crate::api::login_user;
use crate::app::Route;

/// Full-screen login page (outside the Navbar layout).
///
/// On success: token + role → `localStorage`, then navigate to [`Route::Home`].
#[component]
pub fn Login() -> Element {
    let mut email    = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error    = use_signal(|| Option::<String>::None);
    let mut loading  = use_signal(|| false);
    let mut is_light = use_signal(|| false);
    let nav = use_navigator();

    #[cfg(target_arch = "wasm32")]
    {
        let already_logged_in = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item("hermes-token").ok().flatten())
            .is_some();
        if already_logged_in {
            if let Some(win) = web_sys::window() {
                let _ = win.location().replace("/");
            }
            return rsx! { div {} };
        }
    }

    // Restore saved theme on mount
    // Sync button icon with the theme already applied by THEME_INIT
    use_effect(move || {
        spawn(async move {
            let mut ev = eval("dioxus.send(localStorage.getItem('hermes-theme')==='light');");
            if let Ok(light) = ev.recv::<bool>().await {
                is_light.set(light);
            }
        });
    });

    let toggle_theme = move |_| {
        let next = !*is_light.read();
        is_light.set(next);
        spawn(async move {
            let (set_theme, store_val) = if next {
                ("document.documentElement.setAttribute('data-theme','light')", "light")
            } else {
                ("document.documentElement.removeAttribute('data-theme')", "dark")
            };
            let js = format!(
                "const a=()=>{{ {set_theme}; localStorage.setItem('hermes-theme','{store_val}'); }};\
                 document.startViewTransition ? document.startViewTransition(a) : a();"
            );
            let _ = eval(&js);
        });
    };

    let handle_submit = move |evt: Event<FormData>| {
        evt.prevent_default();

        let email_val = email.read().clone();
        let pass_val  = password.read().clone();

        if email_val.is_empty() || pass_val.is_empty() {
            error.set(Some("all fields are required".to_string()));
            return;
        }

        loading.set(true);
        error.set(None);

        let nav = nav.clone();
        spawn(async move {
            match login_user(email_val, pass_val).await {
                Ok(resp) => {
                    #[cfg(target_arch = "wasm32")]
                    {
                        use dioxus::document::eval;
                        let token = resp.token.clone();
                        let role  = resp.role.as_str();
                        let mut ev = eval(&format!(
                            "localStorage.setItem('hermes-token','{token}');\
                             localStorage.setItem('hermes-role','{role}');\
                             dioxus.send(true);"
                        ));
                        let _ = ev.recv::<bool>().await;
                    }
                    drop(resp);
                    nav.push(Route::Home {});
                }
                Err(e) => {
                    let raw = e.to_string();
                    let msg = raw
                        .strip_prefix("error running server function: ")
                        .or_else(|| raw.strip_prefix("ServerFnError: "))
                        .unwrap_or(&raw)
                        .trim_end_matches(" (details: None)")
                        .to_string();
                    error.set(Some(msg));
                    loading.set(false);
                }
            }
        });
    };

    let is_loading = *loading.read();

    rsx! {
        div { class: "relative min-h-screen flex flex-col items-center justify-center px-8 py-12 gap-6 overflow-hidden",

            // ── Theme toggle (top-right corner) ────────────────────────────
            button {
                class: "theme-toggle fixed top-[1.2rem] right-[1.5rem] z-10",
                onclick: toggle_theme,
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

            // ── Ambient signal rings ────────────────────────────────────────
            div { class: "login-ambient",
                div { class: "login-ring login-ring-1" }
                div { class: "login-ring login-ring-2" }
                div { class: "login-ring login-ring-3" }
            }

            // ── Brand mark (replaces navbar on the login page) ──────────────
            div { class: "flex items-center gap-0 z-[1] [animation:fade-up_0.35s_ease_both]",
                span { class: "login-brand-name", "HERMES" }
                span { class: "navbar-brand-cursor" }
            }
            p { class: "text-[0.75rem] tracking-[0.2em] uppercase text-[var(--text-muted)] z-[1] [animation:fade-up_0.35s_ease_0.05s_both]", "P2P SECURE FILE TRANSFER" }

            // ── Card ───────────────────────────────────────────────────────
            div { class: "login-card",

                // Signal status row
                div { class: "flex items-center gap-[0.55rem] mb-[1.6rem]",
                    span { class: "w-[6px] h-[6px] rounded-full bg-[var(--accent)] shadow-[0_0_8px_var(--accent)] shrink-0" }
                    span { class: "text-[0.75rem] tracking-[0.2em] uppercase text-[var(--text-muted)]", "TRANSMISSION SECURED" }
                }

                h1 { class: "login-title", "AUTHENTICATE" }
                p  { class: "text-[0.85rem] text-[var(--text-muted)] tracking-[0.12em] uppercase mb-[0.2rem]", "identity verification required" }

                form {
                    class: "flex flex-col gap-8 mt-8",
                    onsubmit: handle_submit,

                    div { class: "flex flex-col gap-[0.65rem] [animation:fade-up_0.4s_ease_0.12s_both]",
                        label { class: "text-[0.75rem] font-medium tracking-[0.2em] uppercase text-[var(--text-muted)]", r#for: "l-email", "EMAIL ADDRESS" }
                        input {
                            id: "l-email",
                            class: "login-input",
                            r#type: "email",
                            placeholder: "user@domain.com",
                            autocomplete: "email",
                            spellcheck: false,
                            value: "{email}",
                            oninput: move |e| email.set(e.value()),
                            disabled: is_loading,
                        }
                    }

                    div { class: "flex flex-col gap-[0.65rem] [animation:fade-up_0.4s_ease_0.2s_both]",
                        label { class: "text-[0.75rem] font-medium tracking-[0.2em] uppercase text-[var(--text-muted)]", r#for: "l-pass", "PASSPHRASE" }
                        input {
                            id: "l-pass",
                            class: "login-input",
                            r#type: "password",
                            placeholder: "············",
                            autocomplete: "current-password",
                            value: "{password}",
                            oninput: move |e| password.set(e.value()),
                            disabled: is_loading,
                        }
                    }

                    if let Some(ref msg) = *error.read() {
                        div { class: "login-error",
                            svg {
                                view_box: "0 0 16 16",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "1.5",
                                style: "width:15px;height:15px;flex-shrink:0;margin-top:1px",
                                circle { cx: "8", cy: "8", r: "7" }
                                path { d: "M8 5v3M8 10.5v.5" }
                            }
                            span { "{msg}" }
                        }
                    }

                    button {
                        class: if is_loading { "login-btn login-btn--busy" } else { "login-btn" },
                        r#type: "submit",
                        disabled: is_loading,
                        span { class: "login-btn-content",
                            if is_loading {
                                span { class: "login-spinner" }
                                "AUTHENTICATING"
                                span { class: "login-dots" }
                            } else {
                                span { class: "login-btn-arrow", "▶" }
                                "INITIATE CONNECTION"
                            }
                        }
                        span { class: "login-scanline" }
                    }
                }

                div { class: "mt-10 pt-[1.4rem] border-t border-[var(--border)] text-[0.68rem] tracking-[0.2em] uppercase text-[var(--text-muted)] opacity-45 text-center [animation:fade-up_0.4s_ease_0.35s_both]",
                    "HERMES // SECURE FILE TRANSFER PROTOCOL"
                }
            }

            // ── Page footer ─────────────────────────────────────────────────
            footer { class: "text-[0.65rem] tracking-[0.18em] uppercase text-[var(--text-muted)] opacity-30 z-[1] [animation:fade-up_0.4s_ease_0.4s_both]",
                "© 2026 HERMES"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // Mirrors the error-cleaning logic in the Err branch of handle_submit.
    fn clean(raw: &str) -> String {
        raw.strip_prefix("error running server function: ")
            .or_else(|| raw.strip_prefix("ServerFnError: "))
            .unwrap_or(raw)
            .trim_end_matches(" (details: None)")
            .to_string()
    }

    #[test]
    fn strips_dioxus_wrapper_and_details_suffix() {
        assert_eq!(
            clean("error running server function: invalid email or password (details: None)"),
            "invalid email or password",
        );
    }

    #[test]
    fn strips_server_fn_error_prefix() {
        assert_eq!(clean("ServerFnError: some error"), "some error");
    }

    #[test]
    fn passes_through_already_clean_message() {
        assert_eq!(clean("invalid email or password"), "invalid email or password");
    }

    #[test]
    fn strips_details_none_without_prefix() {
        assert_eq!(
            clean("service unavailable, please try again (details: None)"),
            "service unavailable, please try again",
        );
    }
}
