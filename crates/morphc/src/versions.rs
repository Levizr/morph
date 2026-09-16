use anyhow::Result;
use morph_config::VersionFile;
use std::path::Path;

#[allow(dead_code)]
pub(crate) fn read_version_file(path: &Path) -> Result<VersionFile> {
    VersionFile::from_file(path)
}

pub(crate) fn check_compatibility(morphc_version: &str, runtime_version: &str) -> Compatibility {
    // Simple semver check: major must match, runtime minor <= morphc minor?
    // For now: warn if runtime is older than morphc by major version
    let morphc =
        semver::Version::parse(morphc_version).unwrap_or_else(|_| semver::Version::new(0, 0, 0));
    let runtime =
        semver::Version::parse(runtime_version).unwrap_or_else(|_| semver::Version::new(0, 0, 0));

    if runtime.major != morphc.major {
        return Compatibility::Incompatible;
    }
    if runtime.minor + 2 < morphc.minor {
        return Compatibility::Deprecated;
    }
    Compatibility::Compatible
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Compatibility {
    Compatible,
    Deprecated,
    Incompatible,
}
