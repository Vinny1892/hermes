//! Server configuration page (ADMIN only).
//!
//! Storage defaults are editable at runtime (take effect immediately).
//! All other settings are read-only here — they must be changed via
//! `hermes.toml` or environment variables and require a server restart.

use std::collections::HashMap;

use dioxus::prelude::*;

use crate::api::{get_app_config, set_app_config};
#[allow(unused_imports)]
use crate::app::Route;

// Config key constants (mirrors server::config::keys — duplicated here to
// avoid importing server-only code into the WASM build)
mod keys {
    pub const STORAGE_DEFAULT_QUOTA: &str = "storage.default_quota";
    pub const STORAGE_DEFAULT_LOCAL_RATIO: &str = "storage.default_local_ratio";
    pub const SERVER_BASE_URL: &str = "server.base_url";
    pub const SERVER_LOG: &str = "server.log";
    pub const STORAGE_LOCAL_PATH: &str = "storage.local.path";
    pub const STORAGE_S3_BUCKET: &str = "storage.s3.bucket";
    pub const STORAGE_S3_REGION: &str = "storage.s3.region";
    pub const STORAGE_S3_ENDPOINT: &str = "storage.s3.endpoint";
    pub const STORAGE_S3_ACCESS_KEY_ID: &str = "storage.s3.access_key_id";
    pub const STORAGE_S3_SECRET_ACCESS_KEY: &str = "storage.s3.secret_access_key";
}

// ── Quota helpers (client-side, no server deps) ───────────────────────────────

/// Parse a human-readable quota string → bytes. Returns `None` for unlimited.
fn parse_quota_bytes(s: &str) -> Option<u64> {
    let s = s.trim();
    if s == "0" || s.eq_ignore_ascii_case("unlimited") {
        return None;
    }
    let split = s.chars().position(|c| c.is_alphabetic()).unwrap_or(s.len());
    let n: u64 = s[..split].parse().ok()?;
    let mult: u64 = match s[split..].to_ascii_uppercase().as_str() {
        "" | "B"      => 1,
        "KB" | "K"    => 1_024,
        "MB" | "M"    => 1_024 * 1_024,
        "GB" | "G"    => 1_024 * 1_024 * 1_024,
        "TB" | "T"    => 1_024 * 1_024 * 1_024 * 1_024,
        _             => return None,
    };
    Some(n * mult)
}

fn format_bytes(b: u64) -> String {
    const TB: u64 = 1_024 * 1_024 * 1_024 * 1_024;
    const GB: u64 = 1_024 * 1_024 * 1_024;
    const MB: u64 = 1_024 * 1_024;
    const KB: u64 = 1_024;
    if b == 0                { "0 B".to_owned() }
    else if b.is_multiple_of(TB) { format!("{} TB", b / TB) }
    else if b.is_multiple_of(GB) { format!("{} GB", b / GB) }
    else if b.is_multiple_of(MB) { format!("{} MB", b / MB) }
    else if b.is_multiple_of(KB) { format!("{} KB", b / KB) }
    else if b >= TB          { format!("{:.2} TB", b as f64 / TB as f64) }
    else if b >= GB          { format!("{:.2} GB", b as f64 / GB as f64) }
    else if b >= MB          { format!("{:.2} MB", b as f64 / MB as f64) }
    else if b >= KB          { format!("{:.2} KB", b as f64 / KB as f64) }
    else                     { format!("{} B", b) }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // parse_quota_bytes
    #[test]
    fn parse_quota_unlimited() {
        assert_eq!(parse_quota_bytes("0"), None);
        assert_eq!(parse_quota_bytes("unlimited"), None);
        assert_eq!(parse_quota_bytes("UNLIMITED"), None);
    }

    #[test]
    fn parse_quota_sizes() {
        assert_eq!(parse_quota_bytes("1GB"), Some(1_073_741_824));
        assert_eq!(parse_quota_bytes("500MB"), Some(524_288_000));
        assert_eq!(
            parse_quota_bytes("2TB"),
            Some(2 * 1_024 * 1_024 * 1_024 * 1_024),
        );
    }

    #[test]
    fn parse_quota_invalid_returns_none() {
        assert_eq!(parse_quota_bytes("abc"), None);
        assert_eq!(parse_quota_bytes("100PB"), None);
        assert_eq!(parse_quota_bytes(""), None);
    }

    // format_bytes
    #[test]
    fn format_exact_powers() {
        assert_eq!(format_bytes(1_024), "1 KB");
        assert_eq!(format_bytes(1_024 * 1_024), "1 MB");
        assert_eq!(format_bytes(1_024 * 1_024 * 1_024), "1 GB");
        assert_eq!(format_bytes(1_024u64 * 1_024 * 1_024 * 1_024), "1 TB");
    }

    #[test]
    fn format_zero() {
        assert_eq!(format_bytes(0), "0 B");
    }

    #[test]
    fn format_fractional_gb() {
        // Value >= 1 GB but not divisible by GB/MB/KB → "X.XX GB"
        let b = 1_073_741_824u64 + 1; // 1 GB + 1 byte
        let s = format_bytes(b);
        assert!(s.ends_with("GB"), "expected GB suffix, got: {s}");
    }

    #[test]
    fn format_exact_multiples_prefer_larger_unit() {
        // 2 GB should not be shown as MB
        assert_eq!(format_bytes(2 * 1_024 * 1_024 * 1_024), "2 GB");
    }
}

