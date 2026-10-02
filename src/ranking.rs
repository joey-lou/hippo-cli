pub const SCOPE_BOOST: f64 = 1.0;
pub const RECENCY_WEIGHT: f64 = 0.5;
pub const USAGE_WEIGHT: f64 = 0.5;
pub const EMBED_WEIGHT: f64 = 1.0;
pub const RECENCY_HALFLIFE_DAYS: f64 = 30.0;
pub const USAGE_SATURATION: f64 = 3.0;

pub fn recency_boost(updated: &str, now: chrono::NaiveDate) -> f64 {
    let Some(updated_date) = parse_date(updated) else {
        return 0.0;
    };
    let age_days = (now - updated_date).num_days().max(0) as f64;
    RECENCY_WEIGHT * 0.5_f64.powf(age_days / RECENCY_HALFLIFE_DAYS)
}

pub fn usage_boost(hits: i64) -> f64 {
    let hits = hits.max(0) as f64;
    USAGE_WEIGHT * hits / (hits + USAGE_SATURATION)
}

pub fn adjust(
    bm25: f64,
    scope_match: bool,
    updated: &str,
    hits: i64,
    now: chrono::NaiveDate,
    embed_boost: f64,
) -> f64 {
    let mut boost = recency_boost(updated, now) + usage_boost(hits) + embed_boost;
    if scope_match {
        boost += SCOPE_BOOST;
    }
    bm25 - boost
}

pub fn parse_date(value: &str) -> Option<chrono::NaiveDate> {
    let head = value.get(..10.min(value.len()))?;
    chrono::NaiveDate::parse_from_str(head, "%Y-%m-%d").ok()
}

pub fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}
