#![allow(dead_code)]

use crate::manifest::LoopCheckpoint;
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approve,
    Deny,
    Pending,
}

pub trait ApprovalController {
    fn request_approval(&mut self, checkpoint: &LoopCheckpoint) -> ApprovalDecision;
}

#[derive(Default)]
pub struct ScriptedApprovalController {
    decisions: BTreeMap<String, VecDeque<ApprovalDecision>>,
}

impl ScriptedApprovalController {
    pub fn push(&mut self, checkpoint_id: impl Into<String>, decision: ApprovalDecision) {
        self.decisions
            .entry(checkpoint_id.into())
            .or_default()
            .push_back(decision);
    }
}

impl ApprovalController for ScriptedApprovalController {
    fn request_approval(&mut self, checkpoint: &LoopCheckpoint) -> ApprovalDecision {
        self.decisions
            .get_mut(&checkpoint.id)
            .and_then(VecDeque::pop_front)
            .unwrap_or(ApprovalDecision::Approve)
    }
}
