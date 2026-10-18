//! Tracks the subset of terminal state that persists across a cut in the output stream,
//! so that a pruned scrollback can be replayed with a short prelude that restores it.

use std::{collections::BTreeMap, fmt};
use vte::{Params, Parser, Perform};

/// DEC private modes that are replayed in the prelude (alt-screen modes are handled separately).
const TRACKED_MODES: &[u16] = &[
    1, 6, 7, 12, 25, 1000, 1002, 1003, 1004, 1005, 1006, 1015, 1016, 2004,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Color {
    #[default]
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

/// Select Graphic Rendition attributes
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Sgr {
    bold: bool,
    faint: bool,
    italic: bool,
    underline: bool,
    blink: bool,
    inverse: bool,
    hidden: bool,
    strike: bool,
    fg: Color,
    bg: Color,
}

impl Sgr {
    fn apply(&mut self, params: &Params) {
        let groups: Vec<&[u16]> = params.iter().collect();
        if groups.is_empty() {
            *self = Self::default();
            return;
        }

        let mut i = 0;
        while i < groups.len() {
            let group = groups[i];
            i += 1;

            match group[0] {
                0 => *self = Self::default(),
                1 => self.bold = true,
                2 => self.faint = true,
                3 => self.italic = true,
                4 => self.underline = group.get(1) != Some(&0),
                21 => self.underline = true,
                5 | 6 => self.blink = true,
                7 => self.inverse = true,
                8 => self.hidden = true,
                9 => self.strike = true,
                22 => {
                    self.bold = false;
                    self.faint = false;
                }
                23 => self.italic = false,
                24 => self.underline = false,
                25 => self.blink = false,
                27 => self.inverse = false,
                28 => self.hidden = false,
                29 => self.strike = false,
                n @ 30..=37 => self.fg = Color::Indexed((n - 30) as u8),
                38 => {
                    if let Some(color) = extended_color(group, &groups, &mut i) {
                        self.fg = color;
                    }
                }
                39 => self.fg = Color::Default,
                n @ 40..=47 => self.bg = Color::Indexed((n - 40) as u8),
                48 => {
                    if let Some(color) = extended_color(group, &groups, &mut i) {
                        self.bg = color;
                    }
                }
                49 => self.bg = Color::Default,
                n @ 90..=97 => self.fg = Color::Indexed((n - 90 + 8) as u8),
                n @ 100..=107 => self.bg = Color::Indexed((n - 100 + 8) as u8),
                _ => {}
            }
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        if *self == Self::default() {
            return;
        }

        let mut codes = vec!["0".to_string()];
        let flags = [
            (self.bold, "1"),
            (self.faint, "2"),
            (self.italic, "3"),
            (self.underline, "4"),
            (self.blink, "5"),
            (self.inverse, "7"),
            (self.hidden, "8"),
            (self.strike, "9"),
        ];
        codes.extend(
            flags
                .iter()
                .filter(|(on, _)| *on)
                .map(|(_, c)| c.to_string()),
        );

        for (base, color) in [(38, self.fg), (48, self.bg)].iter() {
            match color {
                Color::Default => {}
                Color::Indexed(n) => codes.push(format!("{};5;{}", base, n)),
                Color::Rgb(r, g, b) => codes.push(format!("{};2;{};{};{}", base, r, g, b)),
            }
        }

        out.extend(format!("\x1b[{}m", codes.join(";")).bytes());
    }
}

/// Parses the arguments of SGR 38/48, in both the `38;5;n` and `38:5:n` forms.
/// `next` is advanced past any semicolon-separated groups that were consumed.
fn extended_color(group: &[u16], groups: &[&[u16]], next: &mut usize) -> Option<Color> {
    if group.len() > 1 {
        return match group[1] {
            5 => Some(Color::Indexed(*group.get(2)? as u8)),
            2 => {
                let rgb = match group.len() {
                    6.. => &group[3..6],
                    5 => &group[2..5],
                    _ => return None,
                };
                Some(Color::Rgb(rgb[0] as u8, rgb[1] as u8, rgb[2] as u8))
            }
            _ => None,
        };
    }

    let mut take = || {
        let value = groups.get(*next)?.first().copied()?;
        *next += 1;
        Some(value)
    };

    match take()? {
        5 => Some(Color::Indexed(take()? as u8)),
        2 => {
            let (r, g, b) = (take()?, take()?, take()?);
            Some(Color::Rgb(r as u8, g as u8, b as u8))
        }
        _ => None,
    }
}

/// Terminal state that must be restored when replay begins mid-stream.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalState {
    alt_screen: bool,
    sgr: Sgr,
    modes: BTreeMap<u16, bool>,
    scroll_region: Option<(u16, u16)>,
    keypad_application: bool,
    g0_graphics: bool,
    g1_graphics: bool,
    shift_out: bool,
    cursor_style: Option<u16>,
    title: Option<Vec<u8>>,
}

impl TerminalState {
    #[cfg(test)]
    pub fn alt_screen(&self) -> bool {
        self.alt_screen
    }

    fn csi(&mut self, params: &Params, intermediates: &[u8], action: char) {
        match (intermediates, action) {
            ([], 'm') => self.sgr.apply(params),
            ([b'?'], 'h') => self.set_modes(params, true),
            ([b'?'], 'l') => self.set_modes(params, false),
            ([], 'r') => {
                let mut values = params.iter().map(|p| p[0]);
                let top = values.next().unwrap_or(0);
                let bottom = values.next().unwrap_or(0);
                self.scroll_region = if top == 0 && bottom == 0 {
                    None
                } else {
                    Some((top, bottom))
                };
            }
            ([b' '], 'q') => {
                let style = params.iter().next().map(|p| p[0]).unwrap_or(0);
                self.cursor_style = if style == 0 { None } else { Some(style) };
            }
            ([b'!'], 'p') => {
                let title = self.title.take();
                *self = Self {
                    alt_screen: self.alt_screen,
                    title,
                    ..Self::default()
                };
            }
            _ => {}
        }
    }

    fn set_modes(&mut self, params: &Params, on: bool) {
        for param in params.iter() {
            match param[0] {
                47 | 1047 | 1049 => self.alt_screen = on,
                mode if TRACKED_MODES.contains(&mode) => {
                    self.modes.insert(mode, on);
                }
                _ => {}
            }
        }
    }

    fn esc(&mut self, intermediates: &[u8], byte: u8) {
        match (intermediates, byte) {
            ([], b'c') => *self = Self::default(),
            ([], b'=') => self.keypad_application = true,
            ([], b'>') => self.keypad_application = false,
            ([b'('], charset) => self.g0_graphics = charset == b'0',
            ([b')'], charset) => self.g1_graphics = charset == b'0',
            _ => {}
        }
    }

    fn osc(&mut self, params: &[&[u8]]) {
        if let [kind, rest @ ..] = params {
            if matches!(*kind, b"0" | b"2") && !rest.is_empty() {
                self.title = Some(rest.join(&b';'));
            }
        }
    }

    /// Escape sequences that recreate this state on a terminal in its default state.
    /// Empty if there is nothing to restore.
    pub fn prelude(&self) -> Vec<u8> {
        let mut out = Vec::new();

        if self.alt_screen {
            out.extend_from_slice(b"\x1b[?1049h");
        }

        if let Some((top, bottom)) = self.scroll_region {
            out.extend(format!("\x1b[{};{}r", top, bottom).bytes());
        }

        self.sgr.write(&mut out);

        for (mode, on) in &self.modes {
            let suffix = if *on { 'h' } else { 'l' };
            out.extend(format!("\x1b[?{}{}", mode, suffix).bytes());
        }

        if self.keypad_application {
            out.extend_from_slice(b"\x1b=");
        }

        if self.g0_graphics {
            out.extend_from_slice(b"\x1b(0");
        }

        if self.g1_graphics {
            out.extend_from_slice(b"\x1b)0");
        }

        if self.shift_out {
            out.push(0x0e);
        }

        if let Some(style) = self.cursor_style {
            out.extend(format!("\x1b[{} q", style).bytes());
        }

        if let Some(title) = &self.title {
            out.extend_from_slice(b"\x1b]2;");
            out.extend_from_slice(title);
            out.push(0x07);
        }

        out
    }
}

/// Feeds bytes through a vte parser, maintaining a `TerminalState` and recording cut points.
pub struct Tracker {
    parser: Parser,
    machine: Machine,
}

struct Machine {
    state: TerminalState,
    /// Absolute stream index just past the byte currently being processed
    pos: usize,
    lines: Vec<usize>,
    hard_cut: Option<usize>,
    alt_since: Option<usize>,
}

impl fmt::Debug for Tracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tracker")
            .field("state", &self.machine.state)
            .field("alt_since", &self.machine.alt_since)
            .finish()
    }
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            parser: Parser::new(),
            machine: Machine {
                state: TerminalState::default(),
                pos: 0,
                lines: Vec::new(),
                hard_cut: None,
                alt_since: None,
            },
        }
    }

    /// Processes `data`, which begins at absolute stream index `start`.
    pub fn feed(&mut self, start: usize, data: &[u8]) {
        for (offset, byte) in data.iter().enumerate() {
            self.machine.pos = start + offset + 1;
            self.parser
                .advance(&mut self.machine, std::slice::from_ref(byte));
        }
    }

    pub fn state(&self) -> &TerminalState {
        &self.machine.state
    }

    /// The stream index at which the active alternate screen was entered
    pub fn alt_since(&self) -> Option<usize> {
        self.machine.alt_since
    }

    /// Stream indices just after each line feed seen since the last call
    pub fn take_lines(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.machine.lines)
    }

    /// The latest index at which the terminal was fully reset or its scrollback cleared
    pub fn take_hard_cut(&mut self) -> Option<usize> {
        self.machine.hard_cut.take()
    }
}

