use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// How a filter value is matched against a directory name's tokens.
///
/// `Token` is the default: a value must equal a whole `-`-separated
/// component exactly (`uk` matches `uk-priv-app`, not `ukpr-app`) - the
/// same behavior this tool has always had, kept as the default so existing
/// `--only`/`--skip` usage doesn't silently change meaning underfoot.
/// `Fuzzy` matches a value anywhere inside a component (`uk` also matches
/// `ukpr-app`), for naming schemes that pack multiple codes into one
/// component with no delimiter between them. Opt-in via `cfg match fuzzy`
/// since it's strictly looser and can match more than a user expects if
/// they're used to `Token`'s exactness.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    #[default]
    Token,
    Fuzzy,
}

impl MatchMode {
    pub fn matches(self, token: &str, value: &str) -> bool {
        match self {
            MatchMode::Token => token == value,
            MatchMode::Fuzzy => token.contains(value),
        }
    }
}

impl fmt::Display for MatchMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MatchMode::Token => write!(f, "token"),
            MatchMode::Fuzzy => write!(f, "fuzzy"),
        }
    }
}

impl FromStr for MatchMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "token" => Ok(MatchMode::Token),
            "fuzzy" => Ok(MatchMode::Fuzzy),
            _ => Err(format!(
                "Match mode must be \"token\" or \"fuzzy\", got \"{s}\"."
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_token() {
        assert_eq!(MatchMode::default(), MatchMode::Token);
    }

    #[test]
    fn token_mode_requires_exact_equality() {
        assert!(MatchMode::Token.matches("uk", "uk"));
        assert!(!MatchMode::Token.matches("ukpr", "uk"));
    }

    #[test]
    fn fuzzy_mode_matches_a_substring_anywhere_in_the_token() {
        assert!(MatchMode::Fuzzy.matches("ukpr", "uk"));
        assert!(MatchMode::Fuzzy.matches("ukco", "co"));
        assert!(MatchMode::Fuzzy.matches("uk", "uk"));
        assert!(!MatchMode::Fuzzy.matches("ukpr", "fi"));
    }

    #[test]
    fn from_str_parses_both_modes_case_insensitively() {
        assert_eq!("token".parse(), Ok(MatchMode::Token));
        assert_eq!("Fuzzy".parse(), Ok(MatchMode::Fuzzy));
        assert_eq!("FUZZY".parse(), Ok(MatchMode::Fuzzy));
    }

    #[test]
    fn from_str_rejects_anything_else() {
        assert!("substring".parse::<MatchMode>().is_err());
        assert!("".parse::<MatchMode>().is_err());
    }

    #[test]
    fn display_round_trips_through_from_str() {
        assert_eq!(MatchMode::Token.to_string().parse(), Ok(MatchMode::Token));
        assert_eq!(MatchMode::Fuzzy.to_string().parse(), Ok(MatchMode::Fuzzy));
    }
}
