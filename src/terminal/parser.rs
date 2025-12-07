//! ANSI/VTE escape sequence parser.
//!
//! Uses the `vte` crate to parse terminal escape sequences.

use super::action::ParsedAction;
use super::style::{Attr, Color};

/// Maximum parameters for CSI sequences.
const MAX_PARAMS: usize = 16;

/// ANSI parser using vte.
pub struct AnsiParser {
    /// VTE parser state machine.
    parser: vte::Parser,
    /// Collected actions.
    actions: Vec<ParsedAction>,
    /// Current text buffer.
    text_buffer: String,
    /// CSI parameters.
    csi_params: Vec<u16>,
    /// CSI intermediate bytes.
    csi_intermediates: Vec<u8>,
}

impl AnsiParser {
    /// Creates a new parser.
    #[must_use]
    pub fn new() -> Self {
        Self {
            parser: vte::Parser::new(),
            actions: Vec::new(),
            text_buffer: String::new(),
            csi_params: Vec::with_capacity(MAX_PARAMS),
            csi_intermediates: Vec::with_capacity(4),
        }
    }

    /// Parses input bytes and returns actions.
    pub fn parse(&mut self, input: &[u8]) -> Vec<ParsedAction> {
        self.actions.clear();

        let mut performer = ParserPerformer {
            actions: &mut self.actions,
            text_buffer: &mut self.text_buffer,
            csi_params: &mut self.csi_params,
            csi_intermediates: &mut self.csi_intermediates,
        };

        // VTE 0.15 takes a slice directly
        self.parser.advance(&mut performer, input);

        performer.flush_text();

        std::mem::take(&mut self.actions)
    }
}

impl Default for AnsiParser {
    fn default() -> Self {
        Self::new()
    }
}

/// VTE performer that converts events to `ParsedAction`.
struct ParserPerformer<'a> {
    actions: &'a mut Vec<ParsedAction>,
    text_buffer: &'a mut String,
    csi_params: &'a mut Vec<u16>,
    csi_intermediates: &'a mut Vec<u8>,
}

impl<'a> ParserPerformer<'a> {
    /// Flushes accumulated text to actions.
    fn flush_text(&mut self) {
        if !self.text_buffer.is_empty() {
            let text = std::mem::take(self.text_buffer);
            self.actions.push(ParsedAction::Print(text));
        }
    }

    /// Gets parameter with default value.
    fn param(&self, index: usize, default: u16) -> u16 {
        self.csi_params
            .get(index)
            .copied()
            .unwrap_or(default)
            .max(1)
    }

    /// Handles SGR (Select Graphic Rendition).
    fn handle_sgr(&mut self) {
        if self.csi_params.is_empty() {
            self.actions.push(ParsedAction::SetAttr(Vec::new()));
            return;
        }

        let mut i = 0;
        while i < self.csi_params.len() {
            let param = self.csi_params[i];
            self.process_sgr_param(param, &mut i);
            i += 1;
        }
    }

    /// Processes a single SGR parameter.
    fn process_sgr_param(&mut self, param: u16, index: &mut usize) {
        match param {
            0 => self.actions.push(ParsedAction::SetAttr(Vec::new())),
            1 => self.actions.push(ParsedAction::SetAttr(vec![Attr::Bold])),
            2 => self.actions.push(ParsedAction::SetAttr(vec![Attr::Dim])),
            3 => self.actions.push(ParsedAction::SetAttr(vec![Attr::Italic])),
            4 => self
                .actions
                .push(ParsedAction::SetAttr(vec![Attr::Underline])),
            5 => self.actions.push(ParsedAction::SetAttr(vec![Attr::Blink])),
            7 => self
                .actions
                .push(ParsedAction::SetAttr(vec![Attr::Reverse])),
            8 => self.actions.push(ParsedAction::SetAttr(vec![Attr::Hidden])),
            9 => self
                .actions
                .push(ParsedAction::SetAttr(vec![Attr::Strikethrough])),
            30..=37 => {
                let color = Color::from_standard((param - 30) as u8);
                self.actions.push(ParsedAction::SetFg(color));
            }
            38 => {
                if let Some(color) = self.parse_extended_color(index) {
                    self.actions.push(ParsedAction::SetFg(color));
                }
            }
            39 => self.actions.push(ParsedAction::SetFg(Color::Default)),
            40..=47 => {
                let color = Color::from_standard((param - 40) as u8);
                self.actions.push(ParsedAction::SetBg(color));
            }
            48 => {
                if let Some(color) = self.parse_extended_color(index) {
                    self.actions.push(ParsedAction::SetBg(color));
                }
            }
            49 => self.actions.push(ParsedAction::SetBg(Color::Default)),
            90..=97 => {
                let color = Color::from_bright((param - 90 + 8) as u8);
                self.actions.push(ParsedAction::SetFg(color));
            }
            100..=107 => {
                let color = Color::from_bright((param - 100 + 8) as u8);
                self.actions.push(ParsedAction::SetBg(color));
            }
            _ => {}
        }
    }

