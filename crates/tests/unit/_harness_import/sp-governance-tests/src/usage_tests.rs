use harness_engine::{Governor, Protocol, Purpose, ResponseAction, Usage};
use sp_governance::UsageTotals;

use crate::support::{call, rig};

const FALLBACK_CONTEXT: u32 = 128_000;
const FALLBACK_OUTPUT: u32 = 8_192;

#[tokio::test]
async fn usage_counts_answers_and_step_tokens_only() {
    let rig = rig(true, 3_600);
    let step = call("claude-sonnet-5-5");
    assert_eq!(
        rig.governance.response(&step, 200, 1).await,
        ResponseAction::Continue
    );
    assert_eq!(
        rig.governance.response(&step, 500, 1).await,
        ResponseAction::Continue
    );
    let usage = Usage {
        input: 1_150,
        output: 58,
        reasoning: 10,
        cache_read: 1_000,
        cache_write: 30,
    };
    rig.governance.step_finished(&step, &usage).await;
    rig.governance.step_finished(&step, &usage).await;
    let mut title = step.clone();
    title.purpose = Purpose::Title;
    rig.governance.step_finished(&title, &usage).await;
    let totals = rig.governance.usage();
    assert_eq!(totals.forwarded_total, 2);
    assert_eq!(totals.tokens_in_total, 2_300);
    assert_eq!(totals.tokens_out_total, 116);
    assert!(totals.last_activity_at.is_some());

    let generation = rig.governance.generation();
    rig.reinstall();
    assert_eq!(rig.governance.usage(), UsageTotals::default());
    assert!(rig.governance.generation() > generation);
    assert_eq!(
        rig.governance.governed_by().as_deref(),
        Some("sp.example.com")
    );
    rig.governance.clear();
    assert_eq!(
        rig.governance.models(),
        Vec::<harness_engine::ModelSpec>::new()
    );
    assert_eq!(
        rig.governance.response(&step, 200, 1).await,
        ResponseAction::Continue
    );
    assert_eq!(rig.governance.usage().forwarded_total, 0);
}

#[tokio::test]
async fn models_map_both_protocols_with_fallbacks() {
    let rig = rig(true, 3_600);
    let models = rig.governance.models();
    assert_eq!(models.len(), 2);
    let claude = &models[0];
    assert_eq!(claude.protocol, Protocol::Messages);
    assert_eq!(
        (claude.context_window, claude.max_output),
        (1_000_000, 128_000)
    );
    assert!(claude.supports.tool_calls && claude.supports.images && claude.supports.caching);
    let chat = &models[1];
    assert_eq!(chat.name, "gpt-x");
    assert_eq!(chat.protocol, Protocol::ChatCompletions);
    assert_eq!(
        (chat.context_window, chat.max_output),
        (FALLBACK_CONTEXT, FALLBACK_OUTPUT)
    );
    assert!(chat.supports.tool_calls && !chat.supports.caching && !chat.supports.reasoning);
}
