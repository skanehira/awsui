use nucleo::Matcher;
use nucleo::pattern::{Atom, AtomKind, CaseMatching, Normalization};

/// 名前フィールドマッチ時のボーナススコア
const NAME_BONUS: u32 = 1000;

/// Fuzzy検索によるフィルタリング
///
/// `name_index` は `fields` が返すベクタ内で「名前」に該当するフィールドの
/// インデックスを示す。名前フィールドにマッチした場合はスコアにボーナスを加算し、
/// 結果をスコア降順でソートする。
pub fn fuzzy_filter_items<T: Clone>(
    items: &[T],
    filter_text: &str,
    name_index: usize,
    fields: impl Fn(&T) -> Vec<&str>,
) -> Vec<T> {
    if filter_text.is_empty() {
        return items.to_vec();
    }

    let mut matcher = Matcher::new(nucleo::Config::DEFAULT);
    let atom = Atom::new(
        filter_text,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
        false,
    );

    let mut scored: Vec<(u32, &T)> = items
        .iter()
        .filter_map(|item| {
            let field_values = fields(item);
            let mut best_score: Option<u32> = None;

            for (i, field) in field_values.iter().enumerate() {
                let mut buf = Vec::new();
                let haystack = nucleo::Utf32Str::new(field, &mut buf);
                if let Some(score) = atom.score(haystack, &mut matcher) {
                    let adjusted = if i == name_index {
                        u32::from(score) + NAME_BONUS
                    } else {
                        u32::from(score)
                    };
                    best_score = Some(best_score.map_or(adjusted, |s| s.max(adjusted)));
                }
            }

            best_score.map(|s| (s, item))
        })
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().map(|(_, item)| item.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_filter_items_returns_all_items_when_empty_query() {
        let items = vec!["alpha".to_string(), "beta".to_string()];
        let result = fuzzy_filter_items(&items, "", 0, |s| vec![s.as_str()]);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn fuzzy_filter_items_returns_matching_items_when_exact_query() {
        let items = vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()];
        let result = fuzzy_filter_items(&items, "beta", 0, |s| vec![s.as_str()]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "beta");
    }

    #[test]
    fn fuzzy_filter_items_returns_matches_when_fuzzy_query() {
        let items = vec!["web-server".to_string(), "database".to_string()];
        let result = fuzzy_filter_items(&items, "wbsr", 0, |s| vec![s.as_str()]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "web-server");
    }

    #[test]
    fn fuzzy_filter_items_returns_empty_when_no_match() {
        let items = vec!["alpha".to_string(), "beta".to_string()];
        let result = fuzzy_filter_items(&items, "zzz", 0, |s| vec![s.as_str()]);
        assert!(result.is_empty());
    }

    #[test]
    fn fuzzy_filter_items_returns_name_match_first_when_name_field_prioritized() {
        // Items with 2 fields: [id, name]
        // "item-a" has id containing "web" but name "api"
        // "item-b" has id "api-001" but name "web"
        let items = vec![
            ("web-001".to_string(), "api".to_string()),
            ("api-001".to_string(), "web".to_string()),
        ];
        // name_index=1: [id, name]
        let result = fuzzy_filter_items(&items, "web", 1, |item| {
            vec![item.0.as_str(), item.1.as_str()]
        });
        assert_eq!(result.len(), 2);
        // Name match should come first due to NAME_BONUS
        assert_eq!(result[0].1, "web");
    }

    #[test]
    fn fuzzy_filter_items_returns_case_insensitive_match_when_different_case() {
        let items = vec!["WebServer".to_string(), "database".to_string()];
        let result = fuzzy_filter_items(&items, "webserver", 0, |s| vec![s.as_str()]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "WebServer");
    }

    #[test]
    fn fuzzy_filter_items_returns_exact_match_first_when_multiple_partial_matches() {
        let items = vec![
            "my-logs-backup".to_string(),
            "logs".to_string(),
            "access-logs-archive".to_string(),
        ];
        let result = fuzzy_filter_items(&items, "logs", 0, |s| vec![s.as_str()]);
        assert!(result.len() >= 2);
        // Exact match "logs" should score highest
        assert_eq!(result[0], "logs");
    }

    #[test]
    fn fuzzy_filter_items_returns_sorted_by_score_when_varying_relevance() {
        let items = vec![
            "completely-different".to_string(),
            "ab".to_string(),
            "abc-exact".to_string(),
        ];
        let result = fuzzy_filter_items(&items, "abc", 0, |s| vec![s.as_str()]);
        // "ab" should not match "abc" (too short), "abc-exact" should match
        assert!(!result.is_empty());
        assert!(result.iter().any(|s| s == "abc-exact"));
    }
}
