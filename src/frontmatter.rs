use serde_yaml::Value;

use crate::capture::check_confidence;
use crate::error::{HippoError, Result};
use crate::store::Memory;

const FENCE: &str = "---";

pub fn parse(text: &str) -> Result<(serde_yaml::Mapping, String)> {
    if !text.starts_with(FENCE) {
        return Err(HippoError::Validation(
            "Missing frontmatter fence '---' at file start.".into(),
        ));
    }
    let rest = &text[FENCE.len()..];
    let Some(end) = rest.find(FENCE) else {
        return Err(HippoError::Validation(
            "Unterminated frontmatter block.".into(),
        ));
    };
    let header = &rest[..end];
    let body = rest[end + FENCE.len()..]
        .trim_start_matches('\n')
        .to_string();
    let value: Value = serde_yaml::from_str(header).unwrap_or(Value::Null);
    let mapping = match value {
        Value::Null => serde_yaml::Mapping::new(),
        Value::Mapping(mapping) => mapping,
        _ => {
            return Err(HippoError::Validation(
                "Frontmatter must be a YAML mapping.".into(),
            ))
        }
    };
    Ok((mapping, body))
}

pub fn serialize(memory: &Memory) -> String {
    let mut lines = Vec::new();
    lines.push(format!("id: {}", yaml_scalar(&memory.id)));
    lines.push(format!("title: {}", yaml_scalar(&memory.title)));
    lines.push("keywords:".into());
    for keyword in &memory.keywords {
        lines.push(format!("- {}", yaml_scalar(keyword)));
    }
    lines.push("tags:".into());
    if memory.tags.is_empty() {
        lines.pop();
        lines.push("tags: []".into());
    } else {
        for tag in &memory.tags {
            lines.push(format!("- {}", yaml_scalar(tag)));
        }
    }
    lines.push(format!("created: {}", yaml_scalar(&memory.created)));
    lines.push(format!("updated: {}", yaml_scalar(&memory.updated)));
    lines.push(format!("scope: {}", yaml_scalar(&memory.scope)));
    if let Some(source) = memory.source.as_deref().filter(|value| !value.is_empty()) {
        lines.push(format!("source: {}", yaml_scalar(source)));
    }
    if let Some(reason) = memory.reason.as_deref().filter(|value| !value.is_empty()) {
        lines.push(format!("reason: {}", yaml_scalar(reason)));
    }
    if let Some(confidence) = memory
        .confidence
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("confidence: {}", yaml_scalar(confidence)));
    }
    format!(
        "{FENCE}\n{}\n{FENCE}\n\n{}\n",
        lines.join("\n"),
        memory.body.trim()
    )
}

pub fn validate_fields(
    id: &str,
    title: &str,
    keywords: &[String],
    created: &str,
    updated: &str,
    confidence: Option<&str>,
    reason: Option<&str>,
) -> Result<()> {
    let mut missing = Vec::new();
    if id.is_empty() {
        missing.push("id");
    }
    if title.is_empty() {
        missing.push("title");
    }
    if keywords.is_empty() {
        missing.push("keywords");
    }
    if created.is_empty() {
        missing.push("created");
    }
    if updated.is_empty() {
        missing.push("updated");
    }
    if !missing.is_empty() {
        return Err(HippoError::Validation(format!(
            "Missing required field(s): {}.",
            missing.join(", ")
        )));
    }
    let _ = reason;
    check_confidence(confidence)?;
    Ok(())
}

fn yaml_scalar(value: &str) -> String {
    if value.is_empty()
        || value.chars().any(|ch| {
            matches!(
                ch,
                ':' | '#'
                    | '\''
                    | '"'
                    | '\n'
                    | ','
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '&'
                    | '*'
                    | '!'
                    | '|'
                    | '>'
                    | '%'
                    | '@'
                    | '`'
            )
        })
        || value.starts_with(|ch: char| ch.is_ascii_whitespace())
    {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

pub fn mapping_str(mapping: &serde_yaml::Mapping, key: &str) -> Option<String> {
    let value = mapping.get(Value::String(key.into()))?;
    scalar_string(value)
}

pub fn mapping_list(mapping: &serde_yaml::Mapping, key: &str) -> Result<Vec<String>> {
    let Some(value) = mapping.get(Value::String(key.into())) else {
        return Ok(Vec::new());
    };
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Sequence(items) => items
            .iter()
            .map(|item| {
                scalar_string(item).ok_or_else(|| {
                    HippoError::Validation(format!("'{key}' must be a non-empty list."))
                })
            })
            .collect(),
        _ => Err(HippoError::Validation(format!(
            "'{key}' must be a non-empty list."
        ))),
    }
}

fn scalar_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}