// ── Field metadata ────────────────────────────────────────────────────────────

struct FieldMeta {
    label: &'static str,
    hint: &'static str,
    secret: bool,
    suffix: &'static str,
}

fn field_meta(key: &str) -> FieldMeta {
    match key {
        keys::STORAGE_DEFAULT_QUOTA => FieldMeta {
            label: "Default quota",
            hint: "e.g. 1GB, 500MB, 2TB, unlimited",
            secret: false,
            suffix: "",
        },
        keys::STORAGE_DEFAULT_LOCAL_RATIO => FieldMeta {
            label: "Local ratio (0–100)",
            hint: "",
            secret: false,
            suffix: "\u{2009}%",
        },
        keys::SERVER_BASE_URL => FieldMeta {
            label: "Base URL",
            hint: "",
            secret: false,
            suffix: "",
        },
        keys::SERVER_LOG => FieldMeta {
            label: "Log level",
            hint: "",
            secret: false,
            suffix: "",
        },
        keys::STORAGE_LOCAL_PATH => FieldMeta {
            label: "Upload directory",
            hint: "",
            secret: false,
            suffix: "",
        },
        keys::STORAGE_S3_BUCKET => FieldMeta { label: "Bucket", hint: "", secret: false, suffix: "" },
        keys::STORAGE_S3_REGION => FieldMeta { label: "Region", hint: "", secret: false, suffix: "" },
        keys::STORAGE_S3_ENDPOINT => FieldMeta { label: "Endpoint", hint: "", secret: false, suffix: "" },
        keys::STORAGE_S3_ACCESS_KEY_ID => FieldMeta { label: "Access key ID", hint: "", secret: false, suffix: "" },
        keys::STORAGE_S3_SECRET_ACCESS_KEY => FieldMeta { label: "Secret access key", hint: "", secret: true, suffix: "" },
        _ => FieldMeta { label: "unknown", hint: "", secret: false, suffix: "" },
    }
}

// ── Section definitions ───────────────────────────────────────────────────────

struct Section {
    title: &'static str,
    keys: &'static [&'static str],
    /// True = editable in the UI. False = read-only, changed via TOML/env.
    editable: bool,
}

const SECTIONS: &[Section] = &[
    Section {
        title: "Storage Defaults",
        keys: &[keys::STORAGE_DEFAULT_QUOTA, keys::STORAGE_DEFAULT_LOCAL_RATIO],
        editable: true,
    },
    Section {
        title: "Server",
        keys: &[keys::SERVER_BASE_URL, keys::SERVER_LOG],
        editable: false,
    },
    Section {
        title: "Local Backend",
        keys: &[keys::STORAGE_LOCAL_PATH],
        editable: false,
    },
    Section {
        title: "S3 Backend",
        keys: &[
            keys::STORAGE_S3_BUCKET,
            keys::STORAGE_S3_REGION,
            keys::STORAGE_S3_ENDPOINT,
            keys::STORAGE_S3_ACCESS_KEY_ID,
            keys::STORAGE_S3_SECRET_ACCESS_KEY,
        ],
        editable: false,
    },
];