    /// Parses extended color (256-color or RGB).
    fn parse_extended_color(&self, index: &mut usize) -> Option<Color> {
        if *index + 1 >= self.csi_params.len() {
            return None;
        }

        let mode = self.csi_params[*index + 1];
        match mode {
            5 => {
                if *index + 2 < self.csi_params.len() {
                    let color_index = self.csi_params[*index + 2] as u8;
                    *index += 2;
                    return Some(Color::Indexed(color_index));
                }
            }
            2 => {
                if *index + 4 < self.csi_params.len() {
                    let r = self.csi_params[*index + 2] as u8;
                    let g = self.csi_params[*index + 3] as u8;
                    let b = self.csi_params[*index + 4] as u8;
                    *index += 4;
                    return Some(Color::Rgb(r, g, b));
                }
            }
            _ => {}
        }
        None
    }

    /// Handles CSI sequence.
    fn handle_csi(&mut self, action: char) {
        let has_question = self.csi_intermediates.contains(&b'?');
        let has_space = self.csi_intermediates.contains(&b' ');

        match action {
            'A' => self.actions.push(ParsedAction::CursorUp(self.param(0, 1))),
            'B' => self
                .actions
                .push(ParsedAction::CursorDown(self.param(0, 1))),
            'C' => self
                .actions
                .push(ParsedAction::CursorForward(self.param(0, 1))),
            'D' => self
                .actions
                .push(ParsedAction::CursorBack(self.param(0, 1))),
            'H' | 'f' => {
                let row = self.param(0, 1);
                let col = if self.csi_params.len() > 1 {
                    self.param(1, 1)
                } else {
                    1
                };
                self.actions.push(ParsedAction::CursorPosition(row, col));
            }
            'J' => {
                let mode = self.csi_params.first().copied().unwrap_or(0) as u8;
                self.actions.push(ParsedAction::EraseDisplay(mode));
            }
            'K' => {
                let mode = self.csi_params.first().copied().unwrap_or(0) as u8;
                self.actions.push(ParsedAction::EraseLine(mode));
            }
            'S' => self.actions.push(ParsedAction::ScrollUp(self.param(0, 1))),
            'T' => self
                .actions
                .push(ParsedAction::ScrollDown(self.param(0, 1))),
            'm' => self.handle_sgr(),
            's' => self.actions.push(ParsedAction::SaveCursor),
            'u' => self.actions.push(ParsedAction::RestoreCursor),
            'h' => self.handle_csi_h(has_question),
            'l' => self.handle_csi_l(has_question),
            'L' => self
                .actions
                .push(ParsedAction::InsertLines(self.param(0, 1))),
            'M' => self
                .actions
                .push(ParsedAction::DeleteLines(self.param(0, 1))),
            '@' => self
                .actions
                .push(ParsedAction::InsertChars(self.param(0, 1))),
            'P' => self
                .actions
                .push(ParsedAction::DeleteChars(self.param(0, 1))),
            'n' => {
                if self.param(0, 0) == 6 {
                    self.actions.push(ParsedAction::DeviceStatusReport);
                }
            }
            'q' => {
                if has_space {
                    let shape = self.csi_params.first().copied().unwrap_or(0) as u8;
                    self.actions.push(ParsedAction::SetCursorShape(shape));
                }
            }
            _ => {
                let desc = format!(
                    "CSI {:?} {:?} {}",
                    self.csi_params, self.csi_intermediates, action
                );
                self.actions.push(ParsedAction::Unknown(desc));
            }
        }
    }

    /// Handles CSI h sequence.
    fn handle_csi_h(&mut self, has_question: bool) {
        if has_question {
            for param in &*self.csi_params {
                match *param {
                    25 => self.actions.push(ParsedAction::ShowCursor),
                    1049 => self.actions.push(ParsedAction::EnterAlternateScreen),
                    _ => {}
                }
            }
        }
    }

    /// Handles CSI l sequence.
    fn handle_csi_l(&mut self, has_question: bool) {
        if has_question {
            for param in &*self.csi_params {
                match *param {
                    25 => self.actions.push(ParsedAction::HideCursor),
                    1049 => self.actions.push(ParsedAction::ExitAlternateScreen),
                    _ => {}
                }
            }
        }
    }

