use super::expression::FilterExpression;
use super::mode::MatchMode;
use super::tokenizer::DirectoryTokenizer;

/// Answers exactly one question - "does this directory satisfy this
/// filter?" - by way of directory metadata (its name's components), not
/// string search over its path.
///
/// How a name becomes tokens (`DirectoryTokenizer`), what a filter's tokens
/// are (`FilterExpression`), and how a value matches a token (`MatchMode`)
/// are each somebody else's concern; this type only combines the three.
pub struct DirectoryMatcher {
    tokenizer: DirectoryTokenizer,
    mode: MatchMode,
}

impl DirectoryMatcher {
    pub fn new(tokenizer: DirectoryTokenizer, mode: MatchMode) -> Self {
        Self { tokenizer, mode }
    }

    /// `only` and `skip` both apply, combined with AND: a directory must
    /// have at least one token matching a value in `only` (when `only` is
    /// non-empty) AND no token matching a value in `skip` (when `skip` is
    /// non-empty), per `self.mode`'s definition of "matching". An empty
    /// side of the filter imposes no constraint on its own.
    pub fn matches(&self, directory_name: &str, filter: &FilterExpression) -> bool {
        let tokens = self.tokenizer.tokenize(directory_name);

        let satisfies_only =
            filter.only.is_empty() || self.any_token_matches(&tokens, &filter.only);
        let satisfies_skip =
            filter.skip.is_empty() || !self.any_token_matches(&tokens, &filter.skip);

        satisfies_only && satisfies_skip
    }

    fn any_token_matches(
        &self,
        tokens: &std::collections::HashSet<String>,
        values: &std::collections::HashSet<String>,
    ) -> bool {
        tokens
            .iter()
            .any(|token| values.iter().any(|value| self.mode.matches(token, value)))
    }
}

