//! Per-modality prompt box state (selected account, model, and options),
//! persisted so it survives restarts. See [`promptbox_state::PromptboxState`].

pub mod modality_promptbox_state;
pub mod promptbox_modality;
pub mod promptbox_state;
pub mod promptbox_state_manager;
