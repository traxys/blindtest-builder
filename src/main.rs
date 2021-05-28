use iced::{
    button, executor, image, pick_list, window, Application, Button, Clipboard, Color, Column,
    Command, Container, Element, Length, PickList, Row, Rule, Settings, Space, Subscription, Text,
};
use iced_aw::{modal, Modal};
use modals::{ModalInnerState, ModalMessage};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Source};
use std::{
    collections::HashMap, io::Cursor, iter::FromIterator, path::PathBuf, sync::Arc, time::Duration,
};

mod export;
mod modals;
mod save;
mod timeline;

type SoundSample = Box<[u8]>;

fn main() -> iced::Result {
    BlindTestBuilder::run(Settings::default())
}

use iced_wgpu as renderer;
use iced_winit as runtime;

#[derive(Clone)]
pub(crate) struct Clip {
    title: String,
    music: Arc<SoundSample>,
    image: image::Handle,
    music_path: PathBuf,
    image_path: PathBuf,
    offset: Duration,
    duration: Duration,
}

impl Clip {
    fn save(&self) -> save::ClipSave {
        save::ClipSave {
            title: self.title.clone(),
            music_path: self.music_path.clone(),
            image_path: self.image_path.clone(),
            offset: self.offset.clone(),
        }
    }

    fn load(clip: save::ClipSave) -> anyhow::Result<Self> {
        let music = Arc::new(std::fs::read(&clip.music_path)?.into_boxed_slice());

        Ok(Clip {
            title: clip.title.clone(),
            image: (&clip.image_path).into(),
            image_path: clip.image_path,
            music,
            music_path: clip.music_path,
            offset: clip.offset,
            duration: Default::default(),
        }
        .fetch_duration())
    }

    pub(crate) fn fetch_duration(self) -> Self {
        let cmd = std::process::Command::new("ffprobe")
            .arg(&self.music_path)
            .arg("-v")
            .arg("quiet")
            .arg("-show_entries")
            .arg("format=duration")
            .arg("-of")
            .arg("csv=p=0")
            .output()
            .expect("ffprobe failed");
        let dur: f64 = String::from_utf8(cmd.stdout)
            .expect("ffprobe is utf8")
            .trim_end()
            .parse()
            .expect("ffprobe is not an f64");

        Self {
            duration: Duration::from_secs_f64(dur),
            ..self
        }
    }

    fn audio(&self, duration: u32) -> Result<impl Source<Item = i16>, String> {
        Ok(Decoder::new(Cursor::new(self.music.as_ref().clone()))
            .map_err(|e| format!("Error reading music: {}", e))?
            .skip_duration(self.offset)
            .take_duration(Duration::from_secs(duration as u64)))
    }
}

struct BlindTestBuilder {
    save: button::State,
    load: button::State,

    choose_clip: pick_list::State<String>,
    choosen_clip: Option<String>,
    edit_clip: button::State,
    add_clip: button::State,
    global_settings: button::State,
    modal_state: modal::State<modals::ModalState>,

    _output_stream: OutputStream,
    stream_handle: OutputStreamHandle,

    clips: HashMap<String, Clip>,
    timeline: timeline::Timeline,

    clip_duration: u32,
    countdown: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub(crate) enum Message {
    AddClip,
    ModalCancel,
    ModalClosed,
    PickedClip(String),
    EditClip(String),
    Modal(ModalMessage),
    SaveRequest,
    SaveTo(Option<PathBuf>),
    LoadRequest,
    LoadFrom(Option<PathBuf>),
    Timeline(timeline::TimelineMessage),
    GlobalSettings,
    EditClipOffset { clip: String, new_offset: u32 },
}

impl BlindTestBuilder {
    fn new() -> Self {
        let (_output_stream, stream_handle) = OutputStream::try_default().unwrap();

        Self {
            save: Default::default(),
            load: Default::default(),
            edit_clip: Default::default(),
            add_clip: Default::default(),
            global_settings: Default::default(),
            modal_state: modal::State::new(modals::ModalState::new()),
            choose_clip: Default::default(),
            choosen_clip: None,
            clips: HashMap::new(),
            _output_stream,
            timeline: timeline::Timeline::new(&stream_handle),
            clip_duration: 30,
            stream_handle,
            countdown: None,
        }
    }

