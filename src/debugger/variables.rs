//! Variable types for the debugger variable inspector.

/// A debugger variable (local, argument, or watch expression result).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    /// Variable name.
    pub name: String,
    /// Variable value as a string.
    pub value: String,
    /// Type name (if available).
    pub type_name: Option<String>,
    /// Variables reference for structured/expandable variables.
    /// Non-zero means this variable has children that can be fetched.
    pub variables_reference: i64,
    /// Whether this variable's children are currently expanded in the UI.
    pub expanded: bool,
    /// Nesting depth for display indentation.
    pub depth: usize,
}

impl Variable {
    /// Creates a new variable with the given name and value.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            type_name: None,
            variables_reference: 0,
            expanded: false,
            depth: 0,
        }
    }

    /// Sets the type name.
    #[must_use]
    pub fn with_type(mut self, type_name: impl Into<String>) -> Self {
        self.type_name = Some(type_name.into());
        self
    }

    /// Sets the variables reference (for expandable variables).
    #[must_use]
    pub fn with_reference(mut self, reference: i64) -> Self {
        self.variables_reference = reference;
        self
    }

    /// Sets the nesting depth.
    #[must_use]
    pub fn with_depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }

    /// Returns true if this variable has children that can be expanded.
    #[must_use]
    pub fn is_expandable(&self) -> bool {
        self.variables_reference > 0
    }

    /// Returns a one-line display string for the variable.
    #[must_use]
    pub fn display_line(&self) -> String {
        let indent = "  ".repeat(self.depth);
        let expand_marker = if self.is_expandable() {
            if self.expanded { "v " } else { "> " }
        } else {
            "  "
        };

        if let Some(ref type_name) = self.type_name {
            format!("{}{}{}: {} = {}", indent, expand_marker, self.name, type_name, self.value)
        } else {
            format!("{}{}{} = {}", indent, expand_marker, self.name, self.value)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_new() {
        let var = Variable::new("x", "42");
        assert_eq!(var.name, "x");
        assert_eq!(var.value, "42");
        assert!(var.type_name.is_none());
        assert_eq!(var.variables_reference, 0);
        assert!(!var.expanded);
        assert_eq!(var.depth, 0);
    }

    #[test]
    fn test_variable_with_type() {
        let var = Variable::new("count", "10").with_type("i32");
        assert_eq!(var.type_name.as_deref(), Some("i32"));
    }

    #[test]
    fn test_variable_is_expandable() {
        let simple = Variable::new("x", "1");
        assert!(!simple.is_expandable());

        let complex = Variable::new("vec", "[1, 2, 3]").with_reference(5);
        assert!(complex.is_expandable());
    }

    #[test]
    fn test_variable_display_line_simple() {
        let var = Variable::new("x", "42");
        assert_eq!(var.display_line(), "  x = 42");
    }

    #[test]
    fn test_variable_display_line_with_type() {
        let var = Variable::new("x", "42").with_type("i32");
        assert_eq!(var.display_line(), "  x: i32 = 42");
    }

    #[test]
    fn test_variable_display_line_expandable() {
        let var = Variable::new("vec", "[...]").with_reference(5);
        assert_eq!(var.display_line(), "> vec = [...]");

        let expanded = Variable {
            expanded: true,
            ..Variable::new("vec", "[...]").with_reference(5)
        };
        assert_eq!(expanded.display_line(), "v vec = [...]");
    }

    #[test]
    fn test_variable_display_line_nested() {
        let var = Variable::new("field", "hello").with_depth(2);
        assert_eq!(var.display_line(), "      field = hello");
    }
}
