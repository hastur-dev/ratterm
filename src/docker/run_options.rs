//! Options for `docker run`.
//!
//! Split out of `create.rs`. Assembling the argument list is pure, so the
//! order of flags and the validation rules are testable without a daemon.

/// Run options for starting a container from an image.
#[derive(Debug, Clone, Default)]
pub struct DockerRunOptions {
    /// Container name (--name).
    pub name: Option<String>,
    /// Port mappings (host:container, -p).
    pub port_mappings: Vec<String>,
    /// Volume mounts (host:container, -v).
    pub volume_mounts: Vec<String>,
    /// Environment variables (KEY=VALUE, -e).
    pub env_vars: Vec<String>,
    /// Run in detached mode (-d). Default false for interactive.
    pub detached: bool,
    /// Remove container on exit (--rm).
    pub remove_on_exit: bool,
    /// Shell to exec into (/bin/sh or /bin/bash).
    pub shell: String,
    /// Additional docker run arguments.
    pub extra_args: Vec<String>,
}

impl DockerRunOptions {
    /// Creates new run options with default shell.
    #[must_use]
    pub fn new() -> Self {
        Self {
            shell: "/bin/sh".to_string(),
            remove_on_exit: true,
            ..Default::default()
        }
    }

    /// Builds the docker run command arguments.
    #[must_use]
    pub fn build_args(&self, image: &str) -> Vec<String> {
        let mut args = Vec::with_capacity(20);

        // Always interactive with TTY for exec
        args.push("-it".to_string());

        // Container name
        if let Some(ref name) = self.name {
            args.push("--name".to_string());
            args.push(name.clone());
        }

        // Remove on exit
        if self.remove_on_exit {
            args.push("--rm".to_string());
        }

        // Port mappings
        for port in &self.port_mappings {
            args.push("-p".to_string());
            args.push(port.clone());
        }

        // Volume mounts
        for vol in &self.volume_mounts {
            args.push("-v".to_string());
            args.push(vol.clone());
        }

        // Environment variables
        for env in &self.env_vars {
            args.push("-e".to_string());
            args.push(env.clone());
        }

        // Extra args
        for extra in &self.extra_args {
            args.push(extra.clone());
        }

        // Image name
        args.push(image.to_string());

        // Shell command
        args.push(self.shell.clone());

        args
    }

    /// Validates the options.
    ///
    /// # Returns
    /// Ok(()) if valid, Err with message if invalid.
    pub fn validate(&self) -> Result<(), String> {
        // Validate port mappings format
        for port in &self.port_mappings {
            if !port.contains(':') {
                return Err(format!(
                    "Invalid port mapping: {} (expected host:container)",
                    port
                ));
            }
        }

        // Validate volume mount format
        for vol in &self.volume_mounts {
            if !vol.contains(':') {
                return Err(format!(
                    "Invalid volume mount: {} (expected host:container)",
                    vol
                ));
            }
        }

        // Validate env var format
        for env in &self.env_vars {
            if !env.contains('=') {
                return Err(format!("Invalid env var: {} (expected KEY=VALUE)", env));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_run_options_build_args() {
        let mut opts = DockerRunOptions::new();
        opts.name = Some("test-container".to_string());
        opts.port_mappings.push("8080:80".to_string());
        opts.env_vars.push("DEBUG=true".to_string());

        let args = opts.build_args("nginx:latest");

        assert!(args.contains(&"-it".to_string()));
        assert!(args.contains(&"--name".to_string()));
        assert!(args.contains(&"test-container".to_string()));
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"8080:80".to_string()));
        assert!(args.contains(&"nginx:latest".to_string()));
    }

    #[test]
    fn test_run_options_validate() {
        let mut opts = DockerRunOptions::new();
        assert!(opts.validate().is_ok());

        opts.port_mappings.push("invalid".to_string());
        assert!(opts.validate().is_err());

        opts.port_mappings.clear();
        opts.volume_mounts.push("no-colon".to_string());
        assert!(opts.validate().is_err());

        opts.volume_mounts.clear();
        opts.env_vars.push("NOEQUALS".to_string());
        assert!(opts.validate().unwrap_err().contains("KEY=VALUE"));
    }
}
