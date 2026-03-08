//! Upload/download progress bar component.

use dioxus::prelude::*;

/// Props for [`ProgressBar`].
#[derive(Props, Clone, PartialEq)]
pub struct ProgressBarProps {
    /// Progress from 0.0 to 1.0.
    pub value: f64,
    /// Optional label displayed inside the bar.
    #[props(default)]
    pub label: Option<String>,
}

/// A simple styled progress bar.
///
/// `value` must be in `[0.0, 1.0]`. Values outside this range are clamped.
///
/// ```rust
/// # use dioxus::prelude::*;
/// # use hermes::components::progress::ProgressBar;
/// rsx! {
///     ProgressBar { value: 0.6, label: Some("60%".to_owned()) }
/// };
/// ```
#[component]
pub fn ProgressBar(props: ProgressBarProps) -> Element {
    let pct = (props.value.clamp(0.0, 1.0) * 100.0).round() as u32;
    let width = format!("{pct}%");

    rsx! {
        div { class: "progress-track",
            div {
                class: "progress-fill",
                style: "width: {width}",
                role: "progressbar",
                aria_valuenow: "{pct}",
                aria_valuemin: "0",
                aria_valuemax: "100",
                if let Some(label) = &props.label {
                    span { class: "progress-label", "{label}" }
                }
            }
        }
    }
}
