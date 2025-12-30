//! Unit tests for systemprompt-core-tui crate
//!
//! Tests cover:
//! - AppState initialization and tab navigation
//! - TuiModeInfo environment and display methods
//! - SseStatus and InitStatus states
//! - Chat types and formatting helpers (format_duration, truncate_text,
//!   short_id)
//! - Agent types (AgentInfo, AgentConnectionStatus)
//! - Tools state (ToolsState approval/rejection workflow)
//! - Conversations state (ConversationsState navigation and editing)
//! - Tool registry (ToolRegistry registration and lookup)
//! - RiskLevel and ToolResult types

#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::expect_used)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::unreadable_literal)]

use chrono::Utc;
use systemprompt_core_tui::state::{
    format_duration, short_id, truncate_text, ActiveTab, AgentConnectionStatus, AgentInfo,
    ApprovalAction, ConversationDisplay, ConversationsState, ExecutionStatus, FocusedPanel,
    InitStatus, InputMode, InputType, LoadingState, SseStatus, SystemInstructionsSource,
    TaskMetadataDisplay, ToolCallStatus,
};
use systemprompt_core_tui::tools::{RiskLevel, ToolResult};
use systemprompt_identifiers::ContextId;

// ============================================================================
// SseStatus Tests
// ============================================================================

#[test]
fn test_sse_status_default() {
    let status = SseStatus::default();
    assert_eq!(status, SseStatus::Disconnected);
}

#[test]
fn test_sse_status_variants() {
    assert_eq!(SseStatus::Disconnected, SseStatus::Disconnected);
    assert_eq!(SseStatus::Connecting, SseStatus::Connecting);
    assert_eq!(SseStatus::Connected, SseStatus::Connected);
    assert_eq!(SseStatus::Reconnecting, SseStatus::Reconnecting);
    assert_eq!(SseStatus::Failed, SseStatus::Failed);
}

#[test]
fn test_sse_status_not_equal() {
    assert_ne!(SseStatus::Disconnected, SseStatus::Connected);
    assert_ne!(SseStatus::Connecting, SseStatus::Reconnecting);
    assert_ne!(SseStatus::Connected, SseStatus::Failed);
}

#[test]
fn test_sse_status_copy() {
    let status = SseStatus::Connected;
    let copied = status;
    assert_eq!(status, copied);
}

// ============================================================================
// InitStatus Tests
// ============================================================================

#[test]
fn test_init_status_default() {
    let status = InitStatus::default();
    assert!(!status.is_initializing);
    assert!(status.current_step.is_empty());
    assert_eq!(status.steps_completed, 0);
    assert_eq!(status.total_steps, 0);
}

#[test]
fn test_init_status_fields() {
    let status = InitStatus {
        is_initializing: true,
        current_step: "Loading agents...".to_string(),
        steps_completed: 3,
        total_steps: 6,
    };

    assert!(status.is_initializing);
    assert_eq!(status.current_step, "Loading agents...");
    assert_eq!(status.steps_completed, 3);
    assert_eq!(status.total_steps, 6);
}

#[test]
fn test_init_status_progress() {
    let status = InitStatus {
        is_initializing: true,
        current_step: "Connecting...".to_string(),
        steps_completed: 5,
        total_steps: 10,
    };

    let progress = status.steps_completed as f64 / status.total_steps as f64;
    assert!((progress - 0.5).abs() < f64::EPSILON);
}

// ============================================================================
// ActiveTab Tests
// ============================================================================

#[test]
fn test_active_tab_default() {
    let tab = ActiveTab::default();
    assert_eq!(tab, ActiveTab::Chat);
}

#[test]
fn test_active_tab_variants() {
    assert_eq!(ActiveTab::Chat, ActiveTab::Chat);
    assert_eq!(ActiveTab::Conversations, ActiveTab::Conversations);
    assert_eq!(ActiveTab::Agents, ActiveTab::Agents);
    assert_eq!(ActiveTab::Artifacts, ActiveTab::Artifacts);
    assert_eq!(ActiveTab::Users, ActiveTab::Users);
    assert_eq!(ActiveTab::Analytics, ActiveTab::Analytics);
    assert_eq!(ActiveTab::Services, ActiveTab::Services);
    assert_eq!(ActiveTab::Config, ActiveTab::Config);
    assert_eq!(ActiveTab::Shortcuts, ActiveTab::Shortcuts);
    assert_eq!(ActiveTab::Logs, ActiveTab::Logs);
}

