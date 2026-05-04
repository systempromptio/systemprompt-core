pub(super) fn is_low_entropy_challenge(challenge: &str) -> bool {
    let Some(first_char) = challenge.chars().next() else {
        return true;
    };

    if challenge.chars().all(|c| c == first_char) {
        return true;
    }

    if has_repeating_pattern(challenge) {
        return true;
    }

    if has_sequential_run(challenge) {
        return true;
    }

    if has_low_diversity(challenge) {
        return true;
    }

    if shannon_entropy(challenge) < 3.0 {
        return true;
    }

    false
}

fn shannon_entropy(data: &str) -> f64 {
    let len = data.len() as f64;
    if len == 0.0 {
        return 0.0;
    }

    let mut freq = std::collections::HashMap::new();
    for c in data.chars() {
        *freq.entry(c).or_insert(0u64) += 1;
    }

    freq.values().fold(0.0, |entropy, &count| {
        let p = count as f64 / len;
        p.mul_add(-p.log2(), entropy)
    })
}

fn has_repeating_pattern(challenge: &str) -> bool {
    for pattern_length in 2..=4 {
        if challenge.len() >= pattern_length * 3 {
            let pattern = &challenge[..pattern_length];
            let repetitions = challenge.len() / pattern_length;
            if repetitions >= 3 {
                let repeated = pattern.repeat(repetitions);
                if challenge.starts_with(&repeated) {
                    return true;
                }
            }
        }
    }
    false
}

fn has_sequential_run(challenge: &str) -> bool {
    use systemprompt_oauth::constants::validation::MIN_SEQUENTIAL_RUN;

    let chars: Vec<char> = challenge.chars().collect();
    if chars.len() < MIN_SEQUENTIAL_RUN {
        return false;
    }

    let mut ascending_count = 1;
    let mut descending_count = 1;

    for i in 1..chars.len() {
        if let (Some(prev), Some(curr)) = (chars[i - 1].to_digit(36), chars[i].to_digit(36)) {
            if curr == prev.wrapping_add(1) {
                ascending_count += 1;
                if ascending_count >= MIN_SEQUENTIAL_RUN {
                    return true;
                }
            } else {
                ascending_count = 1;
            }

            if prev == curr.wrapping_add(1) {
                descending_count += 1;
                if descending_count >= MIN_SEQUENTIAL_RUN {
                    return true;
                }
            } else {
                descending_count = 1;
            }
        }
    }
    false
}

fn has_low_diversity(challenge: &str) -> bool {
    use systemprompt_oauth::constants::validation::{DIVERSITY_THRESHOLD, MIN_UNIQUE_CHARS};

    let unique_chars: std::collections::HashSet<char> = challenge.chars().collect();
    let entropy_ratio = unique_chars.len() as f64 / challenge.len() as f64;

    if entropy_ratio < DIVERSITY_THRESHOLD {
        return true;
    }

    let min_unique_for_length = challenge.len() / 2;
    unique_chars.len() < min_unique_for_length.min(MIN_UNIQUE_CHARS)
}
