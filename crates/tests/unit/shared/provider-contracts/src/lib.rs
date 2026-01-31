//! Unit tests for systemprompt-provider-contracts crate
//!
//! Tests cover:
//! - Tool types (ToolDefinition, ToolCallRequest, ToolCallResult, ToolContent)
//! - Tool errors (ToolProviderError)
//! - Tool context (ToolContext)
//! - Job types (JobResult, JobContext)
//! - LLM types (ChatMessage, ChatRole, ChatRequest, ChatResponse)
//! - LLM parameters (SamplingParameters, TokenUsage)
//! - LLM errors (LlmProviderError)
//! - Execution context (ToolExecutionContext)

#![allow(clippy::all)]

mod job;
mod llm;
mod tool;
