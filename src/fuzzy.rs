//! Thin wrapper over the skim fuzzy matcher used for both the in-level
//! type-to-filter and the global jump-mode search.

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

/// Return indices into `items` that match `query`, best score first.
/// An empty query keeps the original order and includes everything.
pub fn filter(matcher: &SkimMatcherV2, items: &[String], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..items.len()).collect();
    }
    let mut scored: Vec<(usize, i64)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, s)| matcher.fuzzy_match(s, query).map(|score| (i, score)))
        .collect();
    // Higher score first; ties keep stable original order.
    scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    scored.into_iter().map(|(i, _)| i).collect()
}
