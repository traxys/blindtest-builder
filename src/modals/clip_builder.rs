use crate::{style, Clip, Message};
use iced::{
    button, text_input, Button, Color, Column, Command, Container, Element, Row, Text, TextInput,
};
use std::{borrow::Cow, collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

#[derive(Clone, Debug)]
pub enum ClipBuilderMessage {
    TitleChanged(String),
    PickImage,
    PickMusic,
    PickedImage(Option<PathBuf>),
    PickedMusic(Option<PathBuf>),
    Add,
}

impl From<ClipBuilderMessage> for Message {
    fn from(m: ClipBuilderMessage) -> Self {
        Self::Modal(crate::ModalMessage::ClipBuilder(m))
    }
}

async fn select_file(image: bool) -> Option<PathBuf> {
    let mut dialog = native_dialog::FileDialog::new();
    if image {
        dialog = dialog
            .add_filter("JPEG", &["jpg", "jpeg"])
            .add_filter("PNG", &["png"]);
    } else {
        dialog = dialog
            .add_filter("MP3", &["mp3"])
            .add_filter("OGG", &["ogg"])
            .add_filter("WAV", &["wav"])
            .add_filter("FLAC", &["flac"]);
    }

    match dialog.show_open_single_file() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error getting path: {:?}", e);
            None
        }
    }
}

#[derive(Default)]
pub struct ClipBuilderState {
    title_state: text_input::State,
    img_state: button::State,
    music_state: button::State,

    error: Option<String>,
    title: String,
    image: Option<PathBuf>,
    music: Option<PathBuf>,
}

impl ClipBuilderState {
    fn build(&mut self) -> Result<Clip, String> {
        if self.title.is_empty() {
            return Err("Title must not be empty".into());
        }
        let (music, music_path) = match self.music.take() {
            None => return Err("No music was provided".into()),
            Some(path) => {
                let file = match std::fs::read(&path) {
                    Ok(f) => f,
                    Err(e) => {
                        self.music = Some(path);
                        return Err(format!("Could not open music: {}", e));
                    }
                };
                (Arc::new(file.into_boxed_slice()), path)
            }
        };
        let (image, image_path) = match self.image.take() {
            None => {
                self.music = Some(music_path);
                return Err("No image was provided".into());
            }
            Some(img) => ((&img).into(), img),
        };

        self.error = None;
        Ok(Clip {
            title: std::mem::take(&mut self.title),
            music,
            music_path,
            image,
            image_path,
            offset: Duration::from_secs(0),
            duration: Duration::from_secs(0),
        }
        .fetch_duration())
    }

    pub(crate) fn view(&mut self) -> (String, Element<Message>, Message) {
        let mut form = Column::new();

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
                    .push(Container::new(Text::new("Title:").size(24)).padding(5))
                    .push(
                        TextInput::new(&mut self.title_state, "Title", &self.title, |t| {
                            ClipBuilderMessage::TitleChanged(t).into()
                        })
                        .padding(10),
                    )
                    .align_items(iced::Align::Center)
                    .padding(5),
            )
            .push(
                Row::new()
                    .push(Container::new(Text::new("Image:").size(24)).padding(5))
                    .push(
                        Button::new(
                            &mut self.img_state,
                            Text::new(
                                self.image
                                    .as_ref()
                                    .map(|p| {
                                        p.file_name()
                                            .expect("native dialog selected file name")
                                            .to_string_lossy()
                                    })
                                    .unwrap_or(Cow::Borrowed("No Image Selected")),
                            ),
                        )
                        .padding(10)
                        .style(style::Button::Primary)
                        .on_press(ClipBuilderMessage::PickImage.into()),
                    )
                    .align_items(iced::Align::Center)
                    .padding(5),
            )
            .push(
                Row::new()
                    .push(Container::new(Text::new("Music:").size(24)).padding(5))
                    .push(
                        Button::new(
                            &mut self.music_state,
                            Text::new(
                                self.music
                                    .as_ref()
                                    .map(|p| {
                                        p.file_name()
                                            .expect("native dialog selected file name")
                                            .to_string_lossy()
                                    })
                                    .unwrap_or(Cow::Borrowed("No Music Selected")),
                            ),
                        )
                        .padding(10)
                        .style(style::Button::Primary)
                        .on_press(ClipBuilderMessage::PickMusic.into()),
                    )
                    .align_items(iced::Align::Center)
                    .padding(5),
            );

        (
            "Add Clip".into(),
            form.into(),
            ClipBuilderMessage::Add.into(),
        )
    }

    pub(crate) fn update(
        &mut self,
        message: ClipBuilderMessage,
        clips: &mut HashMap<String, Clip>,
    ) -> (Command<Message>, bool) {
        match message {
            ClipBuilderMessage::TitleChanged(t) => {
                self.title = t;
            }
            ClipBuilderMessage::PickImage => {
                return (
                    Command::perform(select_file(true), |p| {
                        ClipBuilderMessage::PickedImage(p).into()
                    }),
                    false,
                )
            }
            ClipBuilderMessage::PickMusic => {
                return (
                    Command::perform(select_file(false), |p| {
                        ClipBuilderMessage::PickedMusic(p).into()
                    }),
                    false,
                )
            }
            ClipBuilderMessage::PickedImage(img) => self.image = img,
            ClipBuilderMessage::PickedMusic(msc) => self.music = msc,
            ClipBuilderMessage::Add => match self.build() {
                Err(err) => {
                    self.error = Some(err.into());
                }
                Ok(clip) => {
                    clips.insert(clip.title.clone(), clip);
                    return (Command::none(), true);
                }
            },
        }

        (Command::none(), false)
    }
}
