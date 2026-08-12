//! Layer 3 — Presentation ("The Canvas").
//!
//! This crate composes the analyst-facing views. The panel layout types are
//! always available; the actual egui application is gated behind the `app`
//! feature so the workspace builds without a native windowing backend.

pub mod layout;

#[cfg(feature = "app")]
pub mod app;

pub use layout::{Panel, PanelLayout};

#[cfg(feature = "app")]
pub use app::run;