impl Default for DirectoryMatcher {
    fn default() -> Self {
        Self::new(DirectoryTokenizer::default(), MatchMode::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRECTORIES: &[&str] = &[
        "uk-priv-app",
        "uk-corp-app",
        "fi-priv-app",
        "fi-corp-app",
        "nl-priv-app",
        "at-corp-app",
    ];

    fn matching(filter: &FilterExpression) -> Vec<&'static str> {
        let matcher = DirectoryMatcher::default();
        DIRECTORIES
            .iter()
            .copied()
            .filter(|name| matcher.matches(name, filter))
            .collect()
    }

    fn only(values: &[&str]) -> FilterExpression {
        let values: Vec<String> = values.iter().map(|s| s.to_string()).collect();
        FilterExpression::new(Some(&values), None)
    }

    fn skip(values: &[&str]) -> FilterExpression {
        let values: Vec<String> = values.iter().map(|s| s.to_string()).collect();
        FilterExpression::new(None, Some(&values))
    }

    fn only_and_skip(only_values: &[&str], skip_values: &[&str]) -> FilterExpression {
        let only: Vec<String> = only_values.iter().map(|s| s.to_string()).collect();
        let skip: Vec<String> = skip_values.iter().map(|s| s.to_string()).collect();
        FilterExpression::new(Some(&only), Some(&skip))
    }

    #[test]
    fn no_filter_matches_everything() {
        assert_eq!(matching(&FilterExpression::default()), DIRECTORIES.to_vec());
    }

    #[test]
    fn only_uk_selects_both_uk_directories() {
        assert_eq!(matching(&only(&["uk"])), vec!["uk-priv-app", "uk-corp-app"]);
    }

    #[test]
    fn only_fi_selects_both_fi_directories() {
        assert_eq!(matching(&only(&["fi"])), vec!["fi-priv-app", "fi-corp-app"]);
    }

    #[test]
    fn only_app_selects_every_app() {
        assert_eq!(matching(&only(&["app"])), DIRECTORIES.to_vec());
    }

    #[test]
    fn only_corp_selects_every_corporate_app() {
        assert_eq!(
            matching(&only(&["corp"])),
            vec!["uk-corp-app", "fi-corp-app", "at-corp-app"]
        );
    }

    #[test]
    fn skip_priv_excludes_every_private_app() {
        assert_eq!(
            matching(&skip(&["priv"])),
            vec!["uk-corp-app", "fi-corp-app", "at-corp-app"]
        );
    }

    #[test]
    fn only_uk_and_skip_corp_combine_with_and_semantics() {
        assert_eq!(
            matching(&only_and_skip(&["uk"], &["corp"])),
            vec!["uk-priv-app"]
        );
    }

    #[test]
    fn only_app_skip_fi_excludes_finnish_apps_only() {
        assert_eq!(
            matching(&only_and_skip(&["app"], &["fi"])),
            vec!["uk-priv-app", "uk-corp-app", "nl-priv-app", "at-corp-app"]
        );
    }

    #[test]
    fn only_with_multiple_values_is_or_semantics() {
        assert_eq!(
            matching(&only(&["uk", "fi"])),
            vec!["uk-priv-app", "uk-corp-app", "fi-priv-app", "fi-corp-app"]
        );
    }

    #[test]
    fn only_app_skip_multiple_values_excludes_all_of_them() {
        assert_eq!(
            matching(&only_and_skip(&["app"], &["uk", "nl"])),
            vec!["fi-priv-app", "fi-corp-app", "at-corp-app"]
        );
    }

    #[test]
    fn matching_is_scoped_to_the_directory_name_not_its_full_path() {
        // A component matcher should judge a directory by its own name, not
        // by substring search over its full path - a parent folder that
        // happens to contain a filter word must not affect the result.
        let matcher = DirectoryMatcher::default();
        let filter = only(&["repos"]);
        assert!(!matcher.matches("uk-priv-app", &filter));
    }

    #[test]
    fn default_mode_is_token_which_does_not_match_a_component_that_merely_contains_the_value() {
        // ukpr-app has no dash separating "uk" from the rest, so its token
        // is "ukpr", not "uk" - the default (Token) mode must not match it,
        // unlike Fuzzy mode below. This is the behavior this tool has
        // always had; Fuzzy is opt-in via `cfg match fuzzy`, not a silent
        // change to what --only-uk means by default.
        let matcher = DirectoryMatcher::default();
        let filter = only(&["uk"]);
        assert!(!matcher.matches("ukpr-app", &filter));
    }

    /// Country and environment packed into one token with no delimiter
    /// between them (`ukpr-app` is `uk` + `pr` run together, not
    /// `uk-pr-app`) - the real naming scheme Fuzzy mode exists for, since
    /// Token mode can never match either half of it.
    const PACKED_DIRECTORIES: &[&str] = &["ukpr-app", "ukco-app", "fipr-app", "nlco-app", "tools"];

    fn matching_fuzzy(
        directories: &[&'static str],
        filter: &FilterExpression,
    ) -> Vec<&'static str> {
        let matcher = DirectoryMatcher::new(DirectoryTokenizer::default(), MatchMode::Fuzzy);
        directories
            .iter()
            .copied()
            .filter(|name| matcher.matches(name, filter))
            .collect()
    }

    #[test]
    fn fuzzy_only_uk_matches_every_uk_app_regardless_of_where_uk_sits_in_the_token() {
        assert_eq!(
            matching_fuzzy(PACKED_DIRECTORIES, &only(&["uk"])),
            vec!["ukpr-app", "ukco-app"]
        );
    }

    #[test]
    fn fuzzy_only_co_matches_every_co_app_even_though_co_is_a_suffix() {
        assert_eq!(
            matching_fuzzy(PACKED_DIRECTORIES, &only(&["co"])),
            vec!["ukco-app", "nlco-app"]
        );
    }

    #[test]
    fn fuzzy_only_pr_matches_every_pr_app_even_though_pr_is_a_suffix() {
        assert_eq!(
            matching_fuzzy(PACKED_DIRECTORIES, &only(&["pr"])),
            vec!["ukpr-app", "fipr-app"]
        );
    }

    #[test]
    fn fuzzy_skip_app_leaves_only_the_directory_with_no_app_component() {
        assert_eq!(
            matching_fuzzy(PACKED_DIRECTORIES, &skip(&["app"])),
            vec!["tools"]
        );
    }

    #[test]
    fn fuzzy_mode_is_still_scoped_to_the_directory_name_not_its_full_path() {
        let matcher = DirectoryMatcher::new(DirectoryTokenizer::default(), MatchMode::Fuzzy);
        let filter = only(&["repos"]);
        assert!(!matcher.matches("uk-priv-app", &filter));
    }
}