#[test]
fn test_active_tab_count() {
    let tabs = [
        ActiveTab::Chat,
        ActiveTab::Conversations,
        ActiveTab::Agents,
        ActiveTab::Artifacts,
        ActiveTab::Users,
        ActiveTab::Analytics,
        ActiveTab::Services,
        ActiveTab::Config,
        ActiveTab::Shortcuts,
        ActiveTab::Logs,
    ];
    assert_eq!(tabs.len(), 10);
}

// ============================================================================
// FocusedPanel Tests
// ============================================================================

#[test]
fn test_focused_panel_variants() {
    assert_eq!(FocusedPanel::Chat, FocusedPanel::Chat);
    assert_eq!(FocusedPanel::Sidebar, FocusedPanel::Sidebar);
    assert_eq!(FocusedPanel::Logs, FocusedPanel::Logs);
    assert_eq!(FocusedPanel::ApprovalDialog, FocusedPanel::ApprovalDialog);
}

#[test]
fn test_focused_panel_not_equal() {
    assert_ne!(FocusedPanel::Chat, FocusedPanel::Sidebar);
    assert_ne!(FocusedPanel::Logs, FocusedPanel::ApprovalDialog);
}

// ============================================================================
// InputMode Tests
// ============================================================================

#[test]
fn test_input_mode_variants() {
    assert_eq!(InputMode::Normal, InputMode::Normal);
    assert_eq!(InputMode::Insert, InputMode::Insert);
    assert_eq!(InputMode::Command, InputMode::Command);
}

#[test]
fn test_input_mode_not_equal() {
    assert_ne!(InputMode::Normal, InputMode::Insert);
    assert_ne!(InputMode::Insert, InputMode::Command);
    assert_ne!(InputMode::Normal, InputMode::Command);
}

// ============================================================================
// format_duration Tests
// ============================================================================

#[test]
fn test_format_duration_milliseconds() {
    assert_eq!(format_duration(0), "0ms");
    assert_eq!(format_duration(1), "1ms");
    assert_eq!(format_duration(500), "500ms");
    assert_eq!(format_duration(999), "999ms");
}

#[test]
fn test_format_duration_seconds() {
    assert_eq!(format_duration(1000), "1.0s");
    assert_eq!(format_duration(1500), "1.5s");
    assert_eq!(format_duration(5000), "5.0s");
    assert_eq!(format_duration(59999), "60.0s");
}

#[test]
fn test_format_duration_minutes() {
    assert_eq!(format_duration(60000), "1m 0s");
    assert_eq!(format_duration(90000), "1m 30s");
    assert_eq!(format_duration(120000), "2m 0s");
    assert_eq!(format_duration(125000), "2m 5s");
}

#[test]
fn test_format_duration_large_values() {
    assert_eq!(format_duration(3600000), "60m 0s");
    assert_eq!(format_duration(7200000), "120m 0s");
}

// ============================================================================
// truncate_text Tests
// ============================================================================

#[test]
fn test_truncate_text_short() {
    assert_eq!(truncate_text("hello", 10), "hello");
    assert_eq!(truncate_text("hi", 5), "hi");
}

#[test]
fn test_truncate_text_exact_length() {
    assert_eq!(truncate_text("hello", 5), "hello");
    assert_eq!(truncate_text("test", 4), "test");
}

#[test]
fn test_truncate_text_with_ellipsis() {
    assert_eq!(truncate_text("hello world", 8), "hello...");
    assert_eq!(truncate_text("this is a long string", 10), "this is...");
}

#[test]
fn test_truncate_text_minimum_length() {
    assert_eq!(truncate_text("hello", 3), "...");
    assert_eq!(truncate_text("hello", 4), "h...");
}

#[test]
fn test_truncate_text_empty_string() {
    assert_eq!(truncate_text("", 10), "");
    assert_eq!(truncate_text("", 0), "");
}

// ============================================================================
// short_id Tests
// ============================================================================

