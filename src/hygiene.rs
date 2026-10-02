use std::collections::{HashMap, HashSet};

use regex::Regex;

use crate::config::GLOBAL_SCOPE;
use crate::store::Memory;

const SUBJECT_KEYWORD: f64 = 0.34;
const SUBJECT_TITLE: f64 = 0.5;
const DUP_THRESHOLD: f64 = 0.62;
const W_KEYWORDS: f64 = 0.45;
const W_TITLE: f64 = 0.25;
const W_BODY: f64 = 0.30;
const CLAIM_WINDOW: i32 = 3;

fn affirm() -> HashSet<&'static str> {
    [
        "prefer",
        "prefers",
        "preferred",
        "use",
        "uses",
        "using",
        "always",
        "choose",
        "chooses",
        "require",
        "requires",
        "required",
    ]
    .into_iter()
    .collect()
}

fn negate() -> HashSet<&'static str> {
    [
        "not", "never", "avoid", "dont", "doesnt", "isnt", "cant", "cannot",
    ]
    .into_iter()
    .collect()
}

fn stop() -> HashSet<&'static str> {
    [
        "a", "an", "the", "and", "or", "of", "to", "for", "in", "on", "with", "from", "by", "as",
        "at", "is", "are", "be", "this", "that", "it", "its", "into", "over", "than", "do", "does",
        "did",
    ]
    .into_iter()
    .collect()
}

#[derive(Debug, Clone)]
pub struct Notice {
    pub kind: String,
    pub other_id: String,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub kind: String,
    pub a: String,
    pub b: String,
    pub score: f64,
    pub tokens: Vec<String>,
}

impl Finding {
    fn notice_for(&self, memory_id: &str) -> Notice {
        let other = if self.a == memory_id {
            &self.b
        } else {
            &self.a
        };
        let detail = if self.kind == "contradiction" {
            format!("opposing claims on: {}", self.tokens.join(", "))
        } else {
            format!("near-duplicate (score {:.2})", self.score)
        };
        Notice {
            kind: self.kind.clone(),
            other_id: other.clone(),
            detail,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Cluster {
    pub keeper: String,
    pub members: Vec<String>,
    pub score: f64,
}

#[derive(Debug, Default)]
pub struct HygieneReport {
    pub duplicates: Vec<Cluster>,
    pub contradictions: Vec<Finding>,
    pub applied: bool,
    pub removed: Vec<String>,
}

pub fn analyze(memories: &[Memory], hits: &HashMap<String, i64>) -> HygieneReport {
    let by_id: HashMap<&str, &Memory> = memories
        .iter()
        .map(|memory| (memory.id.as_str(), memory))
        .collect();
    let mut dup_pairs = Vec::new();
    let mut contradictions = Vec::new();
    for index in 0..memories.len() {
        for right in index + 1..memories.len() {
            let Some(finding) = compare_pair(&memories[index], &memories[right]) else {
                continue;
            };
            if finding.kind == "contradiction" {
                contradictions.push(finding);
            } else {
                dup_pairs.push((finding.a, finding.b, finding.score));
            }
        }
    }
    contradictions.sort_by(|left, right| left.a.cmp(&right.a).then(left.b.cmp(&right.b)));
    HygieneReport {
        duplicates: cluster(&dup_pairs, &by_id, hits),
        contradictions,
        ..HygieneReport::default()
    }
}

pub fn notices_for(memory: &Memory, others: &[Memory]) -> Vec<Notice> {
    let mut notices = Vec::new();
    for other in others {
        if other.id == memory.id {
            continue;
        }
        if let Some(finding) = compare_pair(memory, other) {
            notices.push(finding.notice_for(&memory.id));
        }
    }
    notices
}

pub fn compare_pair(left: &Memory, right: &Memory) -> Option<Finding> {
    if !compatible(&left.scope, &right.scope) {
        return None;
    }
    let kw = jaccard(&keywords(left), &keywords(right));
    let title = jaccard(&content_tokens(&left.title), &content_tokens(&right.title));
    let body = jaccard(&content_tokens(&left.body), &content_tokens(&right.body));
    let same_subject = kw >= SUBJECT_KEYWORD || title >= SUBJECT_TITLE;
    let conflict = conflicts(left, right);
    if same_subject && !conflict.is_empty() {
        let mut tokens: Vec<String> = conflict.into_iter().collect();
        tokens.sort();
        return Some(Finding {
            kind: "contradiction".into(),
            a: left.id.clone(),
            b: right.id.clone(),
            score: 0.0,
            tokens,
        });
    }
    let score = W_KEYWORDS * kw + W_TITLE * title + W_BODY * body;
    if score >= DUP_THRESHOLD {
        return Some(Finding {
            kind: "duplicate".into(),
            a: left.id.clone(),
            b: right.id.clone(),
            score: (score * 10_000.0).round() / 10_000.0,
            tokens: Vec::new(),
        });
    }
    None
}

fn jaccard(left: &HashSet<String>, right: &HashSet<String>) -> f64 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(right).count() as f64;
    let union = left.union(right).count() as f64;
    intersection / union
}

fn content_tokens(text: &str) -> HashSet<String> {
    let affirm = affirm();
    let negate = negate();
    let stop = stop();
    raw_tokens(text)
        .into_iter()
        .filter(|tok| {
            tok.len() >= 2
                && !stop.contains(tok.as_str())
                && !affirm.contains(tok.as_str())
                && !negate.contains(tok.as_str())
        })
        .collect()
}

fn keywords(memory: &Memory) -> HashSet<String> {
    memory
        .keywords
        .iter()
        .map(|keyword| keyword.trim().to_lowercase())
        .filter(|keyword| !keyword.is_empty())
        .collect()
}

fn compatible(left: &str, right: &str) -> bool {
    left == right || left == GLOBAL_SCOPE || right == GLOBAL_SCOPE
}

fn conflicts(left: &Memory, right: &Memory) -> HashSet<String> {
    let (left_yes, left_no) = claims(&format!("{}\n{}", left.title, left.body));
    let (right_yes, right_no) = claims(&format!("{}\n{}", right.title, right.body));
    left_yes
        .intersection(&right_no)
        .chain(left_no.intersection(&right_yes))
        .cloned()
        .collect()
}

fn claims(text: &str) -> (HashSet<String>, HashSet<String>) {
    let mut affirmed = HashSet::new();
    let mut negated = HashSet::new();
    for sentence in Regex::new(r"[.!?\n]+").expect("split").split(text) {
        let (yes, no) = claims_sentence(sentence);
        affirmed.extend(yes);
        negated.extend(no);
    }
    let both: HashSet<String> = affirmed.intersection(&negated).cloned().collect();
    affirmed.retain(|token| !both.contains(token));
    negated.retain(|token| !both.contains(token));
    (affirmed, negated)
}

fn claims_sentence(sentence: &str) -> (HashSet<String>, HashSet<String>) {
    let affirm = affirm();
    let negate = negate();
    let stop = stop();
    let mut affirmed = HashSet::new();
    let mut negated = HashSet::new();
    let mut mode: Option<&str> = None;
    let mut left = 0;
    for tok in raw_tokens(sentence) {
        if negate.contains(tok.as_str()) {
            mode = Some("negate");
            left = CLAIM_WINDOW;
            continue;
        }
        if tok == "over" && mode == Some("affirm") {
            mode = Some("negate");
            left = CLAIM_WINDOW;
            continue;
        }
        if affirm.contains(tok.as_str()) {
            if mode != Some("negate") {
                mode = Some("affirm");
                left = CLAIM_WINDOW;
            }
            continue;
        }
        if stop.contains(tok.as_str())
            || tok == "over"
            || mode.is_none()
            || left <= 0
            || tok.len() < 2
        {
            continue;
        }
        if mode == Some("negate") {
            negated.insert(tok);
        } else {
            affirmed.insert(tok);
        }
        left -= 1;
        if left == 0 {
            mode = None;
        }
    }
    (affirmed, negated)
}

fn raw_tokens(text: &str) -> Vec<String> {
    let mut normalized = text.to_lowercase().replace('\'', "");
    for (pattern, replacement) in [
        (r"\bno longer\b", " not "),
        (r"\binstead of\b", " not "),
        (r"\brather than\b", " not "),
        (r"\bdo not\b", " not "),
    ] {
        let regex = Regex::new(pattern).expect("phrase");
        normalized = regex.replace_all(&normalized, replacement).into_owned();
    }
    Regex::new(r"[a-z0-9]+")
        .expect("token")
        .find_iter(&normalized)
        .map(|item| item.as_str().to_string())
        .collect()
}

fn reverse_date(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_digit() {
                char::from(b'9' - (ch as u8 - b'0'))
            } else {
                ch
            }
        })
        .collect()
}

