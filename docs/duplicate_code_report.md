# Duplicate Code Analysis Report

This document identifies duplicate code patterns across the Ratterm codebase, categorized by severity and providing refactoring recommendations.

---

## Summary Statistics

| Metric | Count |
|--------|-------|
| Major Duplicate Patterns | 16 |
| High Severity (Exact Duplicates) | 6 |
| Medium Severity (Similar Patterns) | 7 |
| Low Severity (Acceptable Patterns) | 3 |
| Primary Files Affected | 14 |
| Most Duplicated Pattern | Vim navigation keys (5+ occurrences) |

---

## High Severity Duplicates

### 1. Vim Navigation Keys (j/k + Arrow Keys)

**Severity:** HIGH - Exact code duplication across 5+ files

**Files Affected:**
- `src/app/input_ssh.rs:39-43`
- `src/app/input_docker.rs:100-108`
- `src/app/input_health.rs:38-48`
- `src/app/input.rs:529-540` (mode_switcher_key)
- `src/app/input.rs:554-558` (shell_selector_key)
- `src/app/input.rs:577-581` (theme_selector_key)

**Duplicated Pattern:**
```rust
// Pattern 1: Navigation with j/k aliases
(KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
    manager.select_next();
}
(KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
    manager.select_prev();
}
```

**Refactoring Recommendation:**
Create a macro or helper function:
```rust
macro_rules! vim_navigation {
    ($self:expr, $manager:expr, $key:expr) => {
        match ($key.modifiers, $key.code) {
            (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
                $manager.select_next();
                true
            }
            (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
                $manager.select_prev();
                true
            }
            _ => false,
        }
    };
}
```

---

### 2. Terminal Selection with Shift+Arrows (4 Identical Nested Blocks)

**Severity:** HIGH - Exact duplication within same file

**File:** `src/app/input_terminal.rs:74-105`

**Duplicated Pattern:**
```rust
// Lines 74-80
(KeyModifiers::SHIFT, KeyCode::Left) => {
    if let Some(ref mut terminals) = self.terminals {
        if let Some(terminal) = terminals.active_terminal_mut() {
            terminal.select_left();
        }
    }
    return;
}
// REPEATED for Right (82-88), Up (90-96), Down (98-104)
```

**Refactoring Recommendation:**
Extract helper method:
```rust
fn with_active_terminal<F>(&mut self, f: F)
where F: FnOnce(&mut Terminal)
{
    if let Some(ref mut terminals) = self.terminals {
        if let Some(terminal) = terminals.active_terminal_mut() {
            f(terminal);
        }
    }
}

// Usage:
(KeyModifiers::SHIFT, KeyCode::Left) => {
    self.with_active_terminal(|t| t.select_left());
    return;
}
```

---

### 3. Manager Null Check Pattern (34+ Occurrences)

**Severity:** HIGH - Excessive defensive pattern repetition

**Files Affected:**
- `src/app/input_docker.rs` (15+ occurrences)
- `src/app/input_docker_create.rs` (12+ occurrences)
- `src/app/input_ssh.rs` (6+ occurrences)
- `src/app/input_health.rs` (5+ occurrences)

**Duplicated Pattern:**
```rust
// Occurs 34+ times across input handler files
if let Some(ref mut manager) = self.docker_manager {
    manager.method();
}
```

**Refactoring Recommendation:**
Create wrapper methods that encapsulate the pattern:
```rust
impl App {
    fn with_docker_manager<F, R>(&mut self, f: F) -> Option<R>
    where F: FnOnce(&mut DockerManagerSelector) -> R
    {
        self.docker_manager.as_mut().map(f)
    }
}

// Usage:
self.with_docker_manager(|m| m.select_next());
```

---

### 4. Form Field Navigation (Tab/Shift+Tab Pattern)

**Severity:** HIGH - Exact duplication across 4 input handlers

