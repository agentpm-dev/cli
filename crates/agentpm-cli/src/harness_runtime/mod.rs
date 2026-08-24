#![allow(dead_code)]

pub(crate) mod action;
pub(crate) mod approval;
pub(crate) mod model;
pub(crate) mod provider;

pub use action::{ActionDispatchResult, ActionDispatcher, SemanticAction};
pub use approval::{ApprovalController, ApprovalDecision};
pub use model::{
    CapabilityDescriptor, ConsumerContextSnapshot, ModelProviderSelection, ModelRequest,
    ModelRuntime, PackageSnapshot, PromptAssemblyInput, RuntimeSnapshot, ServiceReadinessSnapshot,
    TranscriptEntry, TranscriptEntryKind, assemble_logical_prompt,
};
pub use provider::BuiltInModelRuntime;
