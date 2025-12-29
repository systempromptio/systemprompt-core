use anyhow::Result;
use serde_json::Value;

use systemprompt_core_content::models::ContentError;

pub fn find_latest_items<'a>(
    item: &Value,
    all_items: &'a [Value],
    limit: usize,
) -> Result<Vec<&'a Value>> {
    let item_slug = item
        .get("slug")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_field("slug"))?;

    let results: Vec<&'a Value> = all_items
        .iter()
        .filter(|other| {
            other
                .get("slug")
                .and_then(|v| v.as_str())
                .is_some_and(|other_slug| other_slug != item_slug)
        })
        .take(limit)
        .collect();

    Ok(results)
}

pub fn find_popular_items<'a>(
    item: &Value,
    all_items: &'a [Value],
    popular_ids: &[String],
    exclude_slugs: &[&str],
    limit: usize,
) -> Result<Vec<&'a Value>> {
    let item_slug = item
        .get("slug")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_field("slug"))?;

    let mut popular: Vec<&'a Value> = popular_ids
        .iter()
        .filter_map(|id| {
            all_items.iter().find(|candidate| {
                let item_id = candidate.get("id").and_then(|v| v.as_str());
                let slug = candidate.get("slug").and_then(|v| v.as_str());

                match (item_id, slug) {
                    (Some(item_id), Some(slug)) => {
                        item_id == id && slug != item_slug && !exclude_slugs.contains(&slug)
                    },
                    _ => false,
                }
            })
        })
        .take(limit)
        .collect();

    if popular.len() < limit {
        let remaining = limit - popular.len();
        let popular_slugs: Vec<&str> = popular
            .iter()
            .filter_map(|p| p.get("slug").and_then(|v| v.as_str()))
            .collect();

        let fallback: Vec<&'a Value> = all_items
            .iter()
            .filter(|other| {
                other
                    .get("slug")
                    .and_then(|v| v.as_str())
                    .is_some_and(|other_slug| {
                        other_slug != item_slug
                            && !exclude_slugs.contains(&other_slug)
                            && !popular_slugs.contains(&other_slug)
                    })
            })
            .take(remaining)
            .collect();

        popular.extend(fallback);
    }

    Ok(popular)
}