**Files Affected:**
- `src/app/input_ssh.rs:82-85` (credential entry)
- `src/app/input_ssh.rs:108-111` (add host)
- `src/app/input_docker.rs:228-237` (run options)
- `src/app/input_docker.rs:505-522` (host credentials)

**Duplicated Pattern:**
```rust
(KeyModifiers::NONE, KeyCode::Tab) => {
    manager.next_field();
}
(KeyModifiers::SHIFT, KeyCode::Tab) | (KeyModifiers::SHIFT, KeyCode::BackTab) => {
    manager.prev_field();
}
```

**Refactoring Recommendation:**
Create a trait and helper:
```rust
trait FormNavigation {
    fn next_field(&mut self);
    fn prev_field(&mut self);
}

fn handle_form_tab_navigation<T: FormNavigation>(key: KeyEvent, form: &mut T) -> bool {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Tab) => { form.next_field(); true }
        (KeyModifiers::SHIFT, KeyCode::Tab | KeyCode::BackTab) => { form.prev_field(); true }
        _ => false,
    }
}
```

---

### 5. Text Input Handling (Backspace + Char Insert)

**Severity:** HIGH - Identical pattern in 8+ locations

**Files Affected:**
- `src/app/input_ssh.rs:87-90, 113, 141-143, 158-160, 191-193, 208-210`
- `src/app/input_docker.rs:240-248, 530-548`
- `src/app/input_docker_create.rs:71-79, 227-235, 261-269, 312-320`

**Duplicated Pattern:**
```rust
(KeyModifiers::NONE, KeyCode::Backspace) => manager.backspace(),
(KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
    manager.insert_char(c);
}
```

**Refactoring Recommendation:**
Create a text input trait and handler:
```rust
trait TextInput {
    fn backspace(&mut self);
    fn insert_char(&mut self, c: char);
}

fn handle_text_input<T: TextInput>(key: KeyEvent, input: &mut T) -> bool {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Backspace) => { input.backspace(); true }
        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
            input.insert_char(c);
            true
        }
        _ => false,
    }
}
```

---

### 6. Home/End Selection Pattern

**Severity:** HIGH - Exact duplication across 4 managers

**Files Affected:**
- `src/app/input_ssh.rs:45-46`
- `src/app/input_health.rs:50-60`
- `src/app/input_docker.rs:110-118`
- `src/app/input.rs:514-515` (popup)

**Duplicated Pattern:**
```rust
(KeyModifiers::NONE, KeyCode::Home) => manager.select_first(),
(KeyModifiers::NONE, KeyCode::End) => manager.select_last(),
```

---

## Medium Severity Duplicates

### 7. Popup Text Input Handler (Identical in 3 Popup Types)

**Severity:** MEDIUM - Behavioral duplication

**Files Affected:**
- `src/app/input_ssh.rs:129-147` (SSH subnet key)
- `src/app/input_ssh.rs:149-164` (master password key)
- `src/app/input.rs:499-527` (popup key handler)

**Duplicated Pattern:**
```rust
(KeyModifiers::NONE, KeyCode::Esc) => self.hide_popup(),
(KeyModifiers::NONE, KeyCode::Enter) => {
    let input = self.popup.input().to_string();
    self.hide_popup();
    // Process input
}
(KeyModifiers::NONE, KeyCode::Backspace) => self.popup.backspace(),
(KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
    self.popup.insert_char(c);
}
```

---

### 8. Arrow Key Navigation Across Editor Modes

**Severity:** MEDIUM - Structural duplication with slight variations

**File:** `src/app/input_editor.rs`

**Locations:**
- Lines 58-61 (Emacs mode)
- Lines 82-96 (Default mode with completion dismissal)
- Lines 146-157 (Vim normal mode with h/j/k/l)
- Lines 184-187 (Vim insert mode)

