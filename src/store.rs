use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::config::GLOBAL_SCOPE;
use crate::error::{HippoError, Result};
use crate::frontmatter::{self, mapping_list, mapping_str};

pub const DEFAULT_CATEGORY: &str = "general";

#[derive(Debug, Clone)]
pub struct Memory {
    pub id: String,
    pub title: String,
    pub keywords: Vec<String>,
    pub created: String,
    pub updated: String,
    pub body: String,
    pub tags: Vec<String>,
    pub scope: String,
    pub source: Option<String>,
    pub reason: Option<String>,
    pub confidence: Option<String>,
    pub path: PathBuf,
    pub content_hash: String,
}

pub fn memory_dir(home: &Path) -> PathBuf {
    home.join("memory")
}

pub fn slugify(text: &str) -> Result<String> {
    let mut slug = String::new();
    let mut dash = false;
    for ch in text.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            dash = false;
        } else if !slug.is_empty() && !dash {
            slug.push('-');
            dash = true;
        }
    }
    let slug = slug.trim_end_matches('-').to_string();
    if slug.is_empty() {
        return Err(HippoError::Validation(
            "Cannot derive a slug from the given text.".into(),
        ));
    }
    Ok(slug)
}

pub fn content_hash(raw: &str) -> String {
    let digest = Sha256::digest(raw.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn iter_memory_files(home: &Path) -> Vec<PathBuf> {
    let root = memory_dir(home);
    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();
    files
}

fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, files);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            files.push(path);
        }
    }
}

pub fn read_memory(path: &Path) -> Result<Memory> {
    let raw = fs::read_to_string(path).map_err(HippoError::unexpected)?;
    let (mapping, body) = frontmatter::parse(&raw)?;
    let id = mapping_str(&mapping, "id").unwrap_or_default();
    let title = mapping_str(&mapping, "title").unwrap_or_default();
    let keywords = mapping_list(&mapping, "keywords")?;
    let created = mapping_str(&mapping, "created").unwrap_or_default();
    let updated = mapping_str(&mapping, "updated").unwrap_or_default();
    let reason = mapping_str(&mapping, "reason");
    let confidence = mapping_str(&mapping, "confidence");
    frontmatter::validate_fields(
        &id,
        &title,
        &keywords,
        &created,
        &updated,
        confidence.as_deref(),
        reason.as_deref(),
    )?;
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("");
    if id != stem {
        return Err(HippoError::Validation(format!(
            "id '{id}' does not match filename '{stem}'."
        )));
    }
    let tags = mapping_list(&mapping, "tags").unwrap_or_default();
    let scope = mapping_str(&mapping, "scope")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| GLOBAL_SCOPE.to_string());
    Ok(Memory {
        id,
        title,
        keywords,
        created,
        updated,
        body,
        tags,
        scope,
        source: mapping_str(&mapping, "source"),
        reason,
        confidence,
        path: path.to_path_buf(),
        content_hash: content_hash(&raw),
    })
}

pub fn load_all(home: &Path) -> Result<Vec<Memory>> {
    iter_memory_files(home)
        .iter()
        .map(|path| read_memory(path))
        .collect()
}

pub fn find_path(home: &Path, memory_id: &str) -> Result<PathBuf> {
    iter_memory_files(home)
        .into_iter()
        .find(|path| path.file_stem().and_then(|stem| stem.to_str()) == Some(memory_id))
        .ok_or_else(|| HippoError::NotFound(format!("No memory with id '{memory_id}'.")))
}

pub fn delete(home: &Path, memory_id: &str) -> Result<PathBuf> {
    let path = find_path(home, memory_id)?;
    fs::remove_file(&path).map_err(HippoError::unexpected)?;
    Ok(path)
}

pub struct NewMemory {
    pub title: String,
    pub keywords: Vec<String>,
    pub body: String,
    pub tags: Vec<String>,
    pub category: String,
    pub scope: String,
    pub memory_id: Option<String>,
    pub source: Option<String>,
    pub reason: Option<String>,
    pub confidence: Option<String>,
}

pub fn write_new(home: &Path, draft: NewMemory) -> Result<Memory> {
    let category = slugify(&draft.category)?;
    let memory_id = match draft.memory_id.clone() {
        Some(id) => id,
        None => slugify(&draft.title)?,
    };
    if iter_memory_files(home)
        .iter()
        .any(|path| path.file_stem().and_then(|stem| stem.to_str()) == Some(memory_id.as_str()))
    {
        return Err(HippoError::Validation(format!(
            "Memory id '{memory_id}' already exists."
        )));
    }
    let today = crate::ranking::today().format("%Y-%m-%d").to_string();
    let memory = Memory {
        id: memory_id.clone(),
        title: draft.title,
        keywords: draft.keywords,
        created: today.clone(),
        updated: today,
        body: draft.body,
        tags: draft.tags,
        scope: draft.scope,
        source: draft.source,
        reason: draft.reason,
        confidence: draft.confidence,
        path: memory_dir(home)
            .join(category)
            .join(format!("{memory_id}.md")),
        content_hash: String::new(),
    };
    persist(memory)
}

pub struct MemoryPatch {
    pub title: Option<String>,
    pub keywords: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub body: Option<String>,
    pub scope: Option<String>,
    pub reason: Option<String>,
    pub confidence: Option<String>,
}

pub fn update(home: &Path, memory_id: &str, patch: MemoryPatch) -> Result<Memory> {
    let path = find_path(home, memory_id)?;
    let mut memory = read_memory(&path)?;
    if let Some(title) = patch.title {
        memory.title = title;
    }
    if let Some(keywords) = patch.keywords {
        memory.keywords = keywords;
    }
    if let Some(tags) = patch.tags {
        memory.tags = tags;
    }
    if let Some(body) = patch.body {
        memory.body = body;
    }
    if let Some(scope) = patch.scope {
        memory.scope = scope;
    }
    if let Some(reason) = patch.reason {
        memory.reason = Some(reason);
    }
    if let Some(confidence) = patch.confidence {
        memory.confidence = Some(confidence);
    }
    memory.updated = crate::ranking::today().format("%Y-%m-%d").to_string();
    persist(memory)
}

fn persist(mut memory: Memory) -> Result<Memory> {
    frontmatter::validate_fields(
        &memory.id,
        &memory.title,
        &memory.keywords,
        &memory.created,
        &memory.updated,
        memory.confidence.as_deref(),
        memory.reason.as_deref(),
    )?;
    if let Some(parent) = memory.path.parent() {
        fs::create_dir_all(parent).map_err(HippoError::unexpected)?;
    }
    let raw = frontmatter::serialize(&memory);
    fs::write(&memory.path, &raw).map_err(HippoError::unexpected)?;
    memory.content_hash = content_hash(&raw);
    Ok(memory)
}
