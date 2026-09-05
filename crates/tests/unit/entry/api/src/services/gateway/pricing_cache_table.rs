//! Every catalog provider's own cache rates reach the bill.
//!
//! `pricing.rs` proved cache pricing once, against a hand-built `ModelPricing`
//! with a single hardcoded `cache_read_per_million`. That never crossed the
//! resolver, so a provider entry whose cache rates are declared but never
//! looked up billed at zero and no test noticed. This drives one entry per
//! provider *shape* -- the four wires plus an `OpenAI`-compatible third party
//! -- through `resolve`, and asserts the four-class formula from that entry's
//! own numbers.
//!
//! The rates below are deliberately distinct per provider and per class. A
//! resolver that returned the wrong entry, or an arithmetic that folded two
//! classes together, cannot produce the expected figure by coincidence.

use systemprompt_api::services::gateway::pricing::resolve;
use systemprompt_identifiers::{ModelId, ProviderId, SecretName};
use systemprompt_models::services::{
    ApiSurface, ModelPricing, ProviderEntry, ProviderModel, ProviderRegistry, WireProtocol,
};
use systemprompt_test_fixtures::usage;

const INPUT: u32 = 3_000;
const OUTPUT: u32 = 700;
const CACHE_READ: u32 = 40_000;
const CACHE_WRITE: u32 = 9_000;

struct Row {
    provider: &'static str,
    model: &'static str,
    wire: WireProtocol,
    surface: ApiSurface,
    pricing: ModelPricing,
}

// Why: one row per provider entry shape the catalog actually carries. The
// third-party OpenAI-compatible front (Cerebras) is its own row because it
// speaks the Chat Completions wire under a different provider name, which is
// exactly the lookup that fails when the resolver keys on the wire.
fn table() -> Vec<Row> {
    vec![
        Row {
            provider: "anthropic",
            model: "claude-sonnet-4-cached",
            wire: WireProtocol::Anthropic,
            surface: ApiSurface::Anthropic,
            pricing: ModelPricing {
                input_per_million: 3.0,
                output_per_million: 15.0,
                cache_read_per_million: Some(0.3),
                cache_write_per_million: Some(3.75),
                per_image_cents: None,
            },
        },
        Row {
            provider: "openai",
            model: "gpt-4.1-mini-cached",
            wire: WireProtocol::OpenAiChat,
            surface: ApiSurface::OpenAi,
            pricing: ModelPricing {
                input_per_million: 0.4,
                output_per_million: 1.6,
                cache_read_per_million: Some(0.1),
                cache_write_per_million: Some(0.0),
                per_image_cents: None,
            },
        },
        Row {
            provider: "openai-responses",
            model: "o4-mini-cached",
            wire: WireProtocol::OpenAiResponses,
            surface: ApiSurface::OpenAi,
            pricing: ModelPricing {
                input_per_million: 1.1,
                output_per_million: 4.4,
                cache_read_per_million: Some(0.275),
                cache_write_per_million: Some(0.0),
                per_image_cents: None,
            },
        },
        Row {
            provider: "gemini",
            model: "gemini-2.5-pro-cached",
            wire: WireProtocol::Gemini,
            surface: ApiSurface::Gemini,
            pricing: ModelPricing {
                input_per_million: 1.25,
                output_per_million: 10.0,
                cache_read_per_million: Some(0.31),
                cache_write_per_million: Some(1.0),
                per_image_cents: None,
            },
        },
        Row {
            provider: "cerebras",
            model: "gpt-oss-120b-cached",
            wire: WireProtocol::OpenAiChat,
            surface: ApiSurface::OpenAi,
            pricing: ModelPricing {
                input_per_million: 0.35,
                output_per_million: 0.75,
                cache_read_per_million: Some(0.0),
                cache_write_per_million: Some(0.0),
                per_image_cents: None,
            },
        },
    ]
}

fn registry() -> ProviderRegistry {
    ProviderRegistry {
        providers: table()
            .into_iter()
            .map(|row| ProviderEntry {
                name: ProviderId::new(row.provider),
                wire: row.wire,
                surface: row.surface,
                endpoint: format!("https://{}.example.invalid/v1", row.provider),
                api_key_secret: SecretName::new(row.provider),
                governance: Default::default(),
                extra_headers: Default::default(),
                models: vec![ProviderModel {
                    id: ModelId::new(row.model),
                    aliases: Vec::new(),
                    governance: None,
                    upstream_model: None,
                    pricing: row.pricing,
                    capabilities: Default::default(),
                    limits: Default::default(),
                }],
            })
            .collect(),
    }
}