**Duplicated Pattern:**
```rust
// Emacs (lines 58-61):
(KeyModifiers::NONE, KeyCode::Left) => self.editor.move_left(),
(KeyModifiers::NONE, KeyCode::Right) => self.editor.move_right(),
(KeyModifiers::NONE, KeyCode::Up) => self.editor.move_up(),
(KeyModifiers::NONE, KeyCode::Down) => self.editor.move_down(),

// Default (lines 82-96) - adds completion dismissal:
(KeyModifiers::NONE, KeyCode::Left) => {
    self.dismiss_completion();
    self.editor.move_left();
}
```

---

### 9. Escape Key Cancel Pattern

**Severity:** MEDIUM - Common but contextually appropriate

**Files Affected:**
- `src/app/input_ssh.rs:38, 81, 107, 131, 152, 185, 206`
- `src/app/input_docker.rs:65, 95, 216, 259, 440, 498`
- `src/app/input_docker_create.rs:63, 126, 195, 207, 250, 278, 301, 329`
- `src/app/input_health.rs:84`

**Duplicated Pattern:**
```rust
(KeyModifiers::NONE, KeyCode::Esc) => {
    self.hide_popup();  // or manager.cancel_*()
}
```

**Note:** This is acceptable as the cancel action varies by context.

---

### 10. Docker Manager Mode Dispatch

**Severity:** MEDIUM - Large switch statement

**File:** `src/app/input_docker.rs:18-51`

**Pattern:**
```rust
match manager.mode() {
    DockerManagerMode::List | DockerManagerMode::Discovering => {
        self.handle_docker_list_key(key);
    }
    DockerManagerMode::RunOptions => {
        self.handle_docker_run_options_key(key);
    }
    // ... 10+ more modes
}
```

**Similar in:** `src/app/input_docker_create.rs:25-57`

---

### 11. Completion Dismissal + Movement Pattern

**Severity:** MEDIUM - 7 occurrences in same file

**File:** `src/app/input_editor.rs:82-96, 109-123`

**Duplicated Pattern:**
```rust
(KeyModifiers::NONE, KeyCode::Left) => {
    self.dismiss_completion();
    self.editor.move_left();
}
(KeyModifiers::NONE, KeyCode::Right) => {
    self.dismiss_completion();
    self.editor.move_right();
}
// Similar for Up, Down
```

**Refactoring Recommendation:**
```rust
fn editor_move_with_dismiss(&mut self, direction: Direction) {
    self.dismiss_completion();
    match direction {
        Direction::Left => self.editor.move_left(),
        Direction::Right => self.editor.move_right(),
        Direction::Up => self.editor.move_up(),
        Direction::Down => self.editor.move_down(),
    }
}
```

---

### 12. Vim Visual Mode Cursor Movement

**Severity:** MEDIUM - Structural duplication

**File:** `src/app/input_editor.rs:203-215`

**Duplicated Pattern:**
```rust
// Lines 203-208
(KeyModifiers::NONE, KeyCode::Char('h')) | (KeyModifiers::NONE, KeyCode::Left) => {
    let buffer = self.editor.buffer();
    let mut cursor = self.editor.cursor().clone();
    cursor.move_left(buffer);
    cursor.extend_to(cursor.position());
    *self.editor.cursor_mut() = cursor;
}
// Lines 210-215 - identical structure for 'l'/Right
```

---

### 13. Selection Style Pattern (UI Layer)

**Severity:** MEDIUM - Found in UI widget files

**Pattern found in SSH and Docker manager widgets:**
```rust
let is_selected = index == selected_index;
let style = if is_selected {
    Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD)
} else {
    Style::default()
};
```

---

## Low Severity Duplicates

### 14. Status Message Setting

**Severity:** LOW - Normal API usage

**Occurrences:** 200+ across all `src/app/` files

**Pattern:**
```rust
self.set_status(format!("message: {}", value));
self.set_status("message".to_string());
```

**Note:** This is standard usage but could benefit from helper macros for common message formats.

---

### 15. Ctrl+S Save Pattern

**Severity:** LOW - Intentional consistency

