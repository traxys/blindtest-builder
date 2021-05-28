use std::{borrow::Cow, collections::HashMap, path::PathBuf};

use iced::{
    button, text_input, Button, Color, Column, Command, Container, Element, Row, Space, Text,
    TextInput,
};
use iced_aw::Card;

use crate::{style, Clip, Message};

mod clip_builder;
pub use clip_builder::{ClipBuilderMessage, ClipBuilderState};
mod clip_editor;
pub(crate) use clip_editor::{ClipEditorMessage, ClipEditorState};

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

#[derive(Clone, Debug)]
pub(crate) enum ModalMessage {
    ClipBuilder(ClipBuilderMessage),
    ClipEditor(ClipEditorMessage),
    GlobalSettings(GlobalSettingsMessage),
}

pub(crate) enum ModalInnerState {
    ClipBuilder(ClipBuilderState),
    ClipEditor(ClipEditorState),
    GlobalSettings(GlobalSettingsState),
    None,
}

pub(crate) struct ModalState {
    pub(crate) inner: ModalInnerState,

    cancel_state: button::State,
    ok_state: button::State,
}

impl ModalState {
    pub(crate) fn new() -> Self {
        Self {
            inner: ModalInnerState::None,
            cancel_state: Default::default(),
            ok_state: Default::default(),
        }
    }
    pub(crate) fn close(&mut self) {
        self.inner = ModalInnerState::None;
    }

    pub(crate) fn view(
        &mut self,
        clips: &HashMap<String, Clip>,
        clip_duration: u32,
    ) -> Element<Message> {
        let (title, content, confirm) = match &mut self.inner {
            ModalInnerState::ClipBuilder(c) => c.view(),
            ModalInnerState::ClipEditor(c) => c.view(
                clips
                    .get(c.clip_name())
                    .expect("clip referencing deleted clip"),
                clip_duration,
            ),
            ModalInnerState::None => {
                eprintln!("Error: tried to render empty modal");
                return Space::new(iced::Length::Shrink, iced::Length::Shrink).into();
            }
            ModalInnerState::GlobalSettings(g) => g.view(),
        };

        Card::new(Text::new(title), content)
            .foot(
                Row::new()
                    .spacing(10)
                    .padding(5)
                    .width(iced::Length::Fill)
                    .push(
                        Button::new(
                            &mut self.cancel_state,
                            Text::new("Cancel")
                                .horizontal_alignment(iced::HorizontalAlignment::Center),
                        )
                        .width(iced::Length::Fill)
                        .on_press(Message::ModalCancel)
                        .style(style::Button::Destructive),
                    )
                    .push(
                        Button::new(
                            &mut self.ok_state,
                            Text::new("Ok").horizontal_alignment(iced::HorizontalAlignment::Center),
                        )
                        .width(iced::Length::Fill)
                        .on_press(confirm)
                        .style(style::Button::Primary),
                    ),
            )
            .max_width(400)
            .on_close(Message::ModalClosed)
            .into()
    }
}
