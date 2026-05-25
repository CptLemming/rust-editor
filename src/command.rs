use std::io::Read;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use wl_clipboard_rs::paste;

use crate::terminal::Size;

pub enum Move {
    PageUp,
    PageDown,
    Home,
    End,
    Up,
    Down,
    Left,
    Right,
}

impl TryFrom<KeyEvent> for Move {
    type Error = String;
    fn try_from(event: KeyEvent) -> Result<Self, Self::Error> {
        let KeyEvent {
            code, modifiers, ..
        } = event;

        if modifiers == KeyModifiers::NONE {
            match code {
                KeyCode::Up => Ok(Move::Up),
                KeyCode::Down => Ok(Move::Down),
                KeyCode::Left => Ok(Move::Left),
                KeyCode::Right => Ok(Move::Right),
                KeyCode::PageUp => Ok(Move::PageUp),
                KeyCode::PageDown => Ok(Move::PageDown),
                KeyCode::Home => Ok(Move::Home),
                KeyCode::End => Ok(Move::End),
                _ => Err(format!("Unsupported code {code:?}")),
            }
        } else {
            Err(format!(
                "Unsupported key code {code:?} or modifier {modifiers:?}"
            ))
        }
    }
}

pub enum Edit {
    Insert(char),
    Enter,
    Delete,
    Backspace,
}

impl TryFrom<KeyEvent> for Edit {
    type Error = String;
    fn try_from(event: KeyEvent) -> Result<Self, Self::Error> {
        match (event.code, event.modifiers) {
            (KeyCode::Char(char), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                Ok(Self::Insert(char))
            }
            (KeyCode::Tab, _) => Ok(Self::Insert('\t')),
            (KeyCode::Backspace, _) => Ok(Self::Backspace),
            (KeyCode::Delete, _) => Ok(Self::Delete),
            (KeyCode::Enter, _) => Ok(Self::Enter),
            _ => Err(format!(
                "Unsupported key code {:?} with modifiers {:?}",
                event.code, event.modifiers
            )),
        }
    }
}

pub enum System {
    Save,
    Resize(Size),
    Quit,
    Dismiss,
    Search,
    Copy,
    Paste(String),
}

impl TryFrom<KeyEvent> for System {
    type Error = String;
    fn try_from(event: KeyEvent) -> Result<Self, Self::Error> {
        let KeyEvent {
            code, modifiers, ..
        } = event;

        if modifiers == KeyModifiers::CONTROL {
            match code {
                KeyCode::Char('q') => Ok(Self::Quit),
                KeyCode::Char('s') => Ok(Self::Save),
                KeyCode::Char('f') => Ok(Self::Search),
                KeyCode::Char('c') => Ok(Self::Copy),
                // Manual paste - Ctrl+V
                KeyCode::Char('v') => paste::get_contents(
                    paste::ClipboardType::Regular,
                    paste::Seat::Unspecified,
                    paste::MimeType::Text,
                )
                .map(|(mut pipe, _)| {
                    let mut contents = vec![];
                    let _ = pipe.read_to_end(&mut contents);

                    String::from_utf8_lossy(&contents).to_string()
                })
                .map(Self::Paste)
                .map_err(|err| format!("Paste error : {err:?}")),
                _ => Err(format!("Unsupported CTRL+{code:?} combination")),
            }
        } else if modifiers == KeyModifiers::NONE && matches!(code, KeyCode::Esc) {
            Ok(Self::Dismiss)
        } else {
            Err(format!(
                "Unsupported key code {code:?} or modifier {modifiers:?}"
            ))
        }
    }
}

pub enum Command {
    Move(Move),
    Edit(Edit),
    System(System),
}

impl TryFrom<Event> for Command {
    type Error = String;

    fn try_from(event: Event) -> Result<Self, Self::Error> {
        match event {
            Event::Key(key_event) => Edit::try_from(key_event)
                .map(Command::Edit)
                .or_else(|_| Move::try_from(key_event).map(Command::Move))
                .or_else(|_| System::try_from(key_event).map(Command::System))
                .map_err(|_err| format!("Event not supported : {key_event:?}")),
            // Terminal specific paste - Ctrl+Shift+V
            Event::Paste(text) => Ok(Self::System(System::Paste(text))),
            Event::Resize(width, height) => Ok(Self::System(System::Resize(Size {
                height: height as usize,
                width: width as usize,
            }))),
            _ => Err(format!("Event not supported : {event:?}")),
        }
    }
}
