//! What the Craft apps can do with AI, and what a computer still needs to do it.
//!
//! The launcher shows [`Feature`]s, not providers. Each feature names the tools it powers in the
//! apps, a local recipe (model files and node packs for ComfyUI) where one has been worked out,
//! and hosted options that have an adapter. [`plan`] compares a recipe with an [`Inventory`] of
//! what the machine has and says what is left: ready, a download of a stated size, a licence to
//! accept, or a reason it cannot run.
//!
//! Pure data and logic, serde only, so it compiles for wasm. Scanning the disk and downloading
//! belong to the native side.

#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod catalogue;
mod engine;
mod feature;
pub mod firefly;
mod options;
mod plan;
mod png;
mod recipe;
mod sample;
mod setup;

pub use catalogue::{FLORENCE2_LARGE_DIR, builtin};
pub use engine::{
    EngineAdvice, EngineOffer, Gpu, GpuVendor, Os, Pace, SystemProfile, advise, gpu_route,
};
pub use feature::{AppStatus, AppUse, Feature};
pub use firefly::{
    C2PA_ACTION_COLOR_ADJUSTMENTS, C2PA_ACTION_CREATED, C2PA_ACTION_DUBBED, C2PA_ACTION_EDITED,
    C2paProvenance, CraftHostApp, CreativeAiTask, CreditEstimate, CreditTier, CreditWorkload,
    ExecutionCapability, IPTC_ALGORITHMICALLY_ENHANCED,
    IPTC_COMPOSITE_WITH_TRAINED_ALGORITHMIC_MEDIA, IPTC_TRAINED_ALGORITHMIC_MEDIA, LocalEngine,
    TaskCapabilityEntry, capability_matrix, estimate_generative_credits, generate_c2pa_provenance,
};
pub use options::{Route, SetupOption, setup_options};
pub use plan::{Blocker, InstalledFile, Inventory, Plan, State, plan, plan_native, plans};
pub use recipe::{
    CloudOption, Component, Licence, LocalRecipe, ModelFile, NativeRoute, NodePack, Source,
};
pub use sample::{sample_is_costly, sample_request, sample_request_for};
pub use setup::{
    Agreement, COMFYUI_TAG, PortableBuild, SetupOffer, SetupRoute, SetupStep, StepKind, agreement,
    agreement_for, gigabytes, is_official_build, portable_for, setup_offer, setup_offer_native,
};
