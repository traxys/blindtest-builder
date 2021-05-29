use iced::{button, Button, Element, Row, Space, Text};
use iced_aw::Card;
use std::collections::HashMap;

use crate::{style, Clip, Message};

mod clip_builder;
pub use clip_builder::{ClipBuilderMessage, ClipBuilderState};
mod clip_editor;
pub(crate) use clip_editor::{ClipEditorMessage, ClipEditorState};
mod global_settings;
pub(crate) use global_settings::{GlobalSettingsMessage, GlobalSettingsState};

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

impl ModalInnerState {
    fn has_cancel(&self) -> bool {
        match self {
            ModalInnerState::ClipBuilder(_)
            | ModalInnerState::ClipEditor(_)
            | ModalInnerState::GlobalSettings(_) => true,
            ModalInnerState::None => false,
        }
    }
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
        let has_cancel = self.inner.has_cancel();
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

        let mut controls = Row::new().spacing(10).padding(5).width(iced::Length::Fill);
        if has_cancel {
            controls = controls.push(
                Button::new(
                    &mut self.cancel_state,
                    Text::new("Cancel").horizontal_alignment(iced::HorizontalAlignment::Center),
                )
                .width(iced::Length::Fill)
                .on_press(Message::ModalCancel)
                .style(style::Button::Destructive),
            )
        }

        controls = controls.push(
            Button::new(
                &mut self.ok_state,
                Text::new("Ok").horizontal_alignment(iced::HorizontalAlignment::Center),
            )
            .width(iced::Length::Fill)
            .on_press(confirm)
            .style(style::Button::Primary),
        );

        Card::new(Text::new(title), content)
            .foot(controls)
            .max_width(400)
            .on_close(Message::ModalClosed)
            .into()
    }
}
