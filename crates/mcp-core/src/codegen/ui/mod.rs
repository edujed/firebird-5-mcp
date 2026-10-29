//! UI/form generation targets.
//!
//! These targets generate user-interface artifacts (forms, pages) rather
//! than data-layer code (entities, DTOs). They live in a separate
//! sub-module because they have different field models and generation
//! signatures than the data targets in [`super::targets`].

/// Lazarus/Delphi form generation (.lfm files).
pub mod lazarus;
/// Svelte 5 page and form generation.
pub mod svelte;
