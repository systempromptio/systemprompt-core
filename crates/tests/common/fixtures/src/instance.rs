use systemprompt_identifiers::{AgentName, InstanceId, ServiceName};
use uuid::Uuid;

pub fn unique_instance() -> InstanceId {
    InstanceId::new(format!("test-instance-{}", Uuid::new_v4().simple()))
}

pub fn unique_name(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

pub fn unique_agent_name(prefix: &str) -> AgentName {
    AgentName::new(unique_name(prefix))
}

pub fn unique_service_name(prefix: &str) -> ServiceName {
    ServiceName::new(unique_name(prefix))
}
