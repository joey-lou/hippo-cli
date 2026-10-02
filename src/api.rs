use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::capture::{self, Assessment};
use crate::config::{self, GLOBAL_SCOPE};
use crate::error::Result;
use crate::hygiene::{self, Cluster, HygieneReport, Notice};
use crate::index::{self, ReindexResult};
use crate::ranking;
use crate::render;
use crate::search::{self, Hit};
use crate::store::{self, Memory, MemoryPatch, NewMemory};
use crate::vcs::{self, GitStatus};

#[derive(Debug, Serialize)]
pub struct StatusReport {
    pub indexed: usize,
    pub new: Vec<String>,
    pub changed: Vec<String>,
    pub removed: Vec<String>,
    pub git: GitStatus,
    pub actions: Vec<String>,
}

impl StatusReport {
    pub fn is_clean(&self) -> bool {
        self.new.is_empty() && self.changed.is_empty() && self.removed.is_empty()
    }
}

pub struct MemoryStore {
    pub home: PathBuf,
    pub scope: String,
    pub capture_warnings: Vec<String>,
}

impl MemoryStore {
    pub fn new(home: Option<&Path>, scope: Option<&str>) -> Result<Self> {
        Ok(Self {
            home: config::resolve_home(home)?,
            scope: config::resolve_scope(scope, None),
            capture_warnings: Vec::new(),
        })
    }

    pub fn query(&self, text: &str, k: usize) -> Result<Vec<Hit>> {
        self.query_at(text, k, ranking::today())
    }

    pub fn query_at(&self, text: &str, k: usize, now: chrono::NaiveDate) -> Result<Vec<Hit>> {
        let conn = index::connect(&self.home)?;
        let hits = search::query(&conn, text, k, &self.scope, now)?;
        let ids: Vec<String> = hits.iter().map(|hit| hit.id.clone()).collect();
        index::record_access(&conn, &ids, &now.format("%Y-%m-%d").to_string())?;
        Ok(hits)
    }

    pub fn manifest(&self, as_json: bool) -> Result<String> {
        let conn = index::connect(&self.home)?;
        render::manifest(&conn, as_json)
    }

    pub fn digest(&self, as_json: bool) -> Result<String> {
        let conn = index::connect(&self.home)?;
        render::digest(&conn, as_json)
    }

    pub fn add(&mut self, mut draft: NewMemory, force: bool) -> Result<Memory> {
        self.capture_warnings.clear();
        if draft.scope.is_empty() {
            draft.scope = GLOBAL_SCOPE.to_string();
        }
        if draft.category.is_empty() {
            draft.category = store::DEFAULT_CATEGORY.to_string();
        }
        self.guard(
            &draft.title,
            &draft.body,
            &draft.keywords,
            draft.confidence.as_deref(),
            force,
        )?;
        let memory = store::write_new(&self.home, draft)?;
        let conn = index::connect(&self.home)?;
        index::upsert(&conn, &memory)?;
        Ok(memory)
    }

    pub fn update(&mut self, memory_id: &str, patch: MemoryPatch, force: bool) -> Result<Memory> {
        self.capture_warnings.clear();
        let current = store::read_memory(&store::find_path(&self.home, memory_id)?)?;
        let title = patch.title.clone().unwrap_or(current.title);
        let body = patch.body.clone().unwrap_or(current.body);
        let keywords = patch.keywords.clone().unwrap_or(current.keywords);
        let confidence = patch.confidence.as_deref();
        self.guard(&title, &body, &keywords, confidence, force)?;
        let memory = store::update(&self.home, memory_id, patch)?;
        let conn = index::connect(&self.home)?;
        index::upsert(&conn, &memory)?;
        Ok(memory)
    }

    pub fn review(&self, memory: &Memory) -> Result<Vec<Notice>> {
        let others = store::load_all(&self.home)?;
        Ok(hygiene::notices_for(memory, &others))
    }

    pub fn consolidate(&self, apply: bool) -> Result<HygieneReport> {
        let memories = store::load_all(&self.home)?;
        let conn = index::connect(&self.home)?;
        let hits = index::hit_counts(&conn)?;
        let mut report = hygiene::analyze(&memories, &hits);
        if !apply {
            return Ok(report);
        }
        let by_id: HashMap<String, Memory> = memories
            .iter()
            .cloned()
            .map(|memory| (memory.id.clone(), memory))
            .collect();
        let mut removed = Vec::new();
        for cluster in &report.duplicates {
            removed.extend(self.merge_cluster(cluster, &by_id)?);
        }
        report.applied = true;
        report.removed = removed;
        Ok(report)
    }

    pub fn show(&self, memory_id: &str) -> Result<Memory> {
        store::read_memory(&store::find_path(&self.home, memory_id)?)
    }

    pub fn path(&self, memory_id: &str) -> Result<PathBuf> {
        store::find_path(&self.home, memory_id)
    }

    pub fn reindex(&self, changed_only: bool) -> Result<ReindexResult> {
        let memories = store::load_all(&self.home)?;
        let conn = index::connect(&self.home)?;
        let result = if changed_only {
            reindex_changed(&conn, &memories)?
        } else {
            reindex_all(&conn, &memories)?
        };
        Ok(result)
    }

