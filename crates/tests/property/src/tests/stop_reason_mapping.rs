use proptest::prelude::*;
use systemprompt_models::wire::canonical::CanonicalStopReason;

// Why: the mapping is the whole subject of the wire matrix, and every cell
// there drives one hand-written string. These state the rules the mapping must
// hold for strings nobody thought to write down -- a new provider's private
// finish reason is exactly that case, and the failure it produces is a
// silently dropped tool call rather than an error.
fn all_reasons() -> Vec<CanonicalStopReason> {
    vec![
        CanonicalStopReason::EndTurn,
        CanonicalStopReason::MaxTokens,
        CanonicalStopReason::StopSequence,
        CanonicalStopReason::ToolUse,
        CanonicalStopReason::Other,
    ]
}

proptest! {
    // Total: every string maps, none panics, and an unrecognised one lands on
    // Other rather than on a reason that would end the turn wrongly.
    #[test]
    fn every_string_maps_to_a_reason(s in ".{0,40}") {
        let anthropic = CanonicalStopReason::from_anthropic(&s);
        let openai = CanonicalStopReason::from_openai(&s);
        prop_assert!(all_reasons().contains(&anthropic));
        prop_assert!(all_reasons().contains(&openai));
    }

    // Correcting a generic stop to tool use is a normaliser: applying it twice
    // must not move the reason again, or a re-rendered frame drifts.
    #[test]
    fn with_tool_use_is_idempotent(index in 0usize..5, has_tool in any::<bool>()) {
        let reason = all_reasons()[index];
        let once = reason.with_tool_use(has_tool);
        prop_assert_eq!(once, once.with_tool_use(has_tool));
        prop_assert_eq!(
            reason.with_tool_use(true),
            reason.with_tool_use(true).with_tool_use(true)
        );
    }

    // The correction may only ever produce ToolUse, and only from the two
    // reasons that mean "the turn simply ended". Truncation and an explicit
    // stop sequence must survive it: a call cut mid-arguments is unparseable,
    // so declaring tool use there hands the client a call it cannot run.
    #[test]
    fn with_tool_use_only_upgrades_a_generic_stop(index in 0usize..5) {
        let reason = all_reasons()[index];
        let upgraded = reason.with_tool_use(true);
        match reason {
            CanonicalStopReason::EndTurn | CanonicalStopReason::Other => {
                prop_assert_eq!(upgraded, CanonicalStopReason::ToolUse);
            },
            other => prop_assert_eq!(upgraded, other),
        }
    }

    // Without a tool call the reason is untouched, whatever it was.
    #[test]
    fn with_tool_use_false_is_the_identity(index in 0usize..5) {
        let reason = all_reasons()[index];
        prop_assert_eq!(reason.with_tool_use(false), reason);
    }

    // Every reason renders in both dialects, and the two vocabularies never
    // collapse two distinct outcomes onto one string in a way that loses the
    // tool-use signal.
    #[test]
    fn both_dialects_render_tool_use_distinctly(index in 0usize..5) {
        let reason = all_reasons()[index];
        let anthropic = reason.anthropic_str();
        let openai = reason.openai_str();
        prop_assert!(!anthropic.is_empty());
        prop_assert!(!openai.is_empty());
        if reason == CanonicalStopReason::ToolUse {
            prop_assert_eq!(anthropic, "tool_use");
            prop_assert_eq!(openai, "tool_calls");
        } else {
            prop_assert_ne!(anthropic, "tool_use");
            prop_assert_ne!(openai, "tool_calls");
        }
    }

    // Round trip through each dialect's own vocabulary: a reason that dialect
    // can name must parse back to itself.
    #[test]
    fn anthropic_round_trips_its_own_vocabulary(index in 0usize..5) {
        let reason = all_reasons()[index];
        let parsed = CanonicalStopReason::from_anthropic(reason.anthropic_str());
        match reason {
            // Why: Other has no name of its own in this dialect and renders as
            // end_turn, which is the deliberate lossy edge.
            CanonicalStopReason::Other => {
                prop_assert_eq!(parsed, CanonicalStopReason::EndTurn);
            },
            named => prop_assert_eq!(parsed, named),
        }
    }
}