#[test]
fn test_short_id_long() {
    assert_eq!(short_id("12345678abcdef"), "12345678");
    assert_eq!(short_id("abcdefghijklmnop"), "abcdefgh");
}

#[test]
fn test_short_id_exact_8() {
    assert_eq!(short_id("12345678"), "12345678");
    assert_eq!(short_id("abcdefgh"), "abcdefgh");
}

#[test]
fn test_short_id_short() {
    assert_eq!(short_id("12345"), "12345");
    assert_eq!(short_id("abc"), "abc");
    assert_eq!(short_id(""), "");
}

// ============================================================================
// LoadingState Tests
// ============================================================================

#[test]
fn test_loading_state_default() {
    let state = LoadingState::default();
    assert_eq!(state, LoadingState::Idle);
}

#[test]
fn test_loading_state_variants() {
    assert_eq!(LoadingState::Idle, LoadingState::Idle);
    assert_eq!(LoadingState::Sending, LoadingState::Sending);
    assert_eq!(LoadingState::Connecting, LoadingState::Connecting);
    assert_eq!(LoadingState::Streaming, LoadingState::Streaming);
    assert_eq!(LoadingState::WaitingForTool, LoadingState::WaitingForTool);
    assert_eq!(LoadingState::WaitingForInput, LoadingState::WaitingForInput);
}

// ============================================================================
// ToolCallStatus Tests
// ============================================================================

#[test]
fn test_tool_call_status_variants() {
    assert_eq!(ToolCallStatus::Pending, ToolCallStatus::Pending);
    assert_eq!(ToolCallStatus::Approved, ToolCallStatus::Approved);
    assert_eq!(ToolCallStatus::Rejected, ToolCallStatus::Rejected);
    assert_eq!(ToolCallStatus::Executing, ToolCallStatus::Executing);
    assert_eq!(ToolCallStatus::Completed, ToolCallStatus::Completed);
    assert_eq!(ToolCallStatus::Failed, ToolCallStatus::Failed);
}

// ============================================================================
// InputType Tests
// ============================================================================

#[test]
fn test_input_type_default() {
    let input_type = InputType::default();
    assert_eq!(input_type, InputType::Text);
}

#[test]
fn test_input_type_variants() {
    assert_eq!(InputType::Text, InputType::Text);
    assert_eq!(InputType::Choice, InputType::Choice);
    assert_eq!(InputType::Confirm, InputType::Confirm);
}

// ============================================================================
// TaskMetadataDisplay Tests
// ============================================================================

#[test]
fn test_task_metadata_display_default() {
    let metadata = TaskMetadataDisplay::default();

    assert!(metadata.agent_name.is_none());
    assert!(metadata.model.is_none());
    assert!(metadata.started_at.is_none());
    assert!(metadata.completed_at.is_none());
    assert!(metadata.execution_time_ms.is_none());
    assert_eq!(metadata.step_count, 0);
    assert!(metadata.input_tokens.is_none());
    assert!(metadata.output_tokens.is_none());
    assert!(metadata.artifact_ids.is_empty());
}

#[test]
fn test_task_metadata_duration_display_with_time() {
    let mut metadata = TaskMetadataDisplay::default();
    metadata.execution_time_ms = Some(5000);

    let display = metadata.duration_display();
    assert!(display.is_some());
    assert_eq!(display.as_deref(), Some("5.0s"));
}

#[test]
fn test_task_metadata_tokens_display_both() {
    let mut metadata = TaskMetadataDisplay::default();
    metadata.input_tokens = Some(100);
    metadata.output_tokens = Some(200);

    let display = metadata.tokens_display();
    assert!(display.is_some());
    assert!(display.as_ref().is_some_and(|s| s.contains("100")));
    assert!(display.as_ref().is_some_and(|s| s.contains("200")));
}

#[test]
fn test_task_metadata_tokens_display_input_only() {
    let mut metadata = TaskMetadataDisplay::default();
    metadata.input_tokens = Some(100);

    let display = metadata.tokens_display();
    assert!(display.is_some());
    assert!(display.as_ref().is_some_and(|s| s.contains("in")));
}

