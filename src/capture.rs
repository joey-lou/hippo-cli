use regex::Regex;

use crate::error::{HippoError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assessment {
    pub suggested: String,
    pub warnings: Vec<String>,
}

pub fn assess(title: &str, body: &str, keywords: &[String]) -> Assessment {
    let mut suggested = "high".to_string();
    let mut warnings = Vec::new();
    if body.trim().chars().count() < 40 {
        warnings.push("body is shorter than 40 characters".to_string());
        suggested = at_most(&suggested, "medium");
    }
    if title.trim().chars().count() < 8 {
        warnings.push("title is shorter than 8 characters".to_string());
        suggested = at_most(&suggested, "medium");
    }
    if keywords.len() < 2 {
        warnings.push("fewer than 2 keywords".to_string());
        suggested = at_most(&suggested, "medium");
    }
    let mut secret_text = format!("{title}\n{body}");
    for keyword in keywords {
        secret_text.push('\n');
        secret_text.push_str(keyword);
    }
    let secret = Regex::new(
        r"password=|api_key=|secret=|\bsk-[A-Za-z0-9]{16,}|\bAKIA[A-Za-z0-9]{16}|[0-9]{12,}",
    )
    .expect("secret pattern");
    if secret.is_match(&secret_text) {
        warnings.push("text looks like a secret".to_string());
        suggested = "low".to_string();
    }
    Assessment {
        suggested,
        warnings,
    }
}

pub fn gate(confidence: Option<&str>, assessment: &Assessment, force: bool) -> Result<()> {
    if force {
        return Ok(());
    }
    let mut blocked = Vec::new();
    if assessment.suggested == "low" {
        blocked.push("suggested confidence is low");
    }
    if confidence == Some("low") {
        blocked.push("confidence is low");
    }
    if blocked.is_empty() {
        return Ok(());
    }
    let mut message = format!("Refusing to save: {}.", blocked.join("; "));
    if !assessment.warnings.is_empty() {
        message.push(' ');
        message.push_str(&assessment.warnings.join("; "));
        message.push('.');
    }
    message.push_str(" Pass --force to store it anyway.");
    Err(HippoError::Validation(message))
}

pub fn check_confidence(confidence: Option<&str>) -> Result<()> {
    match confidence {
        None => Ok(()),
        Some("high" | "medium" | "low") => Ok(()),
        Some(other) => Err(HippoError::Validation(format!(
            "'confidence' must be high, medium, or low (got '{other}')."
        ))),
    }
}

fn at_most(suggested: &str, ceiling: &str) -> String {
    if level(suggested) > level(ceiling) {
        ceiling.to_string()
    } else {
        suggested.to_string()
    }
}

fn level(value: &str) -> i32 {
    match value {
        "low" => 0,
        "medium" => 1,
        _ => 2,
    }
}