// ── Page ──────────────────────────────────────────────────────────────────────

#[component]
pub fn Settings() -> Element {
    let _nav = use_navigator();

    #[cfg(target_arch = "wasm32")]
    {
        let role = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item("hermes-role").ok().flatten())
            .unwrap_or_default();
        if role != "ADMIN" {
            _nav.push(Route::Home {});
            return rsx! { div {} };
        }
    }

    let config_res = use_resource(get_app_config);
    let mut edits: Signal<HashMap<String, String>> = use_signal(HashMap::new);

    use_effect(move || {
        if let Some(Ok(entries)) = config_res.read().as_ref() {
            let mut map = HashMap::new();
            for e in entries {
                map.insert(e.key.clone(), e.value.clone());
            }
            edits.set(map);
        }
    });

    #[allow(unused_mut)]
    let mut save_state: Signal<HashMap<&'static str, Option<Result<(), String>>>> =
        use_signal(HashMap::new);

    rsx! {
        div { class: "page-wrapper",
            div { class: "max-w-2xl mx-auto px-4 py-10 flex flex-col gap-8",

                // Header
                div { class: "flex flex-col gap-1",
                    div { class: "flex items-center gap-[0.55rem] mb-1",
                        span { class: "w-[6px] h-[6px] rounded-full bg-[var(--accent)] shadow-[0_0_8px_var(--accent)] shrink-0" }
                        span { class: "text-[0.7rem] tracking-[0.2em] uppercase text-[var(--text-muted)]",
                            "ADMIN PANEL"
                        }
                    }
                    h1 { class: "text-[1.4rem] font-medium tracking-[0.08em] text-[var(--text-bright)]",
                        "SERVER CONFIGURATION"
                    }
                }

                match config_res.read().as_ref() {
                    None => rsx! {
                        div { class: "flex items-center gap-3 text-[var(--text-muted)] text-[0.8rem]",
                            span { class: "login-spinner" }
                            "Loading configuration…"
                        }
                    },
                    Some(Err(e)) => rsx! {
                        div { class: "login-error",
                            svg {
                                view_box: "0 0 16 16", fill: "none", stroke: "currentColor",
                                stroke_width: "1.5", style: "width:15px;height:15px;flex-shrink:0",
                                circle { cx: "8", cy: "8", r: "7" }
                                path { d: "M8 5v3M8 10.5v.5" }
                            }
                            span { "Failed to load config: {e}" }
                        }
                    },
                    Some(Ok(_)) => rsx! {
                        for section in SECTIONS {
                            if section.keys.iter().any(|k| edits.read().contains_key(*k)) {
                                if section.editable {
                                    EditableSection {
                                        title: section.title,
                                        section_keys: section.keys,
                                        edits,
                                        save_state,
                                    }
                                } else {
                                    ReadonlySection {
                                        title: section.title,
                                        section_keys: section.keys,
                                        edits,
                                    }
                                }
                            }
                        }
                    },
                }
            }
        }
    }
}

// ── Editable section ──────────────────────────────────────────────────────────

