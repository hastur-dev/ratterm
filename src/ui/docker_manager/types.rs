//! Docker Manager types and enums.

use crate::docker::{DockerContainer, DockerImage, DockerItemType};

/// Maximum number of items to display in the list.
pub const MAX_DISPLAY_ITEMS: usize = 12;

/// Docker Manager mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DockerManagerMode {
    /// Viewing the container/image list.
    #[default]
    List,
    /// Discovering containers and images.
    Discovering,
    /// Configuring run options for an image.
    RunOptions,
    /// Connecting to a container (exec or run).
    Connecting,
    /// Confirming whether to run an image.
    Confirming,
}

/// Docker list section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DockerListSection {
    /// Running containers.
    #[default]
    RunningContainers,
    /// Stopped containers.
    StoppedContainers,
    /// Available images.
    Images,
}

impl DockerListSection {
    /// Moves to the next section.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::RunningContainers => Self::StoppedContainers,
            Self::StoppedContainers => Self::Images,
            Self::Images => Self::RunningContainers,
        }
    }

    /// Moves to the previous section.
    #[must_use]
    pub fn prev(self) -> Self {
        match self {
            Self::RunningContainers => Self::Images,
            Self::StoppedContainers => Self::RunningContainers,
            Self::Images => Self::StoppedContainers,
        }
    }

    /// Returns the display title for the section.
    #[must_use]
    pub fn title(&self) -> &'static str {
        match self {
            Self::RunningContainers => "Running Containers",
            Self::StoppedContainers => "Stopped Containers",
            Self::Images => "Images",
        }
    }

    /// Returns a short label for the section.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::RunningContainers => "[R]unning",
            Self::StoppedContainers => "[S]topped",
            Self::Images => "[I]mages",
        }
    }
}

/// Field being edited in run options mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunOptionsField {
    /// Container name.
    #[default]
    Name,
    /// Port mappings.
    Ports,
    /// Volume mounts.
    Volumes,
    /// Environment variables.
    EnvVars,
    /// Shell to use.
    Shell,
}

impl RunOptionsField {
    /// Moves to the next field.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Name => Self::Ports,
            Self::Ports => Self::Volumes,
            Self::Volumes => Self::EnvVars,
            Self::EnvVars => Self::Shell,
            Self::Shell => Self::Name,
        }
    }

    /// Moves to the previous field.
    #[must_use]
    pub fn prev(self) -> Self {
        match self {
            Self::Name => Self::Shell,
            Self::Ports => Self::Name,
            Self::Volumes => Self::Ports,
            Self::EnvVars => Self::Volumes,
            Self::Shell => Self::EnvVars,
        }
    }

    /// Returns the field label.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Name => "Container Name",
            Self::Ports => "Port Mappings",
            Self::Volumes => "Volume Mounts",
            Self::EnvVars => "Environment Vars",
            Self::Shell => "Shell",
        }
    }

    /// Returns the placeholder text.
    #[must_use]
    pub fn placeholder(&self) -> &'static str {
        match self {
            Self::Name => "my-container (optional)",
            Self::Ports => "8080:80, 443:443",
            Self::Volumes => "/host/path:/container/path",
            Self::EnvVars => "KEY=value, DEBUG=true",
            Self::Shell => "/bin/sh or /bin/bash",
        }
    }
}

/// Display information for a Docker item (container or image).
#[derive(Debug, Clone)]
pub enum DockerItemDisplay {
    /// A Docker container.
    Container(DockerContainer),
    /// A Docker image.
    Image(DockerImage),
}

impl DockerItemDisplay {
    /// Returns the item ID.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Container(c) => &c.id,
            Self::Image(i) => &i.id,
        }
    }

    /// Returns the display name.
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Container(c) => c.display().to_string(),
            Self::Image(i) => i.display(),
        }
    }

    /// Returns the item type.
    #[must_use]
    pub fn item_type(&self) -> DockerItemType {
        match self {
            Self::Container(c) => c.item_type(),
            Self::Image(_) => DockerItemType::Image,
        }
    }

    /// Returns a short summary.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::Container(c) => c.summary(),
            Self::Image(i) => i.summary(),
        }
    }

    /// Returns the container if this is a container.
    #[must_use]
    pub fn as_container(&self) -> Option<&DockerContainer> {
        match self {
            Self::Container(c) => Some(c),
            Self::Image(_) => None,
        }
    }

    /// Returns the image if this is an image.
    #[must_use]
    pub fn as_image(&self) -> Option<&DockerImage> {
        match self {
            Self::Container(_) => None,
            Self::Image(i) => Some(i),
        }
    }

    /// Returns true if this is a running container.
    #[must_use]
    pub fn is_running(&self) -> bool {
        match self {
            Self::Container(c) => c.is_running(),
            Self::Image(_) => false,
        }
    }
}
