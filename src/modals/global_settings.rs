use super::ModalMessage;
use crate::{style, Message};
use iced::{
    button, text_input, Button, Color, Column, Command, Container, Element, Row, Text, TextInput,
};
use std::{borrow::Cow, path::PathBuf};

pub(crate) struct GlobalSettingsState {
    duration_input: text_input::State,
    current_duration: String,

    countdown: Option<PathBuf>,
    countdown_button: button::State,

    error: Option<String>,
}

pub(crate) struct Settings {
    pub duration: u32,
    pub countdown: Option<PathBuf>,
}

async fn select_file() -> Option<PathBuf> {
    let dialog = native_dialog::FileDialog::new();

    match dialog.show_open_single_file() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error getting path: {:?}", e);
            None
        }
    }
}

impl GlobalSettingsState {
    fn apply(&mut self) -> Result<Settings, String> {
        let duration = match self.current_duration.parse() {
            Err(_) => return Err("Duration is invalid".into()),
            Ok(v) => v,
        };
        Ok(Settings {
            duration,
            countdown: self.countdown.take(),
        })
    }

    pub(crate) fn new(current_duration: u32, countdown: Option<PathBuf>) -> Self {
        GlobalSettingsState {
            duration_input: Default::default(),
            current_duration: current_duration.to_string(),
            countdown_button: Default::default(),
            countdown,
            error: None,
        }
    }

    pub(crate) fn view(&mut self) -> (String, Element<Message>, Message) {
        let mut form = Column::new().spacing(5);

        if let Some(err) = &self.error {
            form = form.push(
                Container::new(
                    Text::new(err)
                        .color(Color::from_rgb8(0xff, 0x00, 0x33))
                        .size(30),
                )
                .padding(20),
            );
        }

        form = form
            .push(
                Row::new()
                    .spacing(10)
                    .align_items(iced::Align::Center)
                    .push(Text::new("Clip Duration:").size(24))
                    .push(
                        TextInput::new(
                            &mut self.duration_input,
                            "",
                            &self.current_duration,
                            wrap_gs(GlobalSettingsMessage::UpdateDuration),
                        )
                        .padding(10),
                    ),
            )
            .push(
                Row::new()
                    .push(Container::new(Text::new("Countdown:").size(24)).padding(5))
                    .push(
                        Button::new(
                            &mut self.countdown_button,
                            Text::new(
                                self.countdown
                                    .as_ref()
                                    .map(|p| {
                                        p.file_name()
                                            .expect("native dialog selected file name")
                                            .to_string_lossy()
                                    })
                                    .unwrap_or(Cow::Borrowed("No Clip Selected")),
                            ),
                        )
                        .padding(10)
                        .style(style::Button::Primary)
                        .on_press(GlobalSettingsMessage::SelectCountdown.into()),
                    )
                    .align_items(iced::Align::Center)
                    .padding(5),
            );

        (
            "Global Settings".into(),
            form.into(),
            GlobalSettingsMessage::UpdateSettings.into(),
        )
    }

    pub(crate) fn update<A: FnOnce(Settings)>(
        &mut self,
        msg: GlobalSettingsMessage,
        apply: A,
    ) -> (Command<Message>, bool) {
        match msg {
            GlobalSettingsMessage::UpdateSettings => match self.apply() {
                Err(e) => {
                    self.error = Some(e);
                }
                Ok(settings) => {
                    apply(settings);
                    return (Command::none(), true);
                }
            },
            GlobalSettingsMessage::UpdateDuration(v) => {
                self.current_duration = v;
            }
            GlobalSettingsMessage::SelectCountdown => {
                return (
                    Command::perform(select_file(), wrap_gs(GlobalSettingsMessage::CountDownPath)),
                    false,
                )
            }
            GlobalSettingsMessage::CountDownPath(p) => self.countdown = p,
        }

        (Command::none(), false)
    }
}

#[derive(Clone, Debug)]
pub(crate) enum GlobalSettingsMessage {
    UpdateDuration(String),
    CountDownPath(Option<PathBuf>),
    SelectCountdown,
    UpdateSettings,
}

fn wrap_gs<T, F: Fn(T) -> GlobalSettingsMessage>(f: F) -> impl Fn(T) -> Message {
    move |v| Message::Modal(ModalMessage::GlobalSettings(f(v)))
}

impl From<GlobalSettingsMessage> for Message {
    fn from(m: GlobalSettingsMessage) -> Self {
        Message::Modal(ModalMessage::GlobalSettings(m))
    }
}
