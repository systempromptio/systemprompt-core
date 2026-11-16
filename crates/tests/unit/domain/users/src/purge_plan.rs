pub(crate) fn purge_plan() -> systemprompt_extension::AdmittedPurge {
    let admitted = systemprompt_test_fixtures::schema_admitted().expect("the core members admit");
    systemprompt_extension::AdmittedPurge::of(admitted.built()).expect("the core purge plan admits")
}
