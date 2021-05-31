use crate::{
    export::{Export, Progress},
    style, Clip, Message,
};
use iced::{
    button, pick_list, scrollable, Button, Column, Command, Container, Element, Image, Length,
    PickList, ProgressBar, Row, Rule, Scrollable, Space, Subscription, Text,
};
use itertools::Itertools;
use rodio::{OutputStreamHandle, Sink};
use std::{
    collections::{HashMap, VecDeque},
    iter::FromIterator,
    path::PathBuf,
    time::Duration,
};

const CLIP_HEIGHT: u32 = 300;

#[derive(Debug, Clone)]
pub(crate) enum TimelineMessage {
    AddStart,
    AddEnd,
    Play,
    Stop,
    TimelineClip(usize, TimelineClipMessage),
    Save,
    SaveTo(Option<PathBuf>),
    ExportProgress(crate::export::Progress),
}

#[derive(Debug, Clone)]
pub(crate) enum TimelineClipMessage {
    SelectedClip(String),
    ValidateClip,
    Action(TimelineAction),
    Play,
    Stop,
}

#[derive(Clone, Debug)]
pub(crate) enum TimelineAction {
    Up,
    Down,
    Delete,
}

impl From<TimelineMessage> for Message {
    fn from(m: TimelineMessage) -> Self {
        Message::Timeline(m)
    }
}

impl From<TimelineAction> for TimelineClipMessage {
    fn from(m: TimelineAction) -> TimelineClipMessage {
        TimelineClipMessage::Action(m)
    }
}

fn timeline_clip_msg(index: usize, msg: TimelineClipMessage) -> Message {
    TimelineMessage::TimelineClip(index, msg).into()
}

struct TimelineClip {
    clip: Option<String>,

    // When no clip is selected
    clip_select: pick_list::State<String>,
    selected: Option<String>,
    validate_clip: button::State,

    // When a clip is selected
    audio_button: button::State,
    playing: bool,
    sink: rodio::Sink,

    // generic controls
    up_button: button::State,
    down_button: button::State,
    delete: button::State,
}

impl TimelineClip {
    fn new(stream_handle: &OutputStreamHandle) -> Self {
        Self {
            clip: None,
            selected: None,
            validate_clip: Default::default(),
            clip_select: Default::default(),
            up_button: Default::default(),
            down_button: Default::default(),
            delete: Default::default(),
            audio_button: Default::default(),
            playing: false,
            sink: Sink::try_new(stream_handle).expect("could not build sink"),
        }
    }

    fn update(
        &mut self,
        msg: TimelineClipMessage,
        clip: Option<&Clip>,
        duration: u32,
        stream_handle: &OutputStreamHandle,
    ) -> (Command<Message>, Option<TimelineAction>) {
        match msg {
            TimelineClipMessage::SelectedClip(c) => self.selected = Some(c),
            TimelineClipMessage::ValidateClip => self.clip = self.selected.clone(),
            TimelineClipMessage::Action(a) => return (Command::none(), Some(a)),
            TimelineClipMessage::Play => {
                let clip = clip.expect("clip must be present in this command");
                match clip.audio(duration) {
                    Ok(a) => self.sink.append(a),
                    Err(e) => eprintln!("Could not decode audio: {:?}", e),
                };
                self.playing = true;
            }
            TimelineClipMessage::Stop => {
                self.sink = Sink::try_new(stream_handle).expect("could not create new sink");
                self.playing = false
            }
        }

        (Command::none(), None)
    }

