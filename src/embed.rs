use sha2::{Digest, Sha256};

pub const DIM: usize = 64;

/// Hashing embedding: word tokens plus character trigrams. Not a neural model.
pub fn embed(text: &str) -> Vec<f64> {
    let mut acc = vec![0.0; DIM];
    let mut found = false;
    for feature in features(text) {
        found = true;
        let (bucket, sign) = bucket_sign(&feature);
        acc[bucket] += sign;
    }
    if !found {
        return acc;
    }
    let norm = acc.iter().map(|value| value * value).sum::<f64>().sqrt();
    if norm == 0.0 {
        return vec![0.0; DIM];
    }
    acc.iter().map(|value| value / norm).collect()
}

pub fn cosine(left: &[f64], right: &[f64]) -> f64 {
    if left.len() != right.len() || left.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0;
    let mut norm_left = 0.0;
    let mut norm_right = 0.0;
    for (a, b) in left.iter().zip(right) {
        dot += a * b;
        norm_left += a * a;
        norm_right += b * b;
    }
    if norm_left == 0.0 || norm_right == 0.0 {
        return 0.0;
    }
    dot / (norm_left.sqrt() * norm_right.sqrt())
}

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .replace(['\'', '\u{2019}', '\u{2018}'], "")
        .replace('-', "")
}

fn features(text: &str) -> Vec<String> {
    let normalized = normalize(text);
    let mut out = Vec::new();
    for token in regex_words(&normalized) {
        out.push(format!("w:{token}"));
    }
    let chars: Vec<char> = normalized.chars().collect();
    if chars.len() >= 3 {
        for index in 0..=chars.len() - 3 {
            let trigram: String = chars[index..index + 3].iter().collect();
            if trigram.chars().any(|ch| ch.is_alphanumeric()) {
                out.push(format!("c:{trigram}"));
            }
        }
    }
    out
}

fn regex_words(text: &str) -> Vec<String> {
    let pattern = regex::Regex::new(r"[a-z0-9]+").expect("word pattern");
    pattern
        .find_iter(text)
        .map(|item| item.as_str().to_string())
        .collect()
}

fn bucket_sign(feature: &str) -> (usize, f64) {
    let digest = Sha256::digest(feature.as_bytes());
    let bucket = u32::from_le_bytes(digest[0..4].try_into().expect("4 bytes")) as usize % DIM;
    let sign = if digest[4] & 1 == 1 { 1.0 } else { -1.0 };
    (bucket, sign)
}