    pub fn status(&self) -> Result<StatusReport> {
        let memories = store::load_all(&self.home)?;
        let fs_hashes: HashMap<String, String> = memories
            .iter()
            .map(|memory| (memory.id.clone(), memory.content_hash.clone()))
            .collect();
        let conn = index::connect(&self.home)?;
        let stored = index::stored_hashes(&conn)?;
        let mut new: Vec<String> = fs_hashes
            .keys()
            .filter(|id| !stored.contains_key(*id))
            .cloned()
            .collect();
        let mut removed: Vec<String> = stored
            .keys()
            .filter(|id| !fs_hashes.contains_key(*id))
            .cloned()
            .collect();
        let mut changed: Vec<String> = fs_hashes
            .iter()
            .filter(|(id, hash)| {
                stored
                    .get(*id)
                    .is_some_and(|stored_hash| stored_hash != *hash)
            })
            .map(|(id, _)| id.clone())
            .collect();
        new.sort();
        removed.sort();
        changed.sort();
        let git = vcs::status(&self.home);
        let mut report = StatusReport {
            indexed: stored.len(),
            new,
            changed,
            removed,
            git: git.clone(),
            actions: Vec::new(),
        };
        report.actions = suggest_actions(&report, &git, &memories);
        Ok(report)
    }

    fn guard(
        &mut self,
        title: &str,
        body: &str,
        keywords: &[String],
        confidence: Option<&str>,
        force: bool,
    ) -> Result<()> {
        capture::check_confidence(confidence)?;
        let assessment: Assessment = capture::assess(title, body, keywords);
        capture::gate(confidence, &assessment, force)?;
        self.capture_warnings = assessment.warnings;
        Ok(())
    }

    fn merge_cluster(
        &self,
        cluster: &Cluster,
        by_id: &HashMap<String, Memory>,
    ) -> Result<Vec<String>> {
        let keeper = &by_id[&cluster.keeper];
        let others: Vec<&Memory> = cluster
            .members
            .iter()
            .filter(|id| *id != &cluster.keeper)
            .map(|id| &by_id[id])
            .collect();
        let mut body = keeper.body.trim_end().to_string();
        for other in &others {
            let extra = other.body.trim();
            if !extra.is_empty() && !body.contains(extra) {
                body.push_str(&format!("\n\nMerged from `{}`:\n\n{extra}", other.id));
            }
        }
        let keywords = union_lists(
            std::iter::once(keeper.keywords.clone())
                .chain(others.iter().map(|other| other.keywords.clone())),
        );
        let tags = union_lists(
            std::iter::once(keeper.tags.clone())
                .chain(others.iter().map(|other| other.tags.clone())),
        );
        let memory = store::update(
            &self.home,
            &keeper.id,
            MemoryPatch {
                title: None,
                keywords: Some(keywords),
                tags: Some(tags),
                body: Some(body),
                scope: None,
                reason: None,
                confidence: None,
            },
        )?;
        let conn = index::connect(&self.home)?;
        index::upsert(&conn, &memory)?;
        let mut removed = Vec::new();
        for other in others {
            store::delete(&self.home, &other.id)?;
            index::delete(&conn, &other.id)?;
            removed.push(other.id.clone());
        }
        Ok(removed)
    }
}

fn reindex_all(conn: &rusqlite::Connection, memories: &[Memory]) -> Result<ReindexResult> {
    let stored = index::stored_hashes(conn)?;
    for memory in memories {
        index::upsert(conn, memory)?;
    }
    let live: std::collections::HashSet<&str> =
        memories.iter().map(|memory| memory.id.as_str()).collect();
    for stale_id in stored.keys() {
        if !live.contains(stale_id.as_str()) {
            index::delete(conn, stale_id)?;
        }
    }
    Ok(ReindexResult {
        added: memories.len() as i64,
        ..ReindexResult::default()
    })
}

fn reindex_changed(conn: &rusqlite::Connection, memories: &[Memory]) -> Result<ReindexResult> {
    let stored = index::stored_hashes(conn)?;
    let mut result = ReindexResult::default();
    let mut seen = std::collections::HashSet::new();
    for memory in memories {
        seen.insert(memory.id.clone());
        match stored.get(&memory.id) {
            None => {
                index::upsert(conn, memory)?;
                result.added += 1;
            }
            Some(hash) if hash != &memory.content_hash => {
                index::upsert(conn, memory)?;
                result.updated += 1;
            }
            _ => {}
        }
    }
    for stale_id in stored.keys() {
        if !seen.contains(stale_id) {
            index::delete(conn, stale_id)?;
            result.deleted += 1;
        }
    }
    Ok(result)
}

fn suggest_actions(report: &StatusReport, git: &GitStatus, memories: &[Memory]) -> Vec<String> {
    let mut actions = Vec::new();
    if !report.is_clean() {
        actions.push("hippo reindex --changed".into());
    }
    let findings = hygiene::analyze(memories, &HashMap::new());
    if !findings.duplicates.is_empty() {
        actions.push("hippo consolidate".into());
    }
    if !findings.contradictions.is_empty() {
        actions.push("resolve memory contradictions (hippo consolidate)".into());
    }
    if git.is_repo && git.dirty {
        actions.push("commit memory changes in the data repo".into());
    }
    if git.is_repo && git.ahead > 0 {
        actions.push(format!("push {} unpushed commit(s)", git.ahead));
    }
    actions
}

fn union_lists(groups: impl Iterator<Item = Vec<String>>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut merged = Vec::new();
    for group in groups {
        for item in group {
            let key = item.trim().to_lowercase();
            if !key.is_empty() && seen.insert(key) {
                merged.push(item);
            }
        }
    }
    merged
}
