use systemprompt_mcp::McpToolHandler;
use systemprompt_runtime::documents::mcp::{GuideHandler, GuideInput};
use systemprompt_runtime::documents::refusal::RefusalCode;

use crate::documents_mcp_support::{ctx_for, exec, harness, registered};

async fn guide(topics: &[&str]) -> String {
    let h = harness().await;
    let owner = registered("guide").await;
    GuideHandler { tools: &h.tools }
        .handle(
            GuideInput {
                topics: topics.iter().map(|t| (*t).to_owned()).collect(),
            },
            &ctx_for(Some(&owner)),
            &exec(),
        )
        .await
        .expect("guide")
        .1
}

#[tokio::test]
async fn every_refusal_code_has_a_guide_topic() {
    for code in RefusalCode::ALL {
        let topic = format!("refusal.{}", code.as_str());
        let text = guide(&[topic.as_str()]).await;
        assert!(
            text.contains(&format!("{}:", code.as_str())),
            "{topic}: {text}"
        );
    }
}

#[tokio::test]
async fn formats_lists_the_insert_kinds_each_codec_accepts() {
    let text = guide(&["formats"]).await;
    assert!(
        text.contains("html: heading, paragraph, list_item"),
        "{text}"
    );
    assert!(text.contains("text: text"), "{text}");
}

#[tokio::test]
async fn topics_return_in_order_and_unknown_topics_name_the_known_ones() {
    let text = guide(&["editing", "nonsense"]).await;
    let editing = text.find("## editing").expect("editing");
    let unknown = text.find("## nonsense").expect("unknown");
    assert!(editing < unknown);
    assert!(text.contains("before the call"), "{text}");
    assert!(text.contains("refusal.<code>"), "{text}");
}