    fn view(
        &mut self,
        clips: &HashMap<String, Clip>,
        index: usize,
        len: usize,
    ) -> Element<Message> {
        if let Some(clip) = &self.clip {
            if !clips.contains_key(clip) {
                self.clip = None;
            }
        }
        if let Some(clip) = &self.selected {
            if !clips.contains_key(clip) {
                self.selected = None;
            }
        }

        let content: Element<_> = match &self.clip {
            Some(clip) => {
                let clip_data = clips.get(clip).expect("just checked");
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
                    timeline_clip_msg(index, TimelineClipMessage::Stop)
                } else {
                    timeline_clip_msg(index, TimelineClipMessage::Play)
                });

                let controls = Column::new()
                    .align_items(iced::Align::Center)
                    .spacing(5)
                    .push(audio_button);

                Column::new()
                    .push(Text::new(clip))
                    .spacing(5)
                    .push(
                        Container::new(
                            Row::new()
                                .push(Image::new(clip_data.image.clone()))
                                .push(controls)
                                .spacing(10)
                                .align_items(iced::Align::Center),
                        )
                        .max_height(CLIP_HEIGHT - 120),
                    )
                    .align_items(iced::Align::Center)
                    .into()
            }
            None => Column::new()
                .push(Text::new("No Clip Selected"))
                .push(Space::with_height(Length::Units(5)))
                .push(
                    Row::new()
                        .push(PickList::new(
                            &mut self.clip_select,
                            Vec::from_iter(
                                clips
                                    .keys()
                                    .cloned()
                                    .sorted_by(|a, b| lexical_sort::natural_lexical_cmp(a, b)),
                            ),
                            self.selected.clone(),
                            move |c| timeline_clip_msg(index, TimelineClipMessage::SelectedClip(c)),
                        ))
                        .push(Space::with_width(Length::Units(10)))
                        .push({
                            let mut button =
                                Button::new(&mut self.validate_clip, Text::new("Validate Clip"))
                                    .style(style::Button::Primary);
                            if self.selected.is_some() {
                                button = button.on_press(timeline_clip_msg(
                                    index,
                                    TimelineClipMessage::ValidateClip,
                                ))
                            }
                            button
                        }),
                )
                .align_items(iced::Align::Center)
                .into(),
        };

        let mut up =
            Button::new(&mut self.up_button, Text::new("Up")).style(style::Button::Primary);
        if index > 0 {
            up = up.on_press(timeline_clip_msg(index, TimelineAction::Up.into()))
        }

        let mut down =
            Button::new(&mut self.down_button, Text::new("Down")).style(style::Button::Primary);
        if index + 1 < len {
            down = down.on_press(timeline_clip_msg(index, TimelineAction::Down.into()))
        }

        let controls = Row::new()
            .spacing(10)
            .align_items(iced::Align::Center)
            .push(up)
            .push(
                Button::new(&mut self.delete, Text::new("Delete"))
                    .style(style::Button::Destructive)
                    .on_press(timeline_clip_msg(index, TimelineAction::Delete.into())),
            )
            .push(down);

        Container::new(
            Column::new()
                .push(content)
                .push(Space::with_height(Length::Fill))
                .push(Rule::horizontal(20).style(style::Rule))
                .push(Space::with_height(Length::Fill))
                .push(controls)
                .spacing(5)
                .align_items(iced::Align::Center),
        )
        .center_y()
        .into()
    }
}

async fn select_export() -> Option<PathBuf> {
    let dialog = native_dialog::FileDialog::new().add_filter("MP4", &["mp4"]);
    let res = dialog.show_save_single_file();

    match res {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error getting path: {:?}", e);
            None
        }
    }
}

pub(crate) struct Timeline {
    clips: VecDeque<TimelineClip>,

    start_button: button::State,
    end_button: button::State,
    sink: Sink,
    playing: bool,
    audio_button: button::State,
    export_button: button::State,

    export: Option<PathBuf>,
    progress: f32,
    progress_max: f32,

    scroll_data: scrollable::State,
}

impl Timeline {
    pub(crate) fn subscription(
        &self,
        clips: &HashMap<String, Clip>,
        countdown: Option<PathBuf>,
        duration: u32,
    ) -> Subscription<Message> {
        match &self.export {
            Some(path) => {
                let items: Vec<_> = self
                    .clips
                    .iter()
                    .filter_map(|clip| clip.clip.as_ref())
                    .map(|s| -> &str { s })
                    .collect();
                Subscription::from_recipe(
                    Export::new(
                        path.clone(),
                        &items,
                        clips,
                        countdown.expect("Countdown was not provided"),
                        duration,
                    )
                    .expect("Export failed"),
                )
                .map(|p| Message::Timeline(TimelineMessage::ExportProgress(p)))
            }
            None => Subscription::none(),
        }
    }

    fn play_all(&self, clips: &HashMap<String, Clip>, duration: u32) {
        for clip in &self.clips {
            if let Some(clip) = &clip.clip {
                let clip_data = clips.get(clip).expect("clip not present");
                match clip_data.audio(duration) {
                    Ok(a) => self.sink.append(a),
                    Err(e) => eprintln!("Could not decode audio: {:?}", e),
                };
            }
        }
    }

    pub(crate) fn save(&self) -> Vec<Option<String>> {
        self.clips.iter().map(|clip| clip.clip.clone()).collect()
    }

    pub(crate) fn load(&mut self, clips: Vec<Option<String>>, stream_handle: &OutputStreamHandle) {
        self.clips = clips
            .into_iter()
            .map(|clip| {
                let mut tclip = TimelineClip::new(stream_handle);
                tclip.selected = clip.clone();
                tclip.clip = clip;
                tclip
            })
            .collect();
    }

    pub(crate) fn new(stream_handle: &OutputStreamHandle) -> Self {
        Timeline {
            clips: VecDeque::new(),
            scroll_data: Default::default(),
            start_button: Default::default(),
            end_button: Default::default(),
            audio_button: Default::default(),
            export_button: Default::default(),
            sink: Sink::try_new(stream_handle).expect("could not create sink"),
            playing: false,
            export: None,
            progress: 0.,
            progress_max: 0.,
        }
    }