impl Machine {
    fn track_alt_screen(&mut self, was_alt: bool) {
        match (was_alt, self.state.alt_screen) {
            (false, true) => self.alt_since = Some(self.pos),
            (true, false) => self.alt_since = None,
            _ => {}
        }
    }
}

impl Perform for Machine {
    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => self.lines.push(self.pos),
            0x0e => self.state.shift_out = true,
            0x0f => self.state.shift_out = false,
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        self.state.osc(params);
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        let was_alt = self.state.alt_screen;

        // CSI 3 J clears scrollback, but is not meaningful on the alternate screen
        if intermediates.is_empty()
            && action == 'J'
            && !was_alt
            && params.iter().next().map(|p| p[0]) == Some(3)
        {
            self.hard_cut = Some(self.pos);
        }

        self.state.csi(params, intermediates, action);
        self.track_alt_screen(was_alt);
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        let was_alt = self.state.alt_screen;

        if intermediates.is_empty() && byte == b'c' {
            self.hard_cut = Some(self.pos);
        }

        self.state.esc(intermediates, byte);
        self.track_alt_screen(was_alt);
    }
}

#[cfg(test)]
mod tests {
    use super::{TerminalState, Tracker};

    fn track(data: &[u8]) -> Tracker {
        let mut tracker = Tracker::new();
        tracker.feed(0, data);
        tracker
    }

