//! Terminal multiplexer types and enums.

/// Maximum number of terminal tabs.
pub const MAX_TABS: usize = 10;

/// Maximum terminals per grid (2x2).
pub const MAX_GRID_TERMINALS: usize = 4;

/// Direction for grid navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridDirection {
    /// Move focus up.
    Up,
    /// Move focus down.
    Down,
    /// Move focus left.
    Left,
    /// Move focus right.
    Right,
}

/// Split direction for terminal panes (kept for backward compatibility).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplitDirection {
    /// No split - single terminal.
    #[default]
    None,
    /// Horizontal split (top/bottom).
    Horizontal,
    /// Vertical split (left/right).
    Vertical,
}

/// Which pane is focused in a split (kept for backward compatibility).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplitFocus {
    /// First pane (top or left).
    #[default]
    First,
    /// Second pane (bottom or right).
    Second,
}

impl SplitFocus {
    /// Toggles between first and second pane.
    #[must_use]
    pub fn toggle(self) -> Self {
        match self {
            Self::First => Self::Second,
            Self::Second => Self::First,
        }
    }
}

/// Information about a tab.
#[derive(Debug, Clone)]
pub struct TabInfo {
    /// Tab index.
    pub index: usize,
    /// Tab name.
    pub name: String,
    /// Whether this is the active tab.
    pub is_active: bool,
    /// Split direction.
    pub split: SplitDirection,
}

/// Dummy type for backward compatibility - actual terminals are in grid.
#[derive(Debug, Clone, Copy)]
pub struct DummyTerminalRef;
