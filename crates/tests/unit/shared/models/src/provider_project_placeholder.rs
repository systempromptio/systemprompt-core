//! A provider catalog must not name a Google Cloud project literally.
//!
//! The project id is a tenant identifier that Vertex echoes in every IAM
//! error, and the gateway relays those to the caller. A catalog ships with
//! the image, so a literal id reaches every installation and every client
//! that trips a 403. The endpoint says `{project}` and the gateway fills it
//! from the service account in the secret.

use systemprompt_identifiers::{ProviderId, SecretName};
use systemprompt_models::services::providers::{
    PROJECT_PLACEHOLDER, ProviderRegistryError, names_a_project_literally,
};
use systemprompt_models::services::{ApiSurface, ProviderEntry, ProviderRegistry, WireProtocol};

fn vertex(endpoint: &str) -> ProviderEntry {
    ProviderEntry {
        name: ProviderId::new("vertex"),
        wire: WireProtocol::Gemini,
        surface: ApiSurface::Gemini,
        endpoint: endpoint.to_owned(),
        api_key_secret: SecretName::new("vertex"),
        governance: Default::default(),
        extra_headers: Default::default(),
        models: Vec::new(),
    }
}

#[test]
fn a_literal_project_on_vertex_is_detected_and_the_placeholder_is_not() {
    assert!(names_a_project_literally(
        "https://us-central1-aiplatform.googleapis.com/v1/projects/acme-123/locations/us-central1/publishers/google"
    ));
    assert!(names_a_project_literally(
        "https://aiplatform.googleapis.com/v1beta1/projects/acme-123/locations/global/endpoints/openapi"
    ));
    assert!(!names_a_project_literally(&format!(
        "https://us-central1-aiplatform.googleapis.com/v1/projects/{PROJECT_PLACEHOLDER}/locations/us-central1/publishers/google"
    )));
    assert!(
        !names_a_project_literally("https://example.com/v1/projects/acme-123"),
        "only Vertex hosts are project-scoped in this sense"
    );
    assert!(!names_a_project_literally("not a url"));
}

// Why: this is the boot gate. A catalog that would ship a tenant id is
// refused before the server serves a request, with the fix in the message.
#[test]
fn the_registry_refuses_a_catalog_that_names_a_project() {
    let literal = ProviderRegistry {
        providers: vec![vertex(
            "https://us-central1-aiplatform.googleapis.com/v1/projects/acme-123/locations/us-central1/publishers/google",
        )],
    };
    let err = literal.validate().unwrap_err();
    assert!(
        matches!(err, ProviderRegistryError::LiteralProjectInEndpoint { ref provider, .. } if provider == "vertex"),
        "{err}"
    );
    assert!(err.to_string().contains("projects/{project}"), "{err}");

    let placeholder = ProviderRegistry {
        providers: vec![vertex(&format!(
            "https://us-central1-aiplatform.googleapis.com/v1/projects/{PROJECT_PLACEHOLDER}/locations/us-central1/publishers/google"
        ))],
    };
    placeholder
        .validate()
        .expect("a placeholder endpoint is valid");
}