    pub(crate) fn update(
        &mut self,
        message: TimelineMessage,
        clips: &HashMap<String, Clip>,
        stream_handle: &OutputStreamHandle,
        duration: u32,
    ) -> Command<Message> {
        match message {
            TimelineMessage::AddStart => {
                self.scroll_data.scroll_to_percentage(0.);
                self.clips.push_front(TimelineClip::new(stream_handle))
            }
            TimelineMessage::AddEnd => {
                self.scroll_data.scroll_to_percentage(1.);
                self.clips.push_back(TimelineClip::new(stream_handle))
            }
            TimelineMessage::TimelineClip(index, msg) => {
                let clip = self.clips[index]
                    .clip
                    .as_ref()
                    .map(|clip| clips.get(clip).expect("clip not present"));
                let (cmd, action) = self.clips[index].update(msg, clip, duration, stream_handle);
                if let Some(action) = action {
                    match action {
                        TimelineAction::Up => {
                            self.scroll_data.scroll_to_percentage(
                                (index - 1) as f32 / (self.clips.len() as f32 - 1.0),
                            );
                            self.clips.swap(index, index - 1)
                        }
                        TimelineAction::Down => {
                            self.scroll_data.scroll_to_percentage(
                                (index + 1) as f32 / (self.clips.len() as f32 - 1.0),
                            );
                            self.clips.swap(index, index + 1)
                        }
                        TimelineAction::Delete => {
                            self.clips.remove(index);
                        }
                    }
                }
                return cmd;
            }
            TimelineMessage::Play => {
                self.play_all(clips, duration);
                self.playing = true;
            }
            TimelineMessage::Stop => {
                self.sink = Sink::try_new(stream_handle).expect("could not create new sink");
                self.playing = false;
            }
            TimelineMessage::Save => {
                return Command::perform(select_export(), |p| {
                    Message::Timeline(TimelineMessage::SaveTo(p))
                })
            }
            TimelineMessage::SaveTo(path) => {
                self.export = path;
                self.progress_max = (25 * duration as usize * self.clips.len()) as f32;
            }
            TimelineMessage::ExportProgress(p) => match p {
                Progress::Started => {}
                Progress::Frame(f) => self.progress = f as _,
                Progress::Done => self.export = None,
                Progress::Error(e) => println!("Error in export: {}", e),
            },
        }

        Command::none()
    }

    pub(crate) fn view(
        &mut self,
        clips: &HashMap<String, Clip>,
        clip_duration: u32,
    ) -> Element<Message> {
        let mut scrollable = Scrollable::new(&mut self.scroll_data)
            .align_items(iced::Align::Center)
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .style(style::Scrollable)
            .padding(10)
            .spacing(10);

        let len = self.clips.len();
        for (index, clip) in self.clips.iter_mut().enumerate() {
            scrollable = scrollable.push(Element::from(
                Container::new(clip.view(clips, index, len))
                    .style(style::BorderContainer)
                    .width(iced::Length::Fill)
                    .max_height(CLIP_HEIGHT)
                    .padding(10)
                    .center_x(),
            ));
        }

        let audio_button = Button::new(
            &mut self.audio_button,
            Text::new(if self.playing { "Stop" } else { "Play All" }),
        )
        .style(if self.playing {
            style::Button::Destructive
        } else {
            style::Button::Primary
        })
        .on_press(if self.playing {
            TimelineMessage::Stop.into()
        } else {
            TimelineMessage::Play.into()
        });

        let mut export_button =
            Button::new(&mut self.export_button, Text::new("Export")).style(style::Button::Primary);
        if self.export.is_none() {
            export_button = export_button.on_press(TimelineMessage::Save.into());
        }

        let mut column = Column::new();
        if len > 0 {
            column = column.push(Text::new(format!(
                "Total duration: {}",
                humantime::format_duration(Duration::from_secs(
                    (len * (clip_duration as usize)) as u64
                ))
            )));
        }
        if self.export.is_some() {
            column = column.push(
                Row::new()
                    .push(Space::with_width(Length::FillPortion(1)))
                    .push(
                        ProgressBar::new(0.0..=self.progress_max, self.progress)
                            .width(Length::FillPortion(1)),
                    )
                    .push(Space::with_width(Length::FillPortion(1))),
            )
        }

        column = column
            .push(
                Row::new()
                    .spacing(10)
                    .align_items(iced::Align::Center)
                    .push(
                        Button::new(&mut self.start_button, Text::new("Add Clip at Start"))
                            .style(style::Button::Primary)
                            .on_press(TimelineMessage::AddStart.into()),
                    )
                    .push(audio_button)
                    .push(export_button),
            )
            .push(scrollable)
            .push(
                Button::new(&mut self.end_button, Text::new("Add Clip at End"))
                    .style(style::Button::Primary)
                    .on_press(TimelineMessage::AddEnd.into()),
            )
            .align_items(iced::Align::Center)
            .spacing(10);

        column.into()
    }
}
