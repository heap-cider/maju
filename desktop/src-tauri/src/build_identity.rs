//! Compile-time identity for reusable named demo builds.
//!
//! Production builds leave `MAJU_DESKTOP_BUILD_DEMO_SLUG` unset and retain all
//! existing names. The demo recipe validates one slug and `build.rs` bakes it
//! into the binary; every runtime identity is then derived from that one value.

use std::borrow::Cow;

pub(crate) fn demo_slug() -> Option<&'static str> {
    option_env!("MAJU_DESKTOP_BUILD_DEMO_SLUG")
}

pub(crate) fn is_demo_build() -> bool {
    demo_slug().is_some()
}

pub(crate) const DEMO_AGENT_CONFIG_ENV: &str = "MAJU_AGENT_CONFIG_DIR";

pub(crate) fn demo_config_home() -> Result<Option<std::path::PathBuf>, String> {
    demo_config_home_for(demo_slug(), dirs::config_dir())
}

pub(crate) fn demo_agent_oauth_cache_dir() -> Result<Option<std::path::PathBuf>, String> {
    Ok(demo_config_home()?.map(|dir| dir.join("maju-agent").join("oauth")))
}

/// Keep child config caches inside this demo build's identity. In particular,
/// bundled maju-agent OAuth tokens must not read or write production's root.
/// Refuse launch if a demo cannot resolve its root; None means production only.
pub(crate) fn apply_demo_config_home(command: &mut std::process::Command) -> Result<(), String> {
    if let Some(config_home) = demo_config_home()? {
        command.env(DEMO_AGENT_CONFIG_ENV, config_home);
    }
    Ok(())
}

fn demo_config_home_for(
    demo_slug: Option<&str>,
    config_dir: Option<std::path::PathBuf>,
) -> Result<Option<std::path::PathBuf>, String> {
    match demo_slug {
        None => Ok(None),
        Some(slug) => config_dir
            .map(|dir| Some(dir.join(format!("maju-demo-{slug}"))))
            .ok_or_else(|| "cannot resolve demo credential directory".to_string()),
    }
}

pub(crate) fn deep_link_scheme() -> Cow<'static, str> {
    demo_slug()
        .map(|slug| Cow::Owned(format!("maju-demo-{slug}")))
        .unwrap_or(Cow::Borrowed("maju"))
}

pub(crate) fn is_deep_link_for_build(value: &str) -> bool {
    is_deep_link_for_scheme(value, deep_link_scheme().as_ref())
}

fn is_deep_link_for_scheme(value: &str, scheme: &str) -> bool {
    value
        .strip_prefix(scheme)
        .is_some_and(|suffix| suffix.starts_with("://"))
}

pub(crate) fn keyring_service() -> Cow<'static, str> {
    demo_slug()
        .map(|slug| Cow::Owned(format!("maju-desktop-demo.{slug}")))
        .unwrap_or(Cow::Borrowed("maju-desktop"))
}

pub(crate) fn nest_name(is_dev: bool) -> Cow<'static, str> {
    nest_name_for(demo_slug(), is_dev)
}

fn nest_name_for(demo_slug: Option<&str>, is_dev: bool) -> Cow<'_, str> {
    if let Some(slug) = demo_slug {
        Cow::Owned(format!(".maju-demo-{slug}"))
    } else if is_dev {
        Cow::Borrowed(".maju-dev")
    } else {
        Cow::Borrowed(".maju")
    }
}

pub(crate) fn cli_name(is_dev: bool) -> String {
    if let Some(slug) = demo_slug() {
        format!("maju-demo-{slug}")
    } else if is_dev {
        "maju-dev".to_string()
    } else {
        "maju".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "compiled with MAJU_BUILD_DEMO_SLUG by the compiled-flags recipe"]
    fn compiled_demo_slug_matches_expected() {
        let expected = std::env::var("MAJU_TEST_EXPECTED_DEMO_SLUG")
            .expect("MAJU_TEST_EXPECTED_DEMO_SLUG must be set");
        assert_eq!(demo_slug(), Some(expected.as_str()));
    }

    #[test]
    fn ordinary_release_defaults_remain_production_identity() {
        if demo_slug().is_none() {
            assert_eq!(deep_link_scheme(), "maju");
            assert_eq!(keyring_service(), "maju-desktop");
            assert_eq!(nest_name(false), ".maju");
            assert_eq!(cli_name(false), "maju");
        }
    }

    #[test]
    fn demo_agent_config_and_oauth_roots_are_build_scoped() {
        let base = std::path::PathBuf::from("/Users/demo/Library/Application Support");
        assert_eq!(
            demo_config_home_for(None, Some(base.clone())).unwrap(),
            None
        );
        let first = demo_config_home_for(Some("board-1234567812345678"), Some(base.clone()))
            .unwrap()
            .unwrap();
        let second = demo_config_home_for(Some("board-8765432187654321"), Some(base))
            .unwrap()
            .unwrap();
        assert_eq!(
            first,
            std::path::PathBuf::from(
                "/Users/demo/Library/Application Support/maju-demo-board-1234567812345678"
            )
        );
        assert_eq!(
            first.join("maju-agent/oauth"),
            std::path::PathBuf::from(
                "/Users/demo/Library/Application Support/maju-demo-board-1234567812345678/maju-agent/oauth"
            )
        );
        assert_ne!(first, second);
    }

    #[test]
    fn unresolved_demo_credentials_never_select_production_defaults() {
        assert_eq!(demo_config_home_for(None, None).unwrap(), None);
        assert_eq!(
            demo_config_home_for(Some("board-1234567812345678"), None),
            Err("cannot resolve demo credential directory".to_string())
        );
    }

    #[test]
    fn duplicate_instance_links_follow_the_build_scheme() {
        assert!(is_deep_link_for_scheme("maju://message?id=1", "maju"));
        assert!(!is_deep_link_for_scheme(
            "maju-demo-board-1234567812345678://message?id=1",
            "maju"
        ));
        assert!(is_deep_link_for_scheme(
            "maju-demo-board-1234567812345678://message?id=1",
            "maju-demo-board-1234567812345678"
        ));
        assert!(!is_deep_link_for_scheme(
            "maju://message?id=1",
            "maju-demo-board-1234567812345678"
        ));
    }

    #[test]
    fn production_and_named_demo_nests_are_distinct() {
        assert_eq!(nest_name_for(None, false), ".maju");
        assert_eq!(
            nest_name_for(Some("workstream-board"), false),
            ".maju-demo-workstream-board"
        );
        assert_eq!(
            nest_name_for(Some("second-demo"), false),
            ".maju-demo-second-demo"
        );
    }
}
