//! Download page — shown when a user opens `/f/{file_id}`.
//!
//! Fetches file metadata via the [`get_file_info`] server function and
//! presents the filename, size, and expiry time before the user clicks the
//! download button.

use dioxus::prelude::*;

use crate::{api::get_file_info, app::Route};

/// Download page for `/f/{file_id}`.
#[component]
pub fn Download(file_id: String) -> Element {
    let fid = file_id.clone();
    let info = use_resource(move || {
        let id = fid.clone();
        async move { get_file_info(id).await }
    });

    rsx! {
        div { class: "page download-page",
            match &*info.read() {
                None => rsx! {
                    p { class: "loading-text", "Fetching file info" }
                },
                Some(Err(e)) => rsx! {
                    h2 { style: "font-size:1.2rem;font-weight:700;letter-spacing:0.1em;text-transform:uppercase;color:var(--text-bright);margin-bottom:0.5rem;",
                        "File not found"
                    }
                    p { class: "error", "{e}" }
                    Link { to: Route::Home {}, class: "not-found-link", "Back to home" }
                },
                Some(Ok(meta)) => rsx! {
                    div { class: "file-card",
                        div { class: "file-card-header",
                            div { class: "file-card-icon",
                                svg {
                                    fill: "none",
                                    stroke: "currentColor",
                                    view_box: "0 0 24 24",
                                    stroke_width: "1.5",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    style: "width:18px;height:18px",
                                    path { d: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" }
                                    path { d: "M14 2v6h6" }
                                }
                            }
                            span { class: "file-card-name", "{meta.filename}" }
                        }
                        div { class: "file-card-body",
                            div { class: "file-meta",
                                div { class: "file-meta-item",
                                    span { class: "file-meta-label", "size" }
                                    span { class: "file-meta-value", "{fmt_size(meta.size)}" }
                                }
                                div { class: "file-meta-item",
                                    span { class: "file-meta-label", "expires" }
                                    span { class: "file-meta-value", "{fmt_expiry(&meta.expires_at)}" }
                                }
                            }
                            a {
                                class: "btn download-btn",
                                href: "/f/{file_id}",
                                download: "{meta.filename}",
                                svg {
                                    fill: "none",
                                    stroke: "currentColor",
                                    view_box: "0 0 24 24",
                                    stroke_width: "2",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    style: "width:14px;height:14px",
                                    path { d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" }
                                    path { d: "M7 10l5 5 5-5" }
                                    path { d: "M12 15V3" }
                                }
                                "Download"
                            }
                        }
                    }
                },
            }
        }
    }
}

fn fmt_size(bytes: i64) -> String {
    const KB: i64 = 1_024;
    const MB: i64 = 1_024 * KB;
    const GB: i64 = 1_024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn fmt_expiry(dt: &chrono::DateTime<chrono::Utc>) -> String {
    let secs = (*dt - chrono::Utc::now()).num_seconds();
    if secs < 0 {
        "expired".to_owned()
    } else if secs < 3600 {
        format!("in {} min", secs / 60)
    } else if secs < 86_400 {
        format!("in {} h", secs / 3600)
    } else {
        format!("in {} days", secs / 86_400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_size_cases() {
        assert_eq!(fmt_size(512), "512 B");
        assert_eq!(fmt_size(2_048), "2.0 KB");
        assert_eq!(fmt_size(1_572_864), "1.5 MB");
        assert_eq!(fmt_size(1_073_741_824), "1.0 GB");
    }

    #[test]
    fn fmt_expiry_expired() {
        let past = chrono::Utc::now() - chrono::Duration::hours(1);
        assert_eq!(fmt_expiry(&past), "expired");
    }

    #[test]
    fn fmt_expiry_minutes() {
        let soon = chrono::Utc::now() + chrono::Duration::minutes(5);
        assert!(fmt_expiry(&soon).contains("min"));
    }

    #[test]
    fn fmt_expiry_days() {
        let later = chrono::Utc::now() + chrono::Duration::days(3);
        assert!(fmt_expiry(&later).contains("days"));
    }
}
