use super::ModalMessage;
use crate::{style, Clip, Message};
use iced::{button, slider, Button, Column, Command, Element, Image, Row, Slider, Text};
use rodio::{OutputStreamHandle, Sink};
use std::collections::HashMap;

pub(crate) struct ClipEditorState {
    clip: String,

    playing: bool,
    audio_button: button::State,
    delete_button: button::State,
    offset_slider: slider::State,
    sink: Sink,
}

impl ClipEditorState {
    pub(crate) fn clip_name(&self) -> &str {
        &self.clip
    }

    pub(crate) fn new(clip: String, stream_handle: &OutputStreamHandle) -> anyhow::Result<Self> {
        Ok(Self {
            clip,
            playing: false,
            audio_button: Default::default(),
            delete_button: Default::default(),
            offset_slider: Default::default(),
            sink: Sink::try_new(stream_handle)?,
        })
    }

    pub(crate) fn update(
        &mut self,
        message: ClipEditorMessage,
        stream_handle: &OutputStreamHandle,
        duration: u32,
        clips: &mut HashMap<String, crate::Clip>,
    ) -> (Command<Message>, bool) {
        let clip = clips.get(&self.clip).expect("clip was deleted somehow");

        match message {
            ClipEditorMessage::PlayClip => {
                match clip.audio(duration) {
                    Ok(a) => self.sink.append(a),
                    Err(e) => eprintln!("Could not decode audio: {:?}", e),
                };
                self.playing = true
            }
            ClipEditorMessage::StopClip => {
                self.sink = Sink::try_new(stream_handle).expect("could not create new sink");
                self.playing = false
            }
            ClipEditorMessage::Delete => {
                clips.remove(&self.clip);
                return (Command::none(), true);
            }
        }

        (Command::none(), false)
    }

    pub(crate) fn view(
        &mut self,
        clip: &Clip,
        clip_duration: u32,
    ) -> (String, Element<Message>, Message) {
        let audio_button = Button::new(
            &mut self.audio_button,
            Text::new(if self.playing {
                "Stop Clip"
            } else {
                "Play Clip"
            }),
        )
        .style(if self.playing {
            style::Button::Destructive
        } else {
            style::Button::Primary
        })
        .on_press(if self.playing {
            ClipEditorMessage::StopClip.into()
        } else {
            ClipEditorMessage::PlayClip.into()
        });

        let title = clip.title.clone();

        let content = Column::new()
            .spacing(5)
            .push(Image::new(clip.image.clone()))
            .push(
                Row::new()
                    .align_items(iced::Align::Center)
                    .push(Text::new(&format!(
                        "Start offset ({}):",
                        clip.offset.as_secs()
                    )))
                    .push(Slider::new(
                        &mut self.offset_slider,
                        0.0..=(clip.duration.as_secs_f64() - clip_duration as f64),
                        clip.offset.as_secs_f64(),
                        move |new_offset| Message::EditClipOffset {
                            clip: title.clone(),
                            new_offset: new_offset as u32,
                        },
                    )),
            )
            .push(audio_button)
            .push(
                Button::new(&mut self.delete_button, Text::new("Delete Clip"))
                    .style(style::Button::Destructive)
                    .on_press(ClipEditorMessage::Delete.into()),
            )
            .align_items(iced::Align::Center);

        (
            format!("Edit Clip: {}", self.clip),
            content.into(),
            Message::ModalClosed,
        )
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ClipEditorMessage {
    PlayClip,
    StopClip,
    Delete,
}

impl From<ClipEditorMessage> for Message {
    fn from(m: ClipEditorMessage) -> Self {
        Message::Modal(ModalMessage::ClipEditor(m))
    }
}
