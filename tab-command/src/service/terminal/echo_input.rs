use thiserror::Error;

use crate::config::{load_global_config, Action};

/// Parses the bindings in the global workspace config file.
///
/// If the config file does not exists, returns the default bindings.
pub fn key_bindings() -> anyhow::Result<KeyBindings> {
    let config = load_global_config().map(|c| c.key_bindings)?;

    if config.is_none() {
        return Ok(KeyBindings::default());
    }

    let bindings = config.unwrap();
    let bindings = {
        let mut parsed_bindings = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let sequence = parse(binding.keys.as_str())?;
            let action = binding.action;
            parsed_bindings.push(KeyBinding { sequence, action });
        }

        KeyBindings {
            bindings: parsed_bindings,
        }
    };

    Ok(bindings)
}

#[derive(Debug, Error, PartialEq)]
pub enum KeyParseError {
    #[error("Invalid `ctrl-X` sequence: {0} - char '{1}' is not an ASCII control character")]
    InvalidCtrlCharacter(String, char),
    #[error("Invalid sequence: '{0}' - use spaces to separate individual characters")]
    InvalidSequence(String),
}

pub fn parse(seq: &str) -> Result<Vec<u8>, KeyParseError> {
    let mut chars = String::new();

    for entry in seq.split(' ') {
        let entry = entry.trim().to_lowercase();

        if entry.trim().len() == 0 {
            continue;
        }

        if entry.len() == 1 {
            let ch = entry.chars().next().unwrap().to_ascii_lowercase();
            chars.push(ch);
            continue;
        }

        if entry.to_ascii_lowercase() == "esc" {
            chars.push('\x1b');
            continue;
        }

        if entry.starts_with("ctrl-") && entry.len() == 6 {
            let ch = entry.chars().nth(5).unwrap().to_ascii_uppercase();
            if ch.is_ascii() && ch >= '@' && ch <= '_' {
                let code = ch as u8 - 64;
                chars.push(code as char);
            } else {
                return Err(KeyParseError::InvalidCtrlCharacter(entry, ch));
            }
        } else {
            return Err(KeyParseError::InvalidSequence(entry));
        }
    }

    Ok(chars.into_bytes())
}

#[derive(Debug, Clone)]
pub struct KeyBindings {
    pub bindings: Vec<KeyBinding>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            bindings: vec![
                KeyBinding {
                    action: Action::Disconnect,
                    sequence: vec![0x14, 0x03],
                },
                KeyBinding {
                    action: Action::SelectInteractive,
                    sequence: vec![0x14],
                },
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeyBinding {
    pub sequence: Vec<u8>,
    pub action: Action,
}

pub struct BindingFilter {
    index: usize,
    sequence: Vec<u8>,
    action: Action,
}

impl BindingFilter {
    pub fn new(binding: KeyBinding) -> Self {
        Self {
            index: 0,
            sequence: binding.sequence,
            action: binding.action,
        }
    }

    pub fn find(&mut self, data: &[u8]) -> Option<usize> {
        for (index, byte) in data.iter().copied().enumerate() {
            let mut expect = self.sequence[self.index];

            if byte != expect {
                self.index = 0;
                expect = self.sequence[0];
            }

            if byte == expect {
                self.index += 1;

                if self.index == self.sequence.len() {
                    // we use saturating sub in case the sequence began in a previous buffer.
                    // we can't undo stdout sent in the past, but we can stop the characters in this buffer from being sent.
                    let result = Some((index + 1).saturating_sub(self.sequence.len()));
                    self.index = 0;
                    return result;
                }
            }
        }

        None
    }
}

pub struct InputFilter {
    bindings: Vec<BindingFilter>,
}

impl From<KeyBindings> for InputFilter {
    fn from(bindings: KeyBindings) -> Self {
        Self::new(bindings.bindings)
    }
}

impl InputFilter {
    pub fn new(binds: Vec<KeyBinding>) -> Self {
        Self {
            bindings: binds.into_iter().map(BindingFilter::new).collect(),
        }
    }

    pub fn input<'a>(&mut self, data: &'a [u8]) -> Input<'a> {
        for binding in self.bindings.iter_mut() {
            if let Some(match_index) = binding.find(data) {
                return Input {
                    action: Some(binding.action),
                    data: &data[0..match_index],
                };
            }
        }

        Input { action: None, data }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input<'a> {
    pub action: Option<Action>,
    pub data: &'a [u8],
}

#[cfg(test)]
mod parse_tests {
    use super::{parse, KeyParseError};

    #[test]
    fn single_char() {
        let parsed = parse("a");
        assert_eq!(Ok(vec![0x61]), parsed);
    }

    #[test]
    fn upper_char() {
        let parsed = parse("A");
        assert_eq!(Ok(vec![0x61]), parsed);
    }

