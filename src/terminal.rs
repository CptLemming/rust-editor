use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::style::{
    Attribute, Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{
    Clear, ClearType, DisableLineWrap, EnableLineWrap, EnterAlternateScreen, LeaveAlternateScreen,
    SetTitle, disable_raw_mode,
};
use crossterm::terminal::{enable_raw_mode, size};
use crossterm::{Command, queue};
use std::io::{Write, stdout};

use crate::annotated::{AnnotatedString, AnnotationType};

pub struct TerminalAttribute {
    pub foreground: Option<Color>,
    pub background: Option<Color>,
}

impl From<AnnotationType> for TerminalAttribute {
    fn from(value: AnnotationType) -> Self {
        match value {
            AnnotationType::Match => Self {
                foreground: Some(Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                }),
                background: Some(Color::Rgb {
                    r: 100,
                    g: 100,
                    b: 100,
                }),
            },
            AnnotationType::SelectedMatch => Self {
                foreground: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
                background: Some(Color::Rgb {
                    r: 255,
                    g: 251,
                    b: 0,
                }),
            },
            AnnotationType::Number => Self {
                foreground: Some(Color::Rgb {
                    r: 255,
                    g: 99,
                    b: 71,
                }),
                background: None,
            },
            AnnotationType::Keyword => Self {
                foreground: Some(Color::Rgb {
                    r: 100,
                    g: 146,
                    b: 237,
                }),
                background: None,
            },
            AnnotationType::Type => Self {
                foreground: Some(Color::Rgb {
                    r: 175,
                    g: 225,
                    b: 175,
                }),
                background: None,
            },
            AnnotationType::KnownValue => Self {
                foreground: Some(Color::Rgb {
                    r: 195,
                    g: 177,
                    b: 225,
                }),
                background: None,
            },
            AnnotationType::Char => Self {
                foreground: Some(Color::Rgb {
                    r: 255,
                    g: 191,
                    b: 0,
                }),
                background: None,
            },
            AnnotationType::LifetimeSpecifier => Self {
                foreground: Some(Color::Rgb {
                    r: 102,
                    g: 205,
                    b: 170,
                }),
                background: None,
            },
            AnnotationType::Comment => Self {
                foreground: Some(Color::Rgb {
                    r: 34,
                    g: 139,
                    b: 34,
                }),
                background: None,
            },
            AnnotationType::String => Self {
                foreground: Some(Color::Rgb {
                    r: 255,
                    g: 179,
                    b: 102,
                }),
                background: None,
            },
        }
    }
}

#[derive(Default, Clone, PartialEq, Eq)]
pub struct Size {
    pub height: usize,
    pub width: usize,
}

#[derive(Default, Clone)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

impl Position {
    pub const fn saturating_sub(&self, other: &Self) -> Self {
        Self {
            x: self.x.saturating_sub(other.x),
            y: self.y.saturating_sub(other.y),
        }
    }
}

#[derive(Default)]
pub struct Terminal;

impl Terminal {
    pub fn init(&self) -> anyhow::Result<()> {
        enable_raw_mode()?;
        self.enter_alternate_screen()?;
        self.disable_line_wrap()?;
        self.clear()?;
        self.execute()?;
        Ok(())
    }

    pub fn clear(&self) -> anyhow::Result<()> {
        self.queue_command(Clear(ClearType::All))?;
        Ok(())
    }

    pub fn clear_line(&self) -> anyhow::Result<()> {
        self.queue_command(Clear(ClearType::CurrentLine))?;
        Ok(())
    }

    pub fn move_cursor(&self, position: &Position) -> anyhow::Result<()> {
        self.queue_command(MoveTo(position.x as u16, position.y as u16))?;
        Ok(())
    }

    pub fn show_cursor(&self) -> anyhow::Result<()> {
        self.queue_command(Show)?;
        Ok(())
    }

    pub fn hide_cursor(&self) -> anyhow::Result<()> {
        self.queue_command(Hide)?;
        Ok(())
    }

    pub fn set_title(&self, title: &str) -> anyhow::Result<()> {
        self.queue_command(SetTitle(title))?;
        Ok(())
    }

    pub fn print(&self, message: &str) -> anyhow::Result<()> {
        self.queue_command(Print(message))?;
        Ok(())
    }

    pub fn print_row(&self, row: usize, text: &str) -> anyhow::Result<()> {
        self.move_cursor(&Position { x: 0, y: row })?;
        self.clear_line()?;
        self.print(text)?;
        Ok(())
    }

    pub fn print_inverted_row(&self, row: usize, text: &str) -> anyhow::Result<()> {
        let width = self.size()?.width;
        self.print_row(
            row,
            &format!(
                "{}{:width$.width$}{}",
                Attribute::Reverse,
                text,
                Attribute::Reset
            ),
        )
    }

    pub fn print_annotated_row(
        &self,
        row: usize,
        annotated_string: &AnnotatedString,
    ) -> anyhow::Result<()> {
        self.move_cursor(&Position { x: 0, y: row })?;
        self.clear_line()?;

        annotated_string
            .into_iter()
            .try_for_each(|part| -> anyhow::Result<()> {
                if let Some(annotation_type) = part.annotation_type {
                    let attribute: TerminalAttribute = annotation_type.into();
                    self.set_attribute(&attribute)?;
                }

                self.print(part.string)?;
                self.reset_color()?;

                Ok(())
            })?;

        Ok(())
    }

    pub fn set_attribute(&self, attribute: &TerminalAttribute) -> anyhow::Result<()> {
        if let Some(foreground_color) = attribute.foreground {
            self.queue_command(SetForegroundColor(foreground_color))?;
        }
        if let Some(background_color) = attribute.background {
            self.queue_command(SetBackgroundColor(background_color))?;
        }

        Ok(())
    }

    pub fn reset_color(&self) -> anyhow::Result<()> {
        self.queue_command(ResetColor)?;
        Ok(())
    }

    pub fn enter_alternate_screen(&self) -> anyhow::Result<()> {
        self.queue_command(EnterAlternateScreen)?;
        Ok(())
    }

    pub fn leave_alternate_screen(&self) -> anyhow::Result<()> {
        self.queue_command(LeaveAlternateScreen)?;
        Ok(())
    }

    pub fn disable_line_wrap(&self) -> anyhow::Result<()> {
        self.queue_command(DisableLineWrap)?;
        Ok(())
    }

    pub fn enable_line_wrap(&self) -> anyhow::Result<()> {
        self.queue_command(EnableLineWrap)?;
        Ok(())
    }

    pub fn terminate(&self) -> anyhow::Result<()> {
        self.leave_alternate_screen()?;
        self.enable_line_wrap()?;
        self.show_cursor()?;
        self.execute()?;
        disable_raw_mode()?;
        Ok(())
    }

    pub fn execute(&self) -> anyhow::Result<()> {
        stdout().flush()?;
        Ok(())
    }

    pub fn size(&self) -> anyhow::Result<Size> {
        let (width, height) = size()?;
        Ok(Size {
            height: height as usize,
            width: width as usize,
        })
    }

    fn queue_command<T: Command>(&self, command: T) -> anyhow::Result<()> {
        queue!(stdout(), command)?;
        Ok(())
    }
}
