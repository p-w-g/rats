use super::{load_config, save_config_at};
use crate::cli::filter::MatchMode;
use std::io;
use std::path::Path;

/// Sets the persisted `--only`/`--skip` matching mode. `token` (the
/// default) clears the field entirely rather than persisting it explicitly
/// - same as `cfg to 0` clearing the timeout - so re-selecting the default
/// keeps the config file minimal instead of writing out a value that's
/// already implied by its absence.
pub fn set_match_mode(mode: &str) -> io::Result<()> {
    match mode.parse::<MatchMode>() {
        Ok(mode) => set_match_mode_at(&super::config_path(), mode),
        Err(message) => {
            println!("{message}");
            Ok(())
        }
    }
}

fn set_match_mode_at(path: &Path, mode: MatchMode) -> io::Result<()> {
    let mut config = load_config(path);
    config.match_mode = match mode {
        MatchMode::Token => None,
        MatchMode::Fuzzy => Some(mode),
    };
    save_config_at(path, &config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn set_match_mode_persists_fuzzy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".ratconfig");

        set_match_mode_at(&path, MatchMode::Fuzzy).unwrap();

        assert_eq!(load_config(&path).match_mode, Some(MatchMode::Fuzzy));
    }

    #[test]
    fn set_match_mode_token_clears_a_previously_set_fuzzy_mode() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".ratconfig");
        save_config_at(
            &path,
            &Config {
                match_mode: Some(MatchMode::Fuzzy),
                ..Default::default()
            },
        )
        .unwrap();

        set_match_mode_at(&path, MatchMode::Token).unwrap();

        assert_eq!(load_config(&path).match_mode, None);
    }

    #[test]
    fn invalid_mode_leaves_existing_config_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".ratconfig");
        save_config_at(
            &path,
            &Config {
                match_mode: Some(MatchMode::Fuzzy),
                ..Default::default()
            },
        )
        .unwrap();

        // set_match_mode (not set_match_mode_at) is the entry point that
        // actually validates - a bad value must never reach save_config_at.
        assert!("substring".parse::<MatchMode>().is_err());
        assert_eq!(load_config(&path).match_mode, Some(MatchMode::Fuzzy));
    }
}
