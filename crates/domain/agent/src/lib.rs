#![allow(
    clippy::unused_async,
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::missing_const_for_fn,
    clippy::clone_on_ref_ptr,
    clippy::items_after_statements,
    clippy::useless_conversion,
    clippy::redundant_closure,
    clippy::redundant_closure_for_method_calls,
    clippy::map_unwrap_or,
    clippy::struct_field_names,
    clippy::ignored_unit_patterns,
    clippy::unused_self,
    clippy::unnecessary_wraps,
    clippy::set_contains_or_insert,
    clippy::match_same_arms,
    clippy::implicit_clone,
    clippy::doc_markdown,
    clippy::ref_option,
    clippy::manual_let_else,
    clippy::needless_pass_by_value,
    clippy::expect_used,
    clippy::too_many_arguments,
    clippy::option_if_let_else,
    clippy::stable_sort_primitive,
    clippy::cast_lossless,
    clippy::clone_on_copy,
    clippy::single_match_else,
    clippy::needless_borrow,
    clippy::wildcard_enum_match_arm,
    clippy::type_complexity,
    clippy::wildcard_imports,
    clippy::missing_fields_in_debug,
    clippy::new_without_default,
    clippy::explicit_iter_loop,
    clippy::collapsible_if,
    clippy::needless_borrows_for_generic_args,
    clippy::manual_strip,
    clippy::manual_range_contains,
    clippy::redundant_clone,
    clippy::semicolon_if_nothing_returned,
    clippy::collection_is_never_read,
    clippy::option_as_ref_deref,
    clippy::match_wildcard_for_single_variants,
    clippy::collapsible_match,
    clippy::map_clone,
    clippy::unnecessary_sort_by
)]

pub mod error;
pub mod extension;
pub mod models;
pub mod repository;
pub mod services;
pub mod state;

pub use extension::AgentExtension;

pub use state::AgentState;

pub use models::a2a::{
    A2aJsonRpcRequest, A2aRequestParams, A2aResponse, AgentCapabilities, AgentCard, AgentInterface,
    AgentProvider, AgentSkill, Artifact, DataPart, Message, MessageSendParams, Part,
    SecurityScheme, Task, TaskIdParams, TaskQueryParams, TaskState, TaskStatus, TextPart,
    TransportProtocol,
};

pub use error::{AgentError, ArtifactError, ContextError, ProtocolError, TaskError};

pub use services::{
    AgentEvent, AgentEventBus, AgentHandlerState, AgentOrchestrator, AgentServer, AgentStatus,
    ContextService, PlaybookIngestionService, PlaybookService, SkillIngestionService, SkillService,
};