    /// Handles OSC sequence.
    fn handle_osc(&mut self, data: &[&[u8]]) {
        if data.is_empty() {
            return;
        }

        let cmd = String::from_utf8_lossy(data[0]);

        match cmd.as_ref() {
            "0" | "2" => {
                if data.len() > 1 {
                    let title = String::from_utf8_lossy(data[1]).to_string();
                    self.actions.push(ParsedAction::SetTitle(title));
                }
            }
            "7" => {
                // OSC 7: Set current working directory
                // Format: OSC 7 ; file://<host>/<path> ST
                if data.len() > 1 {
                    let uri = String::from_utf8_lossy(data[1]).to_string();
                    // Parse file:// URI to get the path
                    if let Some(path) = parse_file_uri(&uri) {
                        self.actions.push(ParsedAction::SetCwd(path));
                    }
                }
            }
            "8" => {
                if data.len() >= 2 {
                    let params = String::from_utf8_lossy(data[1]);
                    let url = if data.len() > 2 {
                        String::from_utf8_lossy(data[2]).to_string()
                    } else {
                        String::new()
                    };

                    let id = params
                        .split(';')
                        .find(|p| p.starts_with("id="))
                        .map(|p| p.trim_start_matches("id=").to_string());

                    self.actions.push(ParsedAction::Hyperlink { url, id });
                }
            }
            _ => {}
        }
    }
}

/// Parses a file:// URI to extract the path.
fn parse_file_uri(uri: &str) -> Option<String> {
    let uri = uri.trim();

    // Handle file:// URI
    if let Some(rest) = uri.strip_prefix("file://") {
        // Skip the host part (usually localhost or empty)
        // Format: file://[host]/path or file:///path
        let path = if rest.starts_with('/') {
            // file:///path (no host)
            rest.to_string()
        } else if let Some(slash_pos) = rest.find('/') {
            // file://host/path
            rest[slash_pos..].to_string()
        } else {
            return None;
        };

        // On Windows, paths might be like /C:/Users/...
        #[cfg(windows)]
        {
            let path = path.trim_start_matches('/');
            // URL decode the path
            Some(url_decode(path))
        }

        #[cfg(not(windows))]
        {
            Some(url_decode(&path))
        }
    } else {
        // Not a file:// URI, might be a plain path
        Some(uri.to_string())
    }
}

/// Simple URL decoding for file paths.
fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            // Try to read two hex digits
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                    continue;
                }
            }
            // Invalid encoding, keep as-is
            result.push('%');
            result.push_str(&hex);
        } else {
            result.push(c);
        }
    }

    result
}

impl<'a> vte::Perform for ParserPerformer<'a> {
    fn print(&mut self, c: char) {
        self.text_buffer.push(c);
    }

    fn execute(&mut self, byte: u8) {
        self.flush_text();

        match byte {
            0x07 => self.actions.push(ParsedAction::Bell),
            0x08 => self.actions.push(ParsedAction::Backspace),
            0x09 => self.actions.push(ParsedAction::Tab),
            0x0A | 0x0B | 0x0C => self.actions.push(ParsedAction::LineFeed),
            0x0D => self.actions.push(ParsedAction::CarriageReturn),
            _ => {}
        }
    }

    fn hook(&mut self, _params: &vte::Params, _intermediates: &[u8], _ignore: bool, _action: char) {
    }

    fn put(&mut self, _byte: u8) {}

    fn unhook(&mut self) {}

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        self.flush_text();
        self.handle_osc(params);
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        self.flush_text();

        self.csi_params.clear();
        for param in params.iter() {
            if let Some(&value) = param.first() {
                self.csi_params.push(value);
            }
        }

        self.csi_intermediates.clear();
        self.csi_intermediates.extend_from_slice(intermediates);

        self.handle_csi(action);
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        self.flush_text();

        match (intermediates, byte) {
            ([], b'7') => self.actions.push(ParsedAction::SaveCursor),
            ([], b'8') => self.actions.push(ParsedAction::RestoreCursor),
            ([], b'D') => self.actions.push(ParsedAction::LineFeed),
            ([], b'E') => {
                self.actions.push(ParsedAction::LineFeed);
                self.actions.push(ParsedAction::CarriageReturn);
            }
            ([], b'M') => self.actions.push(ParsedAction::ScrollDown(1)),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_plain_text() {
        let mut parser = AnsiParser::new();
        let actions = parser.parse(b"Hello");
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], ParsedAction::Print(s) if s == "Hello"));
    }

    #[test]
    fn test_parser_cursor_up() {
        let mut parser = AnsiParser::new();
        let actions = parser.parse(b"\x1b[5A");
        assert!(matches!(actions.as_slice(), [ParsedAction::CursorUp(5)]));
    }

    #[test]
    fn test_parser_sgr_reset() {
        let mut parser = AnsiParser::new();
        let actions = parser.parse(b"\x1b[0m");
        assert!(matches!(
            actions.as_slice(),
            [ParsedAction::SetAttr(attrs)] if attrs.is_empty()
        ));
    }

    #[test]
    fn test_parser_sgr_bold() {
        let mut parser = AnsiParser::new();
        let actions = parser.parse(b"\x1b[1m");
        assert!(matches!(
            actions.as_slice(),
            [ParsedAction::SetAttr(attrs)] if attrs.contains(&Attr::Bold)
        ));
    }

    #[test]
    fn test_parser_fg_color() {
        let mut parser = AnsiParser::new();
        let actions = parser.parse(b"\x1b[31m");
        assert!(matches!(
            actions.as_slice(),
            [ParsedAction::SetFg(Color::Red)]
        ));
    }
}
