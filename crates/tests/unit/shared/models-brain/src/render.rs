use chrono::{TimeZone, Utc};
use systemprompt_extension::brain::{
    InjectedFact, RecallBlock, RecallOutcome, SkipReason, WithheldSummary,
};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::WithheldReason;

fn block() -> RecallBlock {
    let as_of = Utc
        .with_ymd_and_hms(2026, 10, 30, 9, 15, 0)
        .single()
        .expect("instant");
    let mut withheld = WithheldSummary::default();
    withheld.add(WithheldReason::Source);
    withheld.add(WithheldReason::Source);
    withheld.add(WithheldReason::Budget);
    RecallBlock::new("project:apollo", as_of)
        .with_fact(InjectedFact {
            id: BrainFactId::new("m_11111111-1111-4111-8111-111111111111"),
            revision: 3,
            source: "doc:d1@r7".to_owned(),
            text: "Acme must deliver\n by 2026-12-01".to_owned(),
        })
        .with_fact(InjectedFact {
            id: BrainFactId::new("m_22222222-2222-4222-8222-222222222222"),
            revision: 1,
            source: "entered:u1".to_owned(),
            text: "Use <b>tabs</b> & spaces".to_owned(),
        })
        .with_withheld(withheld)
        .with_tokens(41)
}

#[test]
fn render_produces_team_memory_block() {
    let expected = "<team_memory scope=\"project:apollo\" as_of=\"2026-10-30T09:15:00Z\">\n\
        - [m_11111111-1111-4111-8111-111111111111 · doc:d1@r7] Acme must deliver by 2026-12-01\n\
        - [m_22222222-2222-4222-8222-222222222222 · entered:u1] Use &lt;b&gt;tabs&lt;/b&gt; &amp; spaces\n\
        </team_memory>";
    assert_eq!(block().render(), expected);
}

#[test]
fn render_escapes_scope_attribute() {
    let as_of = Utc
        .with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
        .single()
        .expect("instant");
    let rendered = RecallBlock::new("a\"b", as_of).render();
    assert!(rendered.starts_with("<team_memory scope=\"a&quot;b\""));
    assert!(rendered.ends_with("</team_memory>"));
}

#[test]
fn ids_lists_fact_at_revision() {
    assert_eq!(
        block().ids(),
        "m_11111111-1111-4111-8111-111111111111@3,m_22222222-2222-4222-8222-222222222222@1"
    );
    let empty = RecallBlock::new("org", Utc::now());
    assert_eq!(empty.ids(), "");
}

#[test]
fn descriptors_summarise_the_outcome() {
    assert_eq!(
        RecallOutcome::Injected(block()).descriptor(),
        "injected=2 withheld=3 tokens=41 as_of=2026-10-30T09:15:00Z"
    );
    let skipped = RecallOutcome::Skipped {
        reason: SkipReason::Deadline,
    };
    assert_eq!(skipped.descriptor(), "skipped=deadline");
}

#[test]
fn fact_ids_carry_the_prefix() {
    let id = BrainFactId::generate();
    assert!(id.as_str().starts_with("m_"));
    assert!(BrainFactId::try_new(id.as_str()).is_ok());
    assert!(BrainFactId::try_new("11111111-1111-4111-8111-111111111111").is_err());
    assert!(BrainFactId::try_new("m_not-a-uuid").is_err());
}
