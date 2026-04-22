#[derive(Debug, Clone, Copy)]
pub struct ModelPricing {
    pub input_cost_per_1k: f32,
    pub output_cost_per_1k: f32,
}

impl ModelPricing {
    pub const fn zero() -> Self {
        Self {
            input_cost_per_1k: 0.0,
            output_cost_per_1k: 0.0,
        }
    }
}

pub fn lookup(provider: &str, model: &str) -> ModelPricing {
    let model_lc = model.to_ascii_lowercase();
    let m = model_lc.as_str();

    if provider.eq_ignore_ascii_case("anthropic") {
        return match m {
            x if x.starts_with("claude-opus-4-7") => ModelPricing {
                input_cost_per_1k: 15.0,
                output_cost_per_1k: 75.0,
            },
            x if x.starts_with("claude-opus-4") => ModelPricing {
                input_cost_per_1k: 15.0,
                output_cost_per_1k: 75.0,
            },
            x if x.starts_with("claude-sonnet-4") => ModelPricing {
                input_cost_per_1k: 3.0,
                output_cost_per_1k: 15.0,
            },
            x if x.starts_with("claude-haiku-4") => ModelPricing {
                input_cost_per_1k: 1.0,
                output_cost_per_1k: 5.0,
            },
            _ => unknown(provider, model),
        };
    }

    if provider.eq_ignore_ascii_case("minimax") {
        return match m {
            x if x.contains("minimax-m") => ModelPricing {
                input_cost_per_1k: 0.2,
                output_cost_per_1k: 1.1,
            },
            _ => ModelPricing {
                input_cost_per_1k: 0.2,
                output_cost_per_1k: 1.1,
            },
        };
    }

    if provider.eq_ignore_ascii_case("openai") {
        return match m {
            x if x.starts_with("gpt-4o-mini") => ModelPricing {
                input_cost_per_1k: 0.15,
                output_cost_per_1k: 0.6,
            },
            x if x.starts_with("gpt-4o") => ModelPricing {
                input_cost_per_1k: 2.5,
                output_cost_per_1k: 10.0,
            },
            _ => unknown(provider, model),
        };
    }

    unknown(provider, model)
}

fn unknown(provider: &str, model: &str) -> ModelPricing {
    tracing::warn!(
        provider = provider,
        model = model,
        "Gateway pricing lookup: no entry for (provider, model) — cost_microdollars will be 0"
    );
    ModelPricing::zero()
}

pub fn cost_microdollars(pricing: ModelPricing, input_tokens: u32, output_tokens: u32) -> i64 {
    let input = f64::from(input_tokens);
    let output = f64::from(output_tokens);
    let input_cost = (input / 1000.0) * f64::from(pricing.input_cost_per_1k);
    let output_cost = (output / 1000.0) * f64::from(pricing.output_cost_per_1k);
    ((input_cost + output_cost) * 1_000_000.0).round() as i64
}