    #[test]
    fn two_chars() {
        let parsed = parse("a b");
        assert_eq!(Ok(vec![0x61, 0x62]), parsed);
    }

    #[test]
    fn ctrl_char() {
        let parsed = parse("ctrl-A");
        assert_eq!(Ok(vec![0x01]), parsed);
    }

    #[test]
    fn ctrl_char_lowercase() {
        let parsed = parse("ctrl-a");
        assert_eq!(Ok(vec![0x01]), parsed);
    }

    #[test]
    fn invalid_seq() {
        let parsed = parse("bad");
        assert_eq!(
            Err(KeyParseError::InvalidSequence("bad".to_string())),
            parsed
        );
    }

    #[test]
    fn invalid_ctrl_low() {
        let parsed = parse("ctrl-?");
        assert_eq!(
            Err(KeyParseError::InvalidCtrlCharacter(
                "ctrl-?".to_string(),
                '?'
            )),
            parsed
        );
    }

    #[test]
    fn invalid_ctrl_high() {
        let parsed = parse("ctrl-`");
        assert_eq!(
            Err(KeyParseError::InvalidCtrlCharacter(
                "ctrl-`".to_string(),
                '`'
            )),
            parsed
        );
    }
}

#[cfg(test)]
mod filter_tests {
    use super::{Action, BindingFilter, KeyBinding};

    fn state(sequence: Vec<u8>) -> BindingFilter {
        BindingFilter::new(KeyBinding {
            sequence,
            action: Action::Disconnect,
        })
    }

    #[test]
    fn binding_simple() {
        let mut state = state(vec![0]);
        let data = &[0];
        assert_eq!(Some(0), state.find(data))
    }

    #[test]
    fn binding_simple_long_input() {
        let mut state = state(vec![1u8]);

        let data = &[0, 1];
        assert_eq!(Some(1), state.find(data));
    }

    #[test]
    fn binding_simple_very_long_input() {
        let mut state = state(vec![1]);

        let data = &[0, 1, 2];
        assert_eq!(Some(1), state.find(data))
    }

    #[test]
    fn binding_2_simple() {
        let mut state = state(vec![0, 1]);

        let data = &[0, 1, 2];
        assert_eq!(Some(0), state.find(data))
    }

    #[test]
    fn binding_miss() {
        let mut state = state(vec![0]);
        let data = &[1];
        assert_eq!(None, state.find(data))
    }

    #[test]
    fn binding_miss_long() {
        let mut state = state(vec![0]);
        let data = &[1, 2];
        assert_eq!(None, state.find(data))
    }

    #[test]
    fn binding_2_long() {
        let mut state = state(vec![1, 2]);

        let data = &[0, 1, 2];
        assert_eq!(Some(1), state.find(data))
    }

    #[test]
    fn binding_2_split() {
        let mut state = state(vec![1, 2]);

        assert_eq!(None, state.find(&[0, 1]));
        assert_eq!(Some(0), state.find(&[2]));
    }

    #[test]
    fn binding_2_reset() {
        let mut state = state(vec![1, 2]);

        assert_eq!(Some(2), state.find(&[0, 1, 1, 2]));
    }
}

#[cfg(test)]
mod input_tests {
    use crate::config::Action;

    use super::{Input, InputFilter, KeyBinding};

    #[test]
    fn simple() {
        let binding = KeyBinding {
            sequence: vec![1],
            action: Action::Disconnect,
        };

        let mut filter = InputFilter::new(vec![binding]);
        let data = vec![0, 1];
        let result = filter.input(data.as_slice());
        assert_eq!(
            Input {
                action: Some(Action::Disconnect),
                data: &[0]
            },
            result
        );
    }

    #[test]
    fn multiple_segments() {
        let binding = KeyBinding {
            sequence: vec![1, 2],
            action: Action::Disconnect,
        };

        let mut filter = InputFilter::new(vec![binding]);
        let data = vec![0, 1];
        let result = filter.input(data.as_slice());
        assert_eq!(
            Input {
                action: None,
                data: &[0, 1]
            },
            result
        );

        let data = vec![2, 3];
        let result = filter.input(data.as_slice());
        assert_eq!(
            Input {
                action: Some(Action::Disconnect),
                data: &[]
            },
            result
        );
    }

    #[test]
    fn multiple_matches() {
        let binding1 = KeyBinding {
            sequence: vec![1, 2],
            action: Action::Disconnect,
        };

        let binding2 = KeyBinding {
            sequence: vec![1],
            action: Action::Disconnect,
        };

        let mut filter = InputFilter::new(vec![binding1, binding2]);
        let data = vec![0, 1, 2, 3];
        let result = filter.input(data.as_slice());
        assert_eq!(
            Input {
                action: Some(Action::Disconnect),
                data: &[0]
            },
            result
        );
    }
}
