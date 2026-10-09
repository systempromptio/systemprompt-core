use systemprompt_identifiers::InstanceId;
use uuid::Uuid;

pub fn unique_instance() -> InstanceId {
    InstanceId::new(format!("test-instance-{}", Uuid::new_v4().simple()))
}