    fn modal_update(&mut self, message: ModalMessage) -> Command<Message> {
        let Self {
            ref mut modal_state,
            ref mut clip_duration,
            ref mut countdown,
            ..
        } = self;

        let (command, close) = match (message, &mut modal_state.inner_mut().inner) {
            (ModalMessage::ClipBuilder(c), ModalInnerState::ClipBuilder(cb)) => {
                cb.update(c, &mut self.clips)
            }
            (ModalMessage::ClipEditor(c), ModalInnerState::ClipEditor(ce)) => {
                ce.update(c, &self.stream_handle, self.clip_duration, &mut self.clips)
            }
            (ModalMessage::GlobalSettings(m), ModalInnerState::GlobalSettings(g)) => {
                g.update(m, |settings| {
                    *clip_duration = settings.duration;
                    *countdown = settings.countdown;
                })
            }
            (m, _s) => {
                eprintln!("Message: {:?} in invalid modal state", m);
                return Command::none();
            }
        };

        if close {
            self.modal_state.show(false);
        }

        command
    }

    fn save(&self) -> save::SaveFile {
        save::SaveFile {
            clips: self.clips.values().map(Clip::save).collect(),
            timeline: self.timeline.save(),
            settings: save::Settings {
                duration: self.clip_duration,
                countdown: self.countdown.clone(),
            },
        }
    }

    fn load(&mut self, save: save::SaveFile) {
        self.clips.clear();
        self.choosen_clip = None;
        for clip in save.clips {
            self.clips.insert(
                clip.title.clone(),
                match Clip::load(clip) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Error loading clip: {:?}", e);
                        continue;
                    }
                },
            );
        }
        self.timeline.load(save.timeline, &self.stream_handle);
        self.clip_duration = save.settings.duration;
        self.countdown = save.settings.countdown;
    }
}

async fn select_saveload(save: bool) -> Option<PathBuf> {
    let dialog = native_dialog::FileDialog::new().add_filter("blindtest save", &["bt"]);
    let res = if save {
        dialog.show_save_single_file()
    } else {
        dialog.show_open_single_file()
    };

    match res {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error getting path: {:?}", e);
            None
        }
    }
}

struct Instance<A: Application>(A);
impl<A> iced_winit::Program for Instance<A>
where
    A: Application,
{
    type Renderer = crate::renderer::Renderer;
    type Message = A::Message;
    type Clipboard = iced_winit::Clipboard;

    fn update(
        &mut self,
        message: Self::Message,
        clipboard: &mut iced_winit::Clipboard,
    ) -> Command<Self::Message> {
        self.0.update(message, clipboard)
    }

    fn view(&mut self) -> Element<'_, Self::Message> {
        self.0.view()
    }
}

impl<A> crate::runtime::Application for Instance<A>
where
    A: Application,
{
    type Flags = A::Flags;

    fn new(flags: Self::Flags) -> (Self, Command<A::Message>) {
        let (app, command) = A::new(flags);

        (Instance(app), command)
    }

    fn title(&self) -> String {
        self.0.title()
    }

    fn mode(&self) -> iced_winit::Mode {
        match self.0.mode() {
            window::Mode::Windowed => iced_winit::Mode::Windowed,
            window::Mode::Fullscreen => iced_winit::Mode::Fullscreen,
            window::Mode::Hidden => iced_winit::Mode::Hidden,
        }
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        self.0.subscription()
    }

    fn background_color(&self) -> Color {
        self.0.background_color()
    }

    fn scale_factor(&self) -> f64 {
        self.0.scale_factor()
    }

    fn should_exit(&self) -> bool {
        self.0.should_exit()
    }
}

