use schemars::JsonSchema;
use serde_json::Value as JsonValue;
use systemprompt_models::artifacts::{
    AudioArtifact, ChartArtifact, CliArtifact, CopyPasteTextArtifact, DashboardArtifact,
    ImageArtifact, ListArtifact, PresentationCardArtifact, TableArtifact, TextArtifact,
    ToolResponse, VideoArtifact,
};

pub trait McpOutputSchema: JsonSchema {
    fn artifact_type() -> &'static str;

    fn artifact_type_name(&self) -> String {
        Self::artifact_type().to_string()
    }

    fn artifact_title(&self) -> Option<String> {
        None
    }

    fn validated_schema() -> JsonValue
    where
        Self: Sized,
    {
        let root_schema = schemars::schema_for!(ToolResponse<Self>);

        let mut schema = match serde_json::to_value(&root_schema) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "Failed to serialize schema");
                return JsonValue::Null;
            },
        };

        if let Some(obj) = schema.as_object_mut() {
            obj.insert(
                "x-artifact-type".to_string(),
                JsonValue::String(Self::artifact_type().to_string()),
            );
        }

        schema
    }
}

macro_rules! impl_mcp_output {
    ($ty:ty, $name:expr) => {
        impl McpOutputSchema for $ty {
            fn artifact_type() -> &'static str {
                $name
            }
        }
    };
}

macro_rules! impl_mcp_output_with_optional_title {
    ($ty:ty, $name:expr) => {
        impl McpOutputSchema for $ty {
            fn artifact_type() -> &'static str {
                $name
            }

            fn artifact_title(&self) -> Option<String> {
                self.title.clone()
            }
        }
    };
}

macro_rules! impl_mcp_output_with_required_title {
    ($ty:ty, $name:expr) => {
        impl McpOutputSchema for $ty {
            fn artifact_type() -> &'static str {
                $name
            }

            fn artifact_title(&self) -> Option<String> {
                Some(self.title.clone())
            }
        }
    };
}

macro_rules! impl_mcp_output_delegated {
    ($ty:ty, $name:expr) => {
        impl McpOutputSchema for $ty {
            fn artifact_type() -> &'static str {
                $name
            }

            fn artifact_type_name(&self) -> String {
                self.artifact_type_str().to_string()
            }

            fn artifact_title(&self) -> Option<String> {
                self.title()
            }
        }
    };
}

impl_mcp_output_with_optional_title!(TextArtifact, TextArtifact::ARTIFACT_TYPE_STR);
impl_mcp_output_with_optional_title!(
    CopyPasteTextArtifact,
    CopyPasteTextArtifact::ARTIFACT_TYPE_STR
);
impl_mcp_output_with_optional_title!(AudioArtifact, AudioArtifact::ARTIFACT_TYPE_STR);

impl_mcp_output_with_required_title!(DashboardArtifact, DashboardArtifact::ARTIFACT_TYPE_STR);
impl_mcp_output_with_required_title!(
    PresentationCardArtifact,
    PresentationCardArtifact::ARTIFACT_TYPE_STR
);

impl_mcp_output!(TableArtifact, TableArtifact::ARTIFACT_TYPE_STR);
impl_mcp_output!(ListArtifact, ListArtifact::ARTIFACT_TYPE_STR);
impl_mcp_output!(ChartArtifact, ChartArtifact::ARTIFACT_TYPE_STR);
impl_mcp_output!(ImageArtifact, ImageArtifact::ARTIFACT_TYPE_STR);
impl_mcp_output!(VideoArtifact, VideoArtifact::ARTIFACT_TYPE_STR);

impl_mcp_output_delegated!(CliArtifact, "cli");