// Why: microdollars, computed the long way from the four declared rates, so a
// change to the arithmetic under test cannot be mirrored by a change to the
// expectation.
fn expected_microdollars(p: &ModelPricing) -> i64 {
    let per = |tokens: u32, rate: f64| (f64::from(tokens) / 1_000_000.0) * rate;
    let total = per(INPUT, p.input_per_million)
        + per(OUTPUT, p.output_per_million)
        + per(CACHE_READ, p.cache_read_rate())
        + per(CACHE_WRITE, p.cache_write_rate());
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the table's rates and counts keep this well inside i64"
    )]
    let micro = (total * 1_000_000.0).round() as i64;
    micro
}

#[test]
fn every_provider_entry_prices_its_own_cache_rates() {
    let registry = registry();
    let tokens = usage()
        .input(INPUT)
        .output(OUTPUT)
        .cache_read(CACHE_READ)
        .cache_creation(CACHE_WRITE)
        .build();
    for row in table() {
        let resolved = resolve(row.provider, &[row.model], None, &registry);
        assert!(
            resolved.declares_cache_rate(),
            "{}: the resolver dropped the entry's declared cache rate",
            row.provider
        );
        assert!(
            (resolved.cache_read_rate() - row.pricing.cache_read_rate()).abs() < 1e-9,
            "{}: the resolver lost the declared cache-read rate",
            row.provider
        );
        assert!(
            (resolved.cache_write_rate() - row.pricing.cache_write_rate()).abs() < 1e-9,
            "{}: the resolver lost the declared cache-write rate",
            row.provider
        );
        assert_eq!(
            resolved.cost_microdollars(&tokens),
            expected_microdollars(&row.pricing),
            "{}: the bill must be input*in + output*out + cache_read*cr + cache_write*cw from \
             this entry's own rates",
            row.provider
        );
    }
}

// Why: the double-billing defect this sweep closed, asserted per provider. If
// `input_tokens` still carried the cached slice, every entry with a cache rate
// below its input rate would over-charge by exactly the difference.
#[test]
fn no_provider_bills_the_cached_slice_twice() {
    let registry = registry();
    let exclusive = usage()
        .input(INPUT)
        .output(OUTPUT)
        .cache_read(CACHE_READ)
        .build();
    // The wrong shape: the cached slice left inside the prompt count as well.
    let overlapping = usage()
        .input(INPUT + CACHE_READ)
        .output(OUTPUT)
        .cache_read(CACHE_READ)
        .build();
    for row in table() {
        let p = resolve(row.provider, &[row.model], None, &registry);
        let correct = p.cost_microdollars(&exclusive);
        let doubled = p.cost_microdollars(&overlapping);
        assert_eq!(
            doubled - correct,
            expected_slice_at_input_rate(&p),
            "{}: the two shapes must differ by exactly the cached slice at the input rate",
            row.provider
        );
        assert!(
            correct <= doubled,
            "{}: the exclusive shape can never cost more",
            row.provider
        );
    }
}

fn expected_slice_at_input_rate(p: &ModelPricing) -> i64 {
    let dollars = (f64::from(CACHE_READ) / 1_000_000.0) * p.input_per_million;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "same bounded arithmetic as expected_microdollars"
    )]
    let micro = (dollars * 1_000_000.0).round() as i64;
    micro
}

// Why: a provider that publishes no cache price is a declared zero, not a
// forgotten one, and it must still bill its uncached classes normally.
#[test]
fn a_declared_zero_cache_rate_bills_the_cached_slice_free_not_at_the_input_rate() {
    let registry = registry();
    let p = resolve("cerebras", &["gpt-oss-120b-cached"], None, &registry);
    let with_cache = usage()
        .input(INPUT)
        .output(OUTPUT)
        .cache_read(CACHE_READ)
        .build();
    let without_cache = usage().input(INPUT).output(OUTPUT).build();
    assert_eq!(
        p.cost_microdollars(&with_cache),
        p.cost_microdollars(&without_cache),
        "a zero cache rate must add nothing to the bill"
    );
    assert!(
        p.cost_microdollars(&without_cache) > 0,
        "the uncached classes are still billed"
    );
}