impl Application for BlindTestBuilder {
    type Message = Message;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_: Self::Flags) -> (Self, Command<Self::Message>) {
        (Self::new(), Command::none())
    }

    fn run(settings: Settings<Self::Flags>) -> iced::Result {
        let renderer_settings = crate::renderer::Settings {
            default_font: settings.default_font,
            default_text_size: settings.default_text_size,
            antialiasing: if settings.antialiasing {
                Some(crate::renderer::settings::Antialiasing::MSAAx4)
            } else {
                None
            },
            ..crate::renderer::Settings::from_env()
        };

        let rs: iced_winit::Settings<_> = settings.into();
        let runtime_settings;
        #[cfg(not(target_os = "windows"))]
        {
            runtime_settings = rs;
        }
        #[cfg(target_os = "windows")]
        {
            let mut rs = rs;
            rs.window.platform_specific.drag_and_drop = false;
            runtime_settings = rs;
        }

        Ok(crate::runtime::application::run::<
            Instance<Self>,
            Self::Executor,
            crate::renderer::window::Compositor,
        >(runtime_settings, renderer_settings)?)
    }

    fn title(&self) -> String {
        "Blind test builder".into()
    }

    fn update(&mut self, message: Self::Message, _: &mut Clipboard) -> Command<Self::Message> {
        let Self {
            ref mut timeline,
            ref countdown,
            ref clips,
            ref clip_duration,
            ..
        } = self;

        match message {
            Message::AddClip => {
                self.modal_state.inner_mut().inner =
                    ModalInnerState::ClipBuilder(modals::ClipBuilderState::default());
                self.modal_state.show(true)
            }
            Message::ModalCancel | Message::ModalClosed => {
                self.modal_state.inner_mut().close();
                self.modal_state.show(false);
            }
            Message::Modal(m) => {
                let cmd = self.modal_update(m);
                if let Some(selected) = &self.choosen_clip {
                    if !self.clips.contains_key(selected) {
                        self.choosen_clip = None;
                    }
                }
                return cmd;
            }
            Message::PickedClip(clip) => self.choosen_clip = Some(clip),
            Message::EditClip(c) => {
                self.modal_state.inner_mut().inner = ModalInnerState::ClipEditor(
                    modals::ClipEditorState::new(c, &self.stream_handle)
                        .expect("Could not create stream"),
                );
                self.modal_state.show(true);
            }
            Message::SaveRequest => {
                return Command::perform(select_saveload(true), Message::SaveTo)
            }
            Message::SaveTo(Some(path)) => {
                if let Err(e) = save::store(path, &self.save()) {
                    eprintln!("Error saving file: {:?}", e);
                }
            }
            Message::LoadRequest => {
                return Command::perform(select_saveload(false), Message::LoadFrom)
            }
            Message::LoadFrom(Some(path)) => match save::load(path) {
                Err(e) => eprintln!("Could not load save: {:?}", e),
                Ok(save) => self.load(save),
            },
            Message::SaveTo(None) | Message::LoadFrom(None) => {}
            Message::Timeline(m) => {
                return timeline.update(
                    m,
                    &self.clips,
                    &self.stream_handle,
                    self.clip_duration,
                    |path, items| match &countdown {
                        Some(countdown) => {
                            export::export(path, items, &clips, countdown, *clip_duration)
                        }
                        None => Err("No countdown selected".into()),
                    },
                )
            }
            Message::GlobalSettings => {
                self.modal_state.inner_mut().inner = ModalInnerState::GlobalSettings(
                    modals::GlobalSettingsState::new(self.clip_duration, self.countdown.clone()),
                );
                self.modal_state.show(true)
            }
            Message::EditClipOffset { clip, new_offset } => {
                self.clips
                    .get_mut(&clip)
                    .expect("Tried to modify non existent clip")
                    .offset = Duration::from_secs(new_offset as u64);
            }
        }

        Command::none()
    }

    fn view(&mut self) -> Element<Self::Message> {
        let mut edit_clip_button =
            Button::new(&mut self.edit_clip, Text::new("Edit Clip")).style(style::Button::Primary);
        if let Some(clip) = self.choosen_clip.clone() {
            edit_clip_button = edit_clip_button.on_press(Message::EditClip(clip));
        }

        let content = Container::new(
            Column::new()
                .push(
                    Row::new()
                        .spacing(10)
                        .push(
                            Button::new(&mut self.add_clip, Text::new("Add Clip"))
                                .on_press(Message::AddClip)
                                .style(style::Button::Primary),
                        )
                        .push(
                            Container::new(
                                Row::new()
                                    .push(PickList::new(
                                        &mut self.choose_clip,
                                        Vec::from_iter(self.clips.keys().cloned()),
                                        self.choosen_clip.clone(),
                                        Message::PickedClip,
                                    ))
                                    .spacing(10)
                                    .push(edit_clip_button)
                                    .padding(5)
                                    .align_items(iced::Align::Center),
                            )
                            .style(style::BorderContainer),
                        )
                        .push(
                            Button::new(&mut self.global_settings, Text::new("Global Settings"))
                                .on_press(Message::GlobalSettings)
                                .style(style::Button::Primary),
                        )
                        .push(Space::with_width(Length::Fill))
                        .push(
                            Button::new(&mut self.load, Text::new("Load"))
                                .style(style::Button::Primary)
                                .on_press(Message::LoadRequest),
                        )
                        .push(
                            Button::new(&mut self.save, Text::new("Save"))
                                .style(style::Button::Primary)
                                .on_press(Message::SaveRequest),
                        )
                        .align_items(iced::Align::Center),
                )
                .push(Rule::horizontal(20).style(style::Rule))
                .push(self.timeline.view(&self.clips))
                .align_items(iced::Align::Center),
        )
        .padding(5)
        .height(iced::Length::Fill)
        .width(iced::Length::Fill)
        .style(style::Container);

        let clips = self.clips.clone();
        let clip_duration = self.clip_duration;
        Modal::new(&mut self.modal_state, content, move |state| {
            modals::ModalState::view(state, &clips, clip_duration)
        })
        .backdrop(Message::ModalClosed)
        .on_esc(Message::ModalClosed)
        .into()
    }
}