#[component]
fn EditableSection(
    title: &'static str,
    section_keys: &'static [&'static str],
    mut edits: Signal<HashMap<String, String>>,
    mut save_state: Signal<HashMap<&'static str, Option<Result<(), String>>>>,
) -> Element {
    let visible_keys: Vec<&'static str> = section_keys
        .iter()
        .copied()
        .filter(|k| edits.read().contains_key(*k))
        .collect();

    let state = save_state.read().get(title).cloned().flatten();
    let keys_for_save = visible_keys.clone();

    let save = move |_| {
        let pairs: Vec<(String, String)> = keys_for_save
            .iter()
            .filter_map(|k| edits.read().get(*k).map(|v| (k.to_string(), v.clone())))
            .collect();

        save_state.write().insert(title, None);

        spawn(async move {
            for (key, value) in pairs {
                if let Err(e) = set_app_config(key, value).await {
                    let raw = e.to_string();
                    let msg = raw.strip_prefix("ServerFnError: ").unwrap_or(&raw).to_owned();
                    save_state.write().insert(title, Some(Err(msg)));
                    return;
                }
            }
            save_state.write().insert(title, Some(Ok(())));
        });
    };

    rsx! {
        div { class: "bg-[var(--surface)] border border-[var(--border)] rounded-lg p-6 flex flex-col gap-5 hover:border-[var(--border-lit)] transition-colors",
            h2 { class: "text-[0.82rem] font-medium tracking-[0.18em] uppercase text-[var(--text-bright)]",
                "{title}"
            }

            div { class: "flex flex-col gap-4",
                for key in &visible_keys {
                    SettingsField { field_key: key, edits, readonly: false }
                }
            }

            QuotaBreakdown { edits }

            div { class: "flex items-center justify-between pt-4 border-t border-[var(--border)]",
                div { class: "text-[0.75rem]",
                    match &state {
                        Some(Ok(())) => rsx! { span { class: "text-[var(--accent)]", "✓ Saved" } },
                        Some(Err(msg)) => rsx! { span { class: "text-[var(--error)]", "{msg}" } },
                        None => rsx! { span {} },
                    }
                }
                button {
                    class: "bg-[var(--accent-dim)] border border-[var(--accent)] text-[var(--accent)] font-medium text-[0.72rem] tracking-[0.15em] uppercase px-4 py-[0.4rem] rounded cursor-pointer hover:bg-[rgba(110,114,251,0.2)] hover:shadow-[0_0_16px_rgba(110,114,251,0.2)] transition-all [[data-theme=light]_&]:hover:bg-[rgba(47,40,148,0.12)]",
                    onclick: save,
                    "Save"
                }
            }
        }
    }
}

// ── Read-only section ─────────────────────────────────────────────────────────

#[component]
fn ReadonlySection(
    title: &'static str,
    section_keys: &'static [&'static str],
    edits: Signal<HashMap<String, String>>,
) -> Element {
    let visible_keys: Vec<&'static str> = section_keys
        .iter()
        .copied()
        .filter(|k| edits.read().contains_key(*k))
        .collect();

    rsx! {
        div { class: "bg-[var(--surface)] border border-[var(--border)] rounded-lg p-6 flex flex-col gap-5 opacity-80",
            // Header
            div { class: "flex items-center gap-2",
                h2 { class: "text-[0.82rem] font-medium tracking-[0.18em] uppercase text-[var(--text-bright)]",
                    "{title}"
                }
                span { class: "text-[0.62rem] tracking-[0.12em] uppercase text-[var(--text-muted)] bg-[var(--surface-2)] border border-[var(--border)] rounded px-[0.4rem] py-[0.12rem]",
                    "read only"
                }
            }

            // Fields (disabled)
            div { class: "flex flex-col gap-4",
                for key in &visible_keys {
                    SettingsField { field_key: key, edits, readonly: true }
                }
            }

            // Informative footer
            div { class: "flex items-start gap-2 pt-4 border-t border-[var(--border)] text-[0.75rem] text-[var(--text-muted)]",
                svg {
                    view_box: "0 0 16 16", fill: "none", stroke: "currentColor",
                    stroke_width: "1.5", style: "width:14px;height:14px;flex-shrink:0;margin-top:1px",
                    circle { cx: "8", cy: "8", r: "7" }
                    path { d: "M8 7v4M8 5.5v.5" }
                }
                span {
                    "To change these values, edit "
                    code { class: "text-[var(--accent)] text-[0.72rem]", "hermes.toml" }
                    " or set the corresponding environment variable, then restart the server."
                }
            }
        }
    }
}

// ── Quota breakdown preview ───────────────────────────────────────────────────

