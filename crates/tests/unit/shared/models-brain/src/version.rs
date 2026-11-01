use systemprompt_identifiers::{FactKindId, PipelineVersion};
use systemprompt_models::brain::PipelineVersionInputs;

fn inputs() -> PipelineVersionInputs {
    PipelineVersionInputs::new("declared-abc")
        .with_prompt_version(FactKindId::new("obligation"), "v1")
        .with_prompt_version(FactKindId::new("decision"), "v4")
}

#[test]
fn derive_is_deterministic_and_well_formed() {
    let version = inputs().derive();
    assert_eq!(version, inputs().derive());
    assert!(PipelineVersion::try_new(version.as_str()).is_ok());
}

#[test]
fn derive_ignores_prompt_insertion_order() {
    let reordered = PipelineVersionInputs::new("declared-abc")
        .with_prompt_version(FactKindId::new("decision"), "v4")
        .with_prompt_version(FactKindId::new("obligation"), "v1");
    assert_eq!(reordered.derive(), inputs().derive());
}

#[test]
fn derive_changes_with_every_input() {
    let base = inputs().derive();
    let mut declared = inputs();
    declared.declared_hash = "declared-abd".to_owned();
    let mut code = inputs();
    code.code_version += 1;
    let prompt = inputs().with_prompt_version(FactKindId::new("obligation"), "v2");
    for changed in [declared, code, prompt] {
        assert_ne!(changed.derive(), base);
    }
}

#[test]
fn pipeline_version_shape_is_checked() {
    assert!(PipelineVersion::try_new("0123456789abcdef").is_ok());
    assert!(PipelineVersion::try_new("0123456789ABCDEF").is_err());
    assert!(PipelineVersion::try_new("0123456789abcde").is_err());
}