    fn prelude(data: &[u8]) -> String {
        String::from_utf8(track(data).state().prelude()).unwrap()
    }

    #[test]
    fn default_has_empty_prelude() {
        assert!(TerminalState::default().prelude().is_empty());
        assert!(prelude(b"hello\r\nworld\r\n").is_empty());
    }

    #[test]
    fn sgr_attributes() {
        assert_eq!("\x1b[0;1;38;5;1m", prelude(b"\x1b[1;31m"));
        assert_eq!("\x1b[0;4;48;2;1;2;3m", prelude(b"\x1b[4;48;2;1;2;3m"));
        assert_eq!("\x1b[0;38;5;9m", prelude(b"\x1b[91m"));
    }

    #[test]
    fn sgr_colon_form() {
        assert_eq!("\x1b[0;38;2;1;2;3m", prelude(b"\x1b[38:2::1:2:3m"));
        assert_eq!("\x1b[0;38;5;200m", prelude(b"\x1b[38:5:200m"));
    }

    #[test]
    fn sgr_reset() {
        assert_eq!("", prelude(b"\x1b[1;31mtext\x1b[0m"));
        assert_eq!("", prelude(b"\x1b[1;31mtext\x1b[m"));
        assert_eq!("\x1b[0;1m", prelude(b"\x1b[1;3m\x1b[23m"));
    }