#[test]
fn test_task_metadata_tokens_display_output_only() {
    let mut metadata = TaskMetadataDisplay::default();
    metadata.output_tokens = Some(200);

    let display = metadata.tokens_display();
    assert!(display.is_some());
    assert!(display.as_ref().is_some_and(|s| s.contains("out")));
}

// ============================================================================
// AgentConnectionStatus Tests
// ============================================================================

#[test]
fn test_agent_connection_status_default() {
    let status = AgentConnectionStatus::default();
    assert_eq!(status, AgentConnectionStatus::Disconnected);
}

#[test]
fn test_agent_connection_status_variants() {
    assert_eq!(
        AgentConnectionStatus::Connected,
        AgentConnectionStatus::Connected
    );
    assert_eq!(
        AgentConnectionStatus::Disconnected,
        AgentConnectionStatus::Disconnected
    );
    assert_eq!(
        AgentConnectionStatus::Connecting,
        AgentConnectionStatus::Connecting
    );
}

#[test]
fn test_agent_connection_status_error() {
    let error1 = AgentConnectionStatus::Error("Connection refused".to_string());
    let error2 = AgentConnectionStatus::Error("Connection refused".to_string());
    let error3 = AgentConnectionStatus::Error("Timeout".to_string());

    assert_eq!(error1, error2);
    assert_ne!(error1, error3);
}

// ============================================================================
// SystemInstructionsSource Tests
// ============================================================================

#[test]
fn test_system_instructions_source_default() {
    let source = SystemInstructionsSource::default();
    assert!(matches!(source, SystemInstructionsSource::Unknown));
}

#[test]
fn test_system_instructions_source_inline() {
    let source = SystemInstructionsSource::Inline;
    assert!(matches!(source, SystemInstructionsSource::Inline));
}

#[test]
fn test_system_instructions_source_file_path() {
    use std::path::PathBuf;
    let path = PathBuf::from("/etc/config/instructions.txt");
    let source = SystemInstructionsSource::FilePath(path.clone());

    assert!(
        matches!(&source, SystemInstructionsSource::FilePath(p) if p == &path),
        "Expected FilePath variant"
    );
}

// ============================================================================
// AgentInfo Tests
// ============================================================================

#[test]
fn test_agent_info_new() {
    let agent = AgentInfo::new("test-agent".to_string(), 8080);

    assert_eq!(agent.name, "test-agent");
    assert_eq!(agent.display_name, "test-agent");
    assert_eq!(agent.url, "http://localhost:8080");
    assert_eq!(agent.port, 8080);
    assert!(!agent.is_primary);
    assert_eq!(agent.status, AgentConnectionStatus::Disconnected);
}

#[test]
fn test_agent_info_with_display_name() {
    let agent = AgentInfo::new("test-agent".to_string(), 8080)
        .with_display_name("Test Agent Display".to_string());

    assert_eq!(agent.name, "test-agent");
    assert_eq!(agent.display_name, "Test Agent Display");
}

#[test]
fn test_agent_info_with_primary() {
    let agent = AgentInfo::new("primary-agent".to_string(), 8080).with_primary(true);

    assert!(agent.is_primary);
}

#[test]
fn test_agent_info_with_status() {
    let agent = AgentInfo::new("connected-agent".to_string(), 8080)
        .with_status(AgentConnectionStatus::Connected);

    assert_eq!(agent.status, AgentConnectionStatus::Connected);
}

#[test]
fn test_agent_info_builder_chaining() {
    let agent = AgentInfo::new("chained-agent".to_string(), 9000)
        .with_display_name("Chained Agent".to_string())
        .with_primary(true)
        .with_status(AgentConnectionStatus::Connecting);

    assert_eq!(agent.name, "chained-agent");
    assert_eq!(agent.display_name, "Chained Agent");
    assert_eq!(agent.port, 9000);
    assert!(agent.is_primary);
    assert_eq!(agent.status, AgentConnectionStatus::Connecting);
}

// ============================================================================
// ApprovalAction Tests
// ============================================================================

#[test]
fn test_approval_action_variants() {
    assert_eq!(ApprovalAction::Approve, ApprovalAction::Approve);
    assert_eq!(ApprovalAction::Reject, ApprovalAction::Reject);
    assert_eq!(ApprovalAction::Edit, ApprovalAction::Edit);
}