#[component]
fn QuotaBreakdown(edits: Signal<HashMap<String, String>>) -> Element {
    let quota_str = edits.read()
        .get(keys::STORAGE_DEFAULT_QUOTA)
        .cloned()
        .unwrap_or_default();
    let ratio_str = edits.read()
        .get(keys::STORAGE_DEFAULT_LOCAL_RATIO)
        .cloned()
        .unwrap_or_default();

    let ratio = ratio_str.parse::<u8>().unwrap_or(100).clamp(0, 100);
    let quota_bytes = parse_quota_bytes(&quota_str);

    let (local_label, s3_label) = match quota_bytes {
        None => ("unlimited".to_owned(), "unlimited".to_owned()),
        Some(total) => {
            let local = total * ratio as u64 / 100;
            let s3    = total - local;
            (format_bytes(local), format_bytes(s3))
        }
    };

    let local_pct = ratio;
    let s3_pct    = 100u8.saturating_sub(ratio);

    rsx! {
        div { class: "flex flex-col gap-2 bg-[var(--surface-2)] border border-[var(--border)] rounded p-4",
            // Label row
            div { class: "flex justify-between text-[0.68rem] tracking-[0.15em] uppercase text-[var(--text-muted)] mb-1",
                span { "Allocation preview" }
            }

            // Split bar
            div { class: "flex h-[6px] rounded overflow-hidden w-full",
                div {
                    class: "bg-[var(--accent)] transition-all duration-200",
                    style: "width: {local_pct}%",
                }
                div {
                    class: "bg-[var(--info)] transition-all duration-200",
                    style: "width: {s3_pct}%",
                }
            }

            // Legend
            div { class: "flex gap-6 mt-1",
                div { class: "flex flex-col gap-[0.2rem]",
                    div { class: "flex items-center gap-[0.4rem]",
                        span { class: "w-[8px] h-[8px] rounded-sm bg-[var(--accent)] shrink-0" }
                        span { class: "text-[0.7rem] tracking-[0.1em] uppercase text-[var(--text-muted)]", "Local" }
                    }
                    span { class: "text-[0.85rem] font-medium text-[var(--text-bright)]",
                        "{local_label}"
                    }
                    span { class: "text-[0.68rem] text-[var(--text-muted)]", "{local_pct}%" }
                }
                div { class: "w-px bg-[var(--border)] self-stretch" }
                div { class: "flex flex-col gap-[0.2rem]",
                    div { class: "flex items-center gap-[0.4rem]",
                        span { class: "w-[8px] h-[8px] rounded-sm bg-[var(--info)] shrink-0" }
                        span { class: "text-[0.7rem] tracking-[0.1em] uppercase text-[var(--text-muted)]", "S3" }
                    }
                    span { class: "text-[0.85rem] font-medium text-[var(--text-bright)]",
                        "{s3_label}"
                    }
                    span { class: "text-[0.68rem] text-[var(--text-muted)]", "{s3_pct}%" }
                }
            }
        }
    }
}

// ── Field ─────────────────────────────────────────────────────────────────────

#[component]
fn SettingsField(
    field_key: &'static str,
    edits: Signal<HashMap<String, String>>,
    readonly: bool,
) -> Element {
    let meta = field_meta(field_key);
    let value = edits.read().get(field_key).cloned().unwrap_or_default();

    let has_suffix = !meta.suffix.is_empty();

    let input_class = if readonly {
        "w-full bg-[var(--surface-2)] border border-[var(--border)] rounded text-[var(--text-muted)] font-[var(--font)] text-[0.85rem] px-3 py-[0.55rem] outline-none cursor-not-allowed opacity-60"
    } else {
        "w-full bg-[var(--surface-2)] border border-[var(--border)] rounded text-[var(--text)] font-[var(--font)] text-[0.85rem] px-3 py-[0.55rem] outline-none focus:border-[var(--accent)] focus:shadow-[0_0_0_2px_var(--accent-dim)] transition-all"
    };

    let display_value = if has_suffix {
        format!("{value}{}", meta.suffix)
    } else {
        value.clone()
    };

    rsx! {
        div { class: "flex flex-col gap-[0.45rem]",
            label {
                class: "text-[0.7rem] font-medium tracking-[0.18em] uppercase text-[var(--text-muted)]",
                r#for: field_key,
                "{meta.label}"
            }
            input {
                id: field_key,
                class: input_class,
                r#type: if meta.secret { "password" } else { "text" },
                value: "{display_value}",
                readonly,
                spellcheck: false,
                oninput: move |e| {
                    if !readonly {
                        let raw = e.value();
                        let clean = if has_suffix {
                            raw.chars().filter(|c| c.is_ascii_digit()).collect::<String>()
                        } else {
                            raw
                        };
                        edits.write().insert(field_key.to_string(), clean);
                    }
                },
            }
            if !meta.hint.is_empty() {
                span { class: "text-[0.7rem] text-[var(--text-muted)] opacity-55",
                    "{meta.hint}"
                }
            }
        }
    }
}