    #[test]
    fn modes() {
        assert_eq!("\x1b[?2004h", prelude(b"\x1b[?2004h"));
        assert_eq!("\x1b[?25l", prelude(b"\x1b[?25l"));
        assert_eq!("\x1b[?2004l", prelude(b"\x1b[?2004h\x1b[?2004l"));
    }

    #[test]
    fn scroll_region() {
        assert_eq!("\x1b[2;10r", prelude(b"\x1b[2;10r"));
        assert_eq!("", prelude(b"\x1b[2;10r\x1b[r"));
    }

    #[test]
    fn title() {
        assert_eq!("\x1b]2;a;b\x07", prelude(b"\x1b]0;a;b\x07"));
    }

    #[test]
    fn charset_and_keypad() {
        assert_eq!("\x1b=\x1b(0\x0e", prelude(b"\x1b=\x1b(0\x0e"));
        assert_eq!("", prelude(b"\x1b=\x1b>\x1b(0\x1b(B"));
    }

    #[test]
    fn ris_resets_everything() {
        let mut tracker = track(b"\x1b[1m\x1b[?2004h\x1bc");
        assert!(tracker.state().prelude().is_empty());
        assert_eq!(
            Some(b"\x1b[1m\x1b[?2004h\x1bc".len()),
            tracker.take_hard_cut()
        );
    }

    #[test]
    fn clear_scrollback_is_hard_cut() {
        let mut tracker = track(b"abc\x1b[3J");
        assert_eq!(Some(7), tracker.take_hard_cut());
        assert_eq!(None, tracker.take_hard_cut());
    }

    #[test]
    fn clear_scrollback_in_alt_screen_is_ignored() {
        let mut tracker = track(b"\x1b[?1049h\x1b[3J");
        assert_eq!(None, tracker.take_hard_cut());
    }

    #[test]
    fn alt_screen_tracking() {
        let mut tracker = Tracker::new();
        tracker.feed(0, b"ab\x1b[?1049h");
        assert_eq!(Some(10), tracker.alt_since());
        assert_eq!(
            "\x1b[?1049h",
            String::from_utf8(tracker.state().prelude()).unwrap()
        );

        tracker.feed(10, b"vim\x1b[?1049l");
        assert_eq!(None, tracker.alt_since());
        assert!(!tracker.state().alt_screen());
    }

    #[test]
    fn lines_are_recorded_after_line_feed() {
        let mut tracker = track(b"a\nbc\r\nd");
        assert_eq!(vec![2, 6], tracker.take_lines());
    }

    #[test]
    fn sequences_split_across_feeds() {
        let mut tracker = Tracker::new();
        tracker.feed(0, b"\x1b[?20");
        tracker.feed(5, b"04h");
        assert_eq!(
            "\x1b[?2004h",
            String::from_utf8(tracker.state().prelude()).unwrap()
        );
    }
}