fn keeper_id(
    members: &HashSet<String>,
    by_id: &HashMap<&str, &Memory>,
    hits: &HashMap<String, i64>,
) -> String {
    members
        .iter()
        .min_by(|left, right| {
            let left_key = (
                -hits.get(left.as_str()).copied().unwrap_or(0),
                reverse_date(&by_id[left.as_str()].updated),
                (*left).clone(),
            );
            let right_key = (
                -hits.get(right.as_str()).copied().unwrap_or(0),
                reverse_date(&by_id[right.as_str()].updated),
                (*right).clone(),
            );
            left_key.cmp(&right_key)
        })
        .cloned()
        .unwrap_or_default()
}

fn cluster(
    pairs: &[(String, String, f64)],
    by_id: &HashMap<&str, &Memory>,
    hits: &HashMap<String, i64>,
) -> Vec<Cluster> {
    let mut parent: HashMap<String, String> = HashMap::new();
    fn find(parent: &mut HashMap<String, String>, node: &str) -> String {
        parent
            .entry(node.to_string())
            .or_insert_with(|| node.to_string());
        let mut current = node.to_string();
        while parent[&current] != current {
            let next = parent[&current].clone();
            parent.insert(current.clone(), parent[&next].clone());
            current = next;
        }
        current
    }
    for (left, right, _) in pairs {
        let root_left = find(&mut parent, left);
        let root_right = find(&mut parent, right);
        if root_left != root_right {
            parent.insert(root_right, root_left);
        }
    }
    let mut groups: HashMap<String, HashSet<String>> = HashMap::new();
    let mut best: HashMap<String, f64> = HashMap::new();
    for (left, right, score) in pairs {
        let root = find(&mut parent, left);
        groups.entry(root.clone()).or_default().insert(left.clone());
        groups
            .entry(root.clone())
            .or_default()
            .insert(right.clone());
        let entry = best.entry(root).or_insert(0.0);
        if *score > *entry {
            *entry = *score;
        }
    }
    let mut clusters = Vec::new();
    for (root, members) in groups {
        let keep = keeper_id(&members, by_id, hits);
        let mut ordered = vec![keep.clone()];
        let mut rest: Vec<String> = members.into_iter().filter(|id| id != &keep).collect();
        rest.sort();
        ordered.extend(rest);
        clusters.push(Cluster {
            keeper: keep,
            members: ordered,
            score: (best[&root] * 10_000.0).round() / 10_000.0,
        });
    }
    clusters.sort_by(|left, right| left.keeper.cmp(&right.keeper));
    clusters
}