mod style {
    use iced::{button, container, rule, scrollable, Background, Color, Vector};

    const BACKGROUND: Color = Color::from_rgb(
        0x36 as f32 / 255.0,
        0x39 as f32 / 255.0,
        0x3F as f32 / 255.0,
    );

    const SURFACE: Color = Color::from_rgb(
        0x40 as f32 / 255.0,
        0x44 as f32 / 255.0,
        0x4B as f32 / 255.0,
    );

    const ACTIVE: Color = Color::from_rgb(
        0x72 as f32 / 255.0,
        0x89 as f32 / 255.0,
        0xDA as f32 / 255.0,
    );

    const HOVERED: Color = Color::from_rgb(
        0x67 as f32 / 255.0,
        0x7B as f32 / 255.0,
        0xC4 as f32 / 255.0,
    );

    const SCROLLBAR: Color = Color::from_rgb(
        0x2E as f32 / 255.0,
        0x33 as f32 / 255.0,
        0x38 as f32 / 255.0,
    );

    const SCROLLER: Color = Color::from_rgb(
        0x20 as f32 / 255.0,
        0x22 as f32 / 255.0,
        0x25 as f32 / 255.0,
    );

    const ACCENT: Color = Color::from_rgb(
        0x6F as f32 / 255.0,
        0xFF as f32 / 255.0,
        0xE9 as f32 / 255.0,
    );

    pub struct Rule;

    impl rule::StyleSheet for Rule {
        fn style(&self) -> rule::Style {
            rule::Style {
                color: SURFACE,
                width: 2,
                radius: 1.0,
                fill_mode: rule::FillMode::Percent(30.0),
            }
        }
    }

    pub struct Scrollable;

    impl scrollable::StyleSheet for Scrollable {
        fn active(&self) -> scrollable::Scrollbar {
            scrollable::Scrollbar {
                background: Color {
                    a: 0.8,
                    ..SCROLLBAR
                }
                .into(),
                border_radius: 2.0,
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
                scroller: scrollable::Scroller {
                    color: Color { a: 0.7, ..SCROLLER },
                    border_radius: 2.0,
                    border_width: 0.0,
                    border_color: Color::TRANSPARENT,
                },
            }
        }

        fn hovered(&self) -> scrollable::Scrollbar {
            let active = self.active();

            scrollable::Scrollbar {
                background: SCROLLBAR.into(),
                scroller: scrollable::Scroller {
                    color: SCROLLER,
                    ..active.scroller
                },
                ..active
            }
        }

        fn dragging(&self) -> scrollable::Scrollbar {
            let hovered = self.hovered();

            scrollable::Scrollbar {
                scroller: scrollable::Scroller {
                    color: ACCENT,
                    ..hovered.scroller
                },
                ..hovered
            }
        }
    }

    pub struct BorderContainer;

    impl container::StyleSheet for BorderContainer {
        fn style(&self) -> container::Style {
            container::Style {
                border_color: Color::BLACK.into(),
                border_radius: 5.0,
                border_width: 2.0,
                ..container::Style::default()
            }
        }
    }

    pub struct Container;

    impl container::StyleSheet for Container {
        fn style(&self) -> container::Style {
            container::Style {
                background: BACKGROUND.into(),
                text_color: Color::WHITE.into(),
                ..container::Style::default()
            }
        }
    }

    pub enum Button {
        Primary,
        Destructive,
    }

    impl button::StyleSheet for Button {
        fn active(&self) -> button::Style {
            let (background, text_color) = match self {
                Button::Primary => (Some(ACTIVE), Color::WHITE),
                Button::Destructive => (None, Color::from_rgb8(0xFF, 0x47, 0x47)),
            };

            button::Style {
                text_color,
                background: background.map(Background::Color),
                border_radius: 5.0,
                shadow_offset: Vector::new(0.0, 0.0),
                ..button::Style::default()
            }
        }

        fn hovered(&self) -> button::Style {
            let active = self.active();

            let background = match self {
                Button::Primary => Some(HOVERED),
                Button::Destructive => Some(Color {
                    a: 0.2,
                    ..active.text_color
                }),
            };

            button::Style {
                background: background.map(Background::Color),
                ..active
            }
        }
    }
}