#[test]
fn test_approval_action_cycle() {
    let mut action = ApprovalAction::Approve;

    action = match action {
        ApprovalAction::Approve => ApprovalAction::Reject,
        ApprovalAction::Reject => ApprovalAction::Edit,
        ApprovalAction::Edit => ApprovalAction::Approve,
    };
    assert_eq!(action, ApprovalAction::Reject);

    action = match action {
        ApprovalAction::Approve => ApprovalAction::Reject,
        ApprovalAction::Reject => ApprovalAction::Edit,
        ApprovalAction::Edit => ApprovalAction::Approve,
    };
    assert_eq!(action, ApprovalAction::Edit);

    action = match action {
        ApprovalAction::Approve => ApprovalAction::Reject,
        ApprovalAction::Reject => ApprovalAction::Edit,
        ApprovalAction::Edit => ApprovalAction::Approve,
    };
    assert_eq!(action, ApprovalAction::Approve);
}

// ============================================================================
// ExecutionStatus Tests
// ============================================================================

#[test]
fn test_execution_status_variants() {
    assert_eq!(ExecutionStatus::Executing, ExecutionStatus::Executing);
    assert_eq!(ExecutionStatus::Completed, ExecutionStatus::Completed);
    assert_eq!(ExecutionStatus::Failed, ExecutionStatus::Failed);
    assert_eq!(ExecutionStatus::Rejected, ExecutionStatus::Rejected);
}

#[test]
fn test_execution_status_transitions() {
    let initial = ExecutionStatus::Executing;
    let success = true;

    let final_status = if success {
        ExecutionStatus::Completed
    } else {
        ExecutionStatus::Failed
    };

    assert_eq!(initial, ExecutionStatus::Executing);
    assert_eq!(final_status, ExecutionStatus::Completed);
}

// ============================================================================
// ConversationDisplay Tests
// ============================================================================

#[test]
fn test_conversation_display_creation() {
    let context_id = ContextId::generate();
    let display = ConversationDisplay {
        context_id: context_id.clone(),
        name: "Test Conversation".to_string(),
        task_count: 5,
        message_count: 10,
        last_message_at: Some(Utc::now()),
        updated_at: Some(Utc::now()),
    };

    assert_eq!(display.context_id, context_id);
    assert_eq!(display.name, "Test Conversation");
    assert_eq!(display.task_count, 5);
    assert_eq!(display.message_count, 10);
}

// ============================================================================
// ConversationsState Tests
// ============================================================================

#[test]
fn test_conversations_state_new() {
    let state = ConversationsState::new();

    assert!(state.conversations.is_empty());
    assert_eq!(state.selected_index, 0);
    assert!(!state.editing);
    assert!(state.edit_buffer.is_empty());
}

#[test]
fn test_conversations_state_update() {
    let mut state = ConversationsState::new();
    let conversations = vec![
        ConversationDisplay {
            context_id: ContextId::generate(),
            name: "First".to_string(),
            task_count: 1,
            message_count: 1,
            last_message_at: None,
            updated_at: None,
        },
        ConversationDisplay {
            context_id: ContextId::generate(),
            name: "Second".to_string(),
            task_count: 2,
            message_count: 2,
            last_message_at: None,
            updated_at: None,
        },
    ];

    state.update_conversations(conversations);

    assert_eq!(state.conversations.len(), 2);
    assert!(state.last_refresh.is_some());
}

#[test]
fn test_conversations_state_navigation() {
    let mut state = ConversationsState::new();
    let conversations: Vec<_> = (0..3)
        .map(|i| ConversationDisplay {
            context_id: ContextId::generate(),
            name: format!("Conv {}", i),
            task_count: 0,
            message_count: 0,
            last_message_at: None,
            updated_at: None,
        })
        .collect();
    state.update_conversations(conversations);

    state.select_next();
    assert_eq!(state.selected_index, 1);

    state.select_next();
    assert_eq!(state.selected_index, 2);

    state.select_next();
    assert_eq!(state.selected_index, 0); // Wraps

    state.select_prev();
    assert_eq!(state.selected_index, 2); // Wraps back
}