**Files Affected:**
- `src/app/input_editor.rs:53, 108, 171, 191`

**Pattern:**
```rust
(KeyModifiers::CONTROL, KeyCode::Char('s')) => self.save_current_file(),
```

---

### 16. PageUp/PageDown Pattern

**Severity:** LOW - Simple consistent usage

**Files Affected:**
- `src/app/input_editor.rs:64-65, 100-101, 166-167`
- `src/app/input.rs:325-326, 382-390`

**Pattern:**
```rust
(KeyModifiers::NONE, KeyCode::PageUp) => self.editor.page_up(),
(KeyModifiers::NONE, KeyCode::PageDown) => self.editor.page_down(),
```

---

## Refactoring Priority List

### Priority 1 - High Impact (Implement First)

| Pattern | Files | Est. LOC Saved |
|---------|-------|----------------|
| Manager null check wrapper | 4 | ~70 |
| Vim navigation macro | 6 | ~50 |
| Text input trait | 8 | ~60 |
| Terminal selection helper | 1 | ~25 |

### Priority 2 - Medium Impact

| Pattern | Files | Est. LOC Saved |
|---------|-------|----------------|
| Form tab navigation | 4 | ~30 |
| Popup input handler | 3 | ~25 |
| Editor movement helper | 1 | ~20 |

### Priority 3 - Low Impact (Optional)

| Pattern | Files | Est. LOC Saved |
|---------|-------|----------------|
| Home/End selection | 4 | ~15 |
| Status message macros | All | ~20 |

---

## Recommended Implementation Approach

### Step 1: Create Common Input Traits

Create a new file `src/app/input_traits.rs`:

```rust
//! Common input handling traits and helpers.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Trait for types that support text input.
pub trait TextInput {
    fn backspace(&mut self);
    fn insert_char(&mut self, c: char);
}

/// Trait for types that support list selection.
pub trait ListSelection {
    fn select_next(&mut self);
    fn select_prev(&mut self);
    fn select_first(&mut self);
    fn select_last(&mut self);
}

/// Trait for types that support form field navigation.
pub trait FormNavigation {
    fn next_field(&mut self);
    fn prev_field(&mut self);
}

/// Handles Vim-style navigation (j/k + arrows).
/// Returns true if the key was handled.
pub fn handle_vim_navigation<T: ListSelection>(key: KeyEvent, list: &mut T) -> bool {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            list.select_next();
            true
        }
        (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            list.select_prev();
            true
        }
        (KeyModifiers::NONE, KeyCode::Home) => {
            list.select_first();
            true
        }
        (KeyModifiers::NONE, KeyCode::End) => {
            list.select_last();
            true
        }
        _ => false,
    }
}

/// Handles text input (backspace + char insert).
/// Returns true if the key was handled.
pub fn handle_text_input<T: TextInput>(key: KeyEvent, input: &mut T) -> bool {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Backspace) => {
            input.backspace();
            true
        }
        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
            input.insert_char(c);
            true
        }
        _ => false,
    }
}

/// Handles form tab navigation.
/// Returns true if the key was handled.
pub fn handle_form_navigation<T: FormNavigation>(key: KeyEvent, form: &mut T) -> bool {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Tab) => {
            form.next_field();
            true
        }
        (KeyModifiers::SHIFT, KeyCode::Tab | KeyCode::BackTab) => {
            form.prev_field();
            true
        }
        _ => false,
    }
}
```

### Step 2: Implement Traits for Managers

Have `SSHManagerSelector`, `DockerManagerSelector`, and `HealthDashboard` implement the appropriate traits.

### Step 3: Update Input Handlers

Replace duplicated code with trait-based helpers.

---

## Maintenance Notes

- This report was generated based on code analysis as of the current date
- Run periodic duplicate detection during code review
- Consider using `clippy::cognitive_complexity` to identify functions that need refactoring
- Update this document when significant refactoring is completed
