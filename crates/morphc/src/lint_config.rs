use anyhow::Result;
use morph_config::LintConfig;
use morph_parser::LintError;

/// Reject typos in `lint.severities` instead of silently ignoring them.
pub(crate) fn validate(config: &LintConfig) -> Result<()> {
    for (code, severity) in &config.severities {
        if !matches!(severity.as_str(), "error" | "warning") {
            anyhow::bail!(
                "invalid lint severity for `{code}`: `{severity}` (expected \"error\" or \"warning\")"
            );
        }
    }
    Ok(())
}

/// Apply project-level rule suppression and severity overrides. The linter
/// always produces its built-in severity so the cache stays reusable; the
/// project policy is applied at report time.
pub(crate) fn apply(errors: &mut Vec<LintError>, config: &LintConfig) {
    errors.retain(|error| !config.disable.iter().any(|rule| rule == &error.code));
    for error in errors.iter_mut() {
        if let Some(severity) = config.severities.get(&error.code) {
            error.severity.clone_from(severity);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lint(code: &str, severity: &str) -> LintError {
        LintError {
            severity: severity.to_string(),
            code: code.to_string(),
            message: String::new(),
            suggestion: None,
            file_path: "App.mx".to_string(),
            line: 1,
            col: 1,
        }
    }

    #[test]
    fn disable_removes_matching_rules_only() {
        let mut errors = vec![lint("mx-dup-class", "warning"), lint("mx-key-misuse", "warning")];
        let config =
            LintConfig { disable: vec!["mx-dup-class".to_string()], ..LintConfig::default() };
        apply(&mut errors, &config);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "mx-key-misuse");
    }

    #[test]
    fn severity_overrides_win() {
        let mut errors = vec![lint("mx-key-misuse", "warning"), lint("mx-tag-stub", "warning")];
        let config = LintConfig {
            severities: std::collections::HashMap::from([
                ("mx-key-misuse".to_string(), "error".to_string()),
                ("mx-tag-stub".to_string(), "warning".to_string()),
            ]),
            ..LintConfig::default()
        };
        apply(&mut errors, &config);
        assert_eq!(errors[0].severity, "error");
        assert_eq!(errors[1].severity, "warning");
    }

    #[test]
    fn invalid_severity_is_rejected() {
        let config = LintConfig {
            severities: std::collections::HashMap::from([(
                "mx-key-misuse".to_string(),
                "info".to_string(),
            )]),
            ..LintConfig::default()
        };
        assert!(validate(&config).is_err());
    }
}
