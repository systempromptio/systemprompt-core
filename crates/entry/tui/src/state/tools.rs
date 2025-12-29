use uuid::Uuid;

use crate::tools::PendingToolCall;

#[derive(Debug)]
pub struct ToolsState {
    pub pending_approvals: Vec<PendingApproval>,
    pub execution_history: Vec<ToolExecution>,
}

impl ToolsState {
    pub const fn new() -> Self {
        Self {
            pending_approvals: Vec::new(),
            execution_history: Vec::new(),
        }
    }

    pub fn add_pending(&mut self, tool_call: PendingToolCall) {
        self.pending_approvals.push(PendingApproval {
            tool_call,
            selected_action: ApprovalAction::Approve,
        });
    }

    pub fn current_pending(&self) -> Option<&PendingApproval> {
        self.pending_approvals.first()
    }

    pub fn approve_current(&mut self) -> Option<PendingToolCall> {
        if self.pending_approvals.is_empty() {
            None
        } else {
            let approval = self.pending_approvals.remove(0);
            self.execution_history.push(ToolExecution {
                id: approval.tool_call.id,
                tool_name: approval.tool_call.tool_name.clone(),
                status: ExecutionStatus::Executing,
                result: None,
            });
            Some(approval.tool_call)
        }
    }

    pub fn reject_current(&mut self) -> Option<Uuid> {
        if self.pending_approvals.is_empty() {
            None
        } else {
            let approval = self.pending_approvals.remove(0);
            let id = approval.tool_call.id;
            self.execution_history.push(ToolExecution {
                id,
                tool_name: approval.tool_call.tool_name,
                status: ExecutionStatus::Rejected,
                result: None,
            });
            Some(id)
        }
    }

    pub fn approve(&mut self, id: Uuid) -> Option<PendingToolCall> {
        if let Some(pos) = self
            .pending_approvals
            .iter()
            .position(|p| p.tool_call.id == id)
        {
            let approval = self.pending_approvals.remove(pos);
            self.execution_history.push(ToolExecution {
                id: approval.tool_call.id,
                tool_name: approval.tool_call.tool_name.clone(),
                status: ExecutionStatus::Executing,
                result: None,
            });
            Some(approval.tool_call)
        } else {
            None
        }
    }

    pub fn reject(&mut self, id: Uuid) -> Option<Uuid> {
        if let Some(pos) = self
            .pending_approvals
            .iter()
            .position(|p| p.tool_call.id == id)
        {
            let approval = self.pending_approvals.remove(pos);
            let tool_id = approval.tool_call.id;
            self.execution_history.push(ToolExecution {
                id: tool_id,
                tool_name: approval.tool_call.tool_name,
                status: ExecutionStatus::Rejected,
                result: None,
            });
            Some(tool_id)
        } else {
            None
        }
    }

    pub fn get_pending(&self, id: Uuid) -> Option<&PendingToolCall> {
        self.pending_approvals
            .iter()
            .find(|p| p.tool_call.id == id)
            .map(|p| &p.tool_call)
    }

    pub fn complete_execution(&mut self, id: Uuid, success: bool, result: Option<String>) {
        if let Some(exec) = self.execution_history.iter_mut().find(|e| e.id == id) {
            exec.status = if success {
                ExecutionStatus::Completed
            } else {
                ExecutionStatus::Failed
            };
            exec.result = result;
        }
    }

    pub fn cycle_action(&mut self) {
        if let Some(pending) = self.pending_approvals.first_mut() {
            pending.selected_action = match pending.selected_action {
                ApprovalAction::Approve => ApprovalAction::Reject,
                ApprovalAction::Reject => ApprovalAction::Edit,
                ApprovalAction::Edit => ApprovalAction::Approve,
            };
        }
    }
}

impl Default for ToolsState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct PendingApproval {
    pub tool_call: PendingToolCall,
    pub selected_action: ApprovalAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalAction {
    Approve,
    Reject,
    Edit,
}

#[derive(Debug, Clone)]
pub struct ToolExecution {
    pub id: Uuid,
    pub tool_name: String,
    pub status: ExecutionStatus,
    pub result: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStatus {
    Executing,
    Completed,
    Failed,
    Rejected,
}
