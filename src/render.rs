use std::collections::HashMap;

use rusqlite::Connection;
use serde::Serialize;

use crate::error::{HippoError, Result};
use crate::search::Hit;

const DIGEST_TOP_TOPICS: usize = 15;

#[derive(Serialize)]
struct ManifestEntry {
    id: String,
    title: String,
    keywords: Vec<String>,
    path: String,
}

#[derive(Serialize)]
struct Topic {
    tag: String,
    count: usize,
}

#[derive(Serialize)]
struct DigestJson {
    total: usize,
    topics: Vec<Topic>,
}

pub fn manifest(conn: &Connection, as_json: bool) -> Result<String> {
    let mut stmt = conn
        .prepare("SELECT id, title, keywords, path FROM memories ORDER BY title")
        .map_err(HippoError::unexpected)?;
    let entries = stmt
        .query_map([], |row| {
            let keywords: String = row.get(2)?;
            Ok(ManifestEntry {
                id: row.get(0)?,
                title: row.get(1)?,
                keywords: keywords.split_whitespace().map(str::to_string).collect(),
                path: row.get(3)?,
            })
        })
        .map_err(HippoError::unexpected)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(HippoError::unexpected)?;
    if as_json {
        return serde_json::to_string_pretty(&entries).map_err(HippoError::unexpected);
    }
    if entries.is_empty() {
        return Ok("# Memory catalog\n\n_(empty)_".into());
    }
    let mut lines = vec!["# Memory catalog".to_string(), String::new()];
    for entry in entries {
        lines.push(format!(
            "- **{}** (`{}`) — {}",
            entry.title,
            entry.id,
            entry.keywords.join(", ")
        ));
    }
    Ok(lines.join("\n"))
}

pub fn digest(conn: &Connection, as_json: bool) -> Result<String> {
    let mut stmt = conn
        .prepare("SELECT tags FROM memories ORDER BY rowid")
        .map_err(HippoError::unexpected)?;
    let rows: Vec<String> = stmt
        .query_map([], |row| {
            row.get::<_, Option<String>>(0)
                .map(|value| value.unwrap_or_default())
        })
        .map_err(HippoError::unexpected)?
        .collect::<std::result::Result<_, _>>()
        .map_err(HippoError::unexpected)?;
    let total = rows.len();
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for tags in &rows {
        for tag in tags.split_whitespace() {
            if !counts.contains_key(tag) {
                order.push(tag.to_string());
            }
            *counts.entry(tag.to_string()).or_insert(0) += 1;
        }
    }
    let mut ranked: Vec<(String, usize)> = counts.into_iter().collect();
    ranked.sort_by(|left, right| {
        right.1.cmp(&left.1).then_with(|| {
            order
                .iter()
                .position(|tag| tag == &left.0)
                .cmp(&order.iter().position(|tag| tag == &right.0))
        })
    });
    ranked.truncate(DIGEST_TOP_TOPICS);
    let distinct = order.len();
    if as_json {
        let payload = DigestJson {
            total,
            topics: ranked
                .into_iter()
                .map(|(tag, count)| Topic { tag, count })
                .collect(),
        };
        return serde_json::to_string_pretty(&payload).map_err(HippoError::unexpected);
    }
    Ok(digest_md(total, distinct, &ranked))
}

fn digest_md(total: usize, distinct: usize, top: &[(String, usize)]) -> String {
    if total == 0 {
        return "_No memories yet._ Save durable facts with `hippo add`.".into();
    }
    let noun = if total == 1 { "memory" } else { "memories" };
    let topics = if top.is_empty() {
        "untagged".to_string()
    } else {
        top.iter()
            .map(|(tag, count)| format!("{tag} ({count})"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "You have {total} {noun} across {distinct} topics.\nTop topics: {topics}.\nNothing is preloaded — recall with `hippo query \"<text>\"`."
    )
}

pub fn context_block(hits: &[Hit]) -> String {
    if hits.is_empty() {
        return "_No relevant memories found._\n".into();
    }
    let mut lines = vec!["## Relevant memories".to_string(), String::new()];
    for hit in hits {
        lines.push(format!("### {} (`{}`)", hit.title, hit.id));
        lines.push(hit.snippet.clone());
        lines.push(format!("_source: {}_", hit.path));
        lines.push(String::new());
    }
    lines.join("\n")
}
