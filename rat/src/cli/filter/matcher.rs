use super::expression::FilterExpression;
use super::tokenizer::DirectoryTokenizer;

/// Answers exactly one question - "does this directory satisfy this
/// filter?" - by way of directory metadata (its name's components), not
/// string search over its path.
///
/// How a name becomes tokens (`DirectoryTokenizer`) and what a filter's
/// tokens are (`FilterExpression`) are each somebody else's concern; this
/// type only combines the two.
pub struct DirectoryMatcher {
    tokenizer: DirectoryTokenizer,
}

impl DirectoryMatcher {
    pub fn new(tokenizer: DirectoryTokenizer) -> Self {
        Self { tokenizer }
    }

    /// `only` and `skip` both apply, combined with AND: a directory must
    /// have at least one token that contains a value in `only` (when `only`
    /// is non-empty) AND no token that contains a value in `skip` (when
    /// `skip` is non-empty). An empty side of the filter imposes no
    /// constraint on its own.
    ///
    /// A filter value matches a token by substring, anywhere in the token,
    /// not exact equality - e.g. `uk` matches the token `ukpr` (from a
    /// directory like `ukpr-app`, country + env packed into one token with
    /// no delimiter between them) and `co` matches that same naming
    /// scheme's `ukco` even though `co` sits at the *end* of the token.
    /// This is still component-scoped, not a substring search over the
    /// whole directory name: `uk` does not match `nl-uk-app`'s `nl` token,
    /// only its `uk` token.
    pub fn matches(&self, directory_name: &str, filter: &FilterExpression) -> bool {
        let tokens = self.tokenizer.tokenize(directory_name);

        let satisfies_only = filter.only.is_empty() || any_token_contains(&tokens, &filter.only);
        let satisfies_skip = filter.skip.is_empty() || !any_token_contains(&tokens, &filter.skip);

        satisfies_only && satisfies_skip
    }
}

fn any_token_contains(
    tokens: &std::collections::HashSet<String>,
    values: &std::collections::HashSet<String>,
) -> bool {
    tokens
        .iter()
        .any(|token| values.iter().any(|value| token.contains(value.as_str())))
}

impl Default for DirectoryMatcher {
    fn default() -> Self {
        Self::new(DirectoryTokenizer::default())
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

    /// Country and environment packed into one token with no delimiter
    /// between them (`ukpr-app` is `uk` + `pr` run together, not
    /// `uk-pr-app`) - a real naming scheme where `--only`/`--skip` need to
    /// filter on either half of the token, not just its start.
    const PACKED_DIRECTORIES: &[&str] = &["ukpr-app", "ukco-app", "fipr-app", "nlco-app", "tools"];

    fn matching_within(
        directories: &[&'static str],
        filter: &FilterExpression,
    ) -> Vec<&'static str> {
        let matcher = DirectoryMatcher::default();
        directories
            .iter()
            .copied()
            .filter(|name| matcher.matches(name, filter))
            .collect()
    }

    #[test]
    fn only_uk_matches_every_uk_app_regardless_of_where_uk_sits_in_the_token() {
        // "uk" is a prefix of both "ukpr" and "ukco" - matches both, same
        // as if the folders had been named uk-pr-app/uk-co-app.
        assert_eq!(
            matching_within(PACKED_DIRECTORIES, &only(&["uk"])),
            vec!["ukpr-app", "ukco-app"]
        );
    }

    #[test]
    fn only_co_matches_every_co_app_even_though_co_is_embedded_mid_token() {
        // "co" is a *suffix* of "ukco"/"nlco", not a prefix - a filter
        // anchored to the start of the token would miss both of these.
        assert_eq!(
            matching_within(PACKED_DIRECTORIES, &only(&["co"])),
            vec!["ukco-app", "nlco-app"]
        );
    }

    #[test]
    fn only_pr_matches_every_pr_app_even_though_pr_is_embedded_mid_token() {
        assert_eq!(
            matching_within(PACKED_DIRECTORIES, &only(&["pr"])),
            vec!["ukpr-app", "fipr-app"]
        );
    }

    #[test]
    fn only_nl_matches_only_the_one_directory_that_actually_starts_with_nl() {
        // Guards against a value matching an unrelated token just because
        // some substring elsewhere happens to line up.
        assert_eq!(
            matching_within(PACKED_DIRECTORIES, &only(&["nl"])),
            vec!["nlco-app"]
        );
    }

    #[test]
    fn skip_app_leaves_only_the_directory_with_no_app_component() {
        // "app" is its own dash-separated token in every packed directory
        // (ukpr-app tokenizes to "ukpr", "app"), so --skip-app excludes all
        // of them and leaves "tools" untouched.
        assert_eq!(
            matching_within(PACKED_DIRECTORIES, &skip(&["app"])),
            vec!["tools"]
        );
    }
}
