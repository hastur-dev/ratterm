//! Theme extension loader.
//!
//! Loads theme extensions and converts them to Theme objects.

use std::path::Path;

use crate::theme::{CustomThemeError, Theme, load_custom_theme};

use super::ExtensionError;
use super::manifest::ExtensionManifest;

/// Loads a theme from a theme extension.
pub fn load_theme_extension(
    ext_dir: &Path,
    manifest: &ExtensionManifest,
) -> Result<Theme, ExtensionError> {
    let theme_config = manifest.theme.as_ref().ok_or_else(|| {
        ExtensionError::Manifest("Theme extension missing [theme] section".to_string())
    })?;

    let theme_path = ext_dir.join(&theme_config.file);

    if !theme_path.exists() {
        return Err(ExtensionError::Manifest(format!(
            "Theme file not found: {:?}",
            theme_path
        )));
    }

    load_custom_theme(&theme_path).map_err(|e| match e {
        CustomThemeError::Io(io_err) => ExtensionError::Io(io_err),
        CustomThemeError::Parse(parse_err) => {
            ExtensionError::Manifest(format!("Theme parse error: {}", parse_err))
        }
        CustomThemeError::InvalidColor(color) => {
            ExtensionError::Manifest(format!("Invalid color in theme: {}", color))
        }
        CustomThemeError::BaseNotFound(base) => {
            ExtensionError::Manifest(format!("Base theme not found: {}", base))
        }
    })
}

/// Information about a theme extension.
#[derive(Debug, Clone)]
pub struct ThemeExtensionInfo {
    /// Extension name.
    pub name: String,
    /// Extension version.
    pub version: String,
    /// Theme display name.
    pub theme_name: String,
    /// Theme description.
    pub description: String,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_theme_extension(dir: &Path) {
        // Create extension.toml
        let manifest = r##"
[extension]
name = "test-theme"
version = "1.0.0"
type = "theme"

[theme]
file = "theme.toml"
"##;
        let mut f = fs::File::create(dir.join("extension.toml")).expect("create manifest");
        f.write_all(manifest.as_bytes()).expect("write manifest");

        // Create theme.toml
        let theme = r##"
[theme]
name = "Test Theme"
description = "A test theme"

[colors]
"terminal.background" = "#1a1a2e"
"##;
        let mut f = fs::File::create(dir.join("theme.toml")).expect("create theme");
        f.write_all(theme.as_bytes()).expect("write theme");
    }

    #[test]
    fn test_load_theme_extension() {
        let dir = TempDir::new().expect("temp dir");
        create_theme_extension(dir.path());

        let manifest_path = dir.path().join("extension.toml");
        let manifest =
            super::super::manifest::load_manifest(&manifest_path).expect("load manifest");

        let theme = load_theme_extension(dir.path(), &manifest).expect("load theme");
        assert_eq!(theme.name(), "Test Theme");
    }
}