#[test]
fn test_conversations_state_edit() {
    let mut state = ConversationsState::new();
    let context_id = ContextId::generate();
    state.update_conversations(vec![ConversationDisplay {
        context_id: context_id.clone(),
        name: "Old Name".to_string(),
        task_count: 0,
        message_count: 0,
        last_message_at: None,
        updated_at: None,
    }]);

    state.start_edit();
    assert!(state.editing);
    assert_eq!(state.edit_buffer, "Old Name");

    state.edit_buffer = "New Name".to_string();
    let result = state.finish_edit();

    assert!(result.is_some());
    let (id, new_name) = result.expect("Expected Some");
    assert_eq!(id, context_id);
    assert_eq!(new_name, "New Name");
}

#[test]
fn test_conversations_state_cancel_edit() {
    let mut state = ConversationsState::new();
    state.update_conversations(vec![ConversationDisplay {
        context_id: ContextId::generate(),
        name: "Test".to_string(),
        task_count: 0,
        message_count: 0,
        last_message_at: None,
        updated_at: None,
    }]);

    state.start_edit();
    state.edit_push_char('!');
    state.cancel_edit();

    assert!(!state.editing);
    assert!(state.edit_buffer.is_empty());
}

#[test]
fn test_conversations_state_delete() {
    let mut state = ConversationsState::new();
    let context_id = ContextId::generate();
    state.update_conversations(vec![ConversationDisplay {
        context_id: context_id.clone(),
        name: "To Delete".to_string(),
        task_count: 0,
        message_count: 0,
        last_message_at: None,
        updated_at: None,
    }]);

    let result = state.delete_selected();

    assert_eq!(result, Some(context_id));
    assert!(state.conversations.is_empty());
}

// ============================================================================
// RiskLevel Tests
// ============================================================================

#[test]
fn test_risk_level_variants() {
    assert_eq!(RiskLevel::Safe, RiskLevel::Safe);
    assert_eq!(RiskLevel::Moderate, RiskLevel::Moderate);
    assert_eq!(RiskLevel::Dangerous, RiskLevel::Dangerous);
}

#[test]
fn test_risk_level_symbol() {
    assert_eq!(RiskLevel::Safe.symbol(), "✓");
    assert_eq!(RiskLevel::Moderate.symbol(), "⚠");
    assert_eq!(RiskLevel::Dangerous.symbol(), "⛔");
}

#[test]
fn test_risk_level_label() {
    assert_eq!(RiskLevel::Safe.label(), "Safe");
    assert_eq!(RiskLevel::Moderate.label(), "Moderate");
    assert_eq!(RiskLevel::Dangerous.label(), "Dangerous");
}

#[test]
fn test_risk_level_const() {
    const SAFE_SYMBOL: &str = RiskLevel::Safe.symbol();
    const MODERATE_LABEL: &str = RiskLevel::Moderate.label();

    assert_eq!(SAFE_SYMBOL, "✓");
    assert_eq!(MODERATE_LABEL, "Moderate");
}

// ============================================================================
// ToolResult Tests
// ============================================================================

#[test]
fn test_tool_result_success() {
    let result = ToolResult::success("Operation completed".to_string());

    assert!(result.success);
    assert_eq!(result.output, "Operation completed");
    assert!(result.error.is_none());
}

#[test]
fn test_tool_result_error() {
    let result = ToolResult::error("Something went wrong".to_string());

    assert!(!result.success);
    assert!(result.output.is_empty());
    assert_eq!(result.error, Some("Something went wrong".to_string()));
}

#[test]
fn test_tool_result_success_empty() {
    let result = ToolResult::success(String::new());

    assert!(result.success);
    assert!(result.output.is_empty());
    assert!(result.error.is_none());
}

#[test]
fn test_tool_result_clone() {
    let original = ToolResult::success("cloneable".to_string());
    let cloned = original.clone();

    assert_eq!(original.success, cloned.success);
    assert_eq!(original.output, cloned.output);
    assert_eq!(original.error, cloned.error);
}

#[test]
fn test_tool_result_debug() {
    let result = ToolResult::success("debug test".to_string());
    let debug_str = format!("{:?}", result);

    assert!(debug_str.contains("ToolResult"));
    assert!(debug_str.contains("debug test"));
}
