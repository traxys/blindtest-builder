use bt_export::{clip_duration_command, ffmpeg_command};
use iced_futures::futures;
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{ChildStdout, Command},
};

use crate::Clip;

#[derive(Clone)]
pub(crate) struct Export {
    countdown: PathBuf,
    output: PathBuf,
    items: Vec<(Duration, PathBuf, PathBuf)>,
    duration: u32,
}

impl Export {
    pub fn new(
        output: PathBuf,
        items: &[&str],
        clips: &HashMap<String, Clip>,
        countdown: PathBuf,
        duration: u32,
    ) -> Result<Self, String> {
        let items = items
            .iter()
            .map(|&name| {
                clips
                    .get(name)
                    .ok_or_else(|| "Clip does not exist".to_string())
                    .map(|clip| {
                        (
                            clip.offset,
                            clip.music_path.clone(),
                            clip.image_path.clone(),
                        )
                    })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            countdown,
            output,
            duration,
            items,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Progress {
    Started,
    Frame(u64),
    Done,
    Error(String),
}

enum State {
    Ready(Box<Export>),
    Exporting { stdout: BufReader<ChildStdout> },
    Finished,
}

macro_rules! err_prop {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(e) => return err(e),
        }
    };
}

#[inline]
fn err(err: String) -> Option<(Progress, State)> {
    return Some((Progress::Error(err), State::Finished));
}

impl<H, I> iced_native::subscription::Recipe<H, I> for Export
where
    H: Hasher,
{
    type Output = Progress;

    fn hash(&self, state: &mut H) {
        struct Marker;
        std::any::TypeId::of::<Marker>().hash(state);

        self.output.hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: futures::stream::BoxStream<'static, I>,
    ) -> futures::stream::BoxStream<'static, Self::Output> {
        Box::pin(futures::stream::unfold(
            State::Ready(self),
            move |state| async move {
                match state {
                    State::Ready(export) => {
                        let countdown_duration = err_prop!(video_duration(&export.countdown).await);
                        if countdown_duration > export.duration {
                            return err("Countdown can't be longer than the duration".into());
                        }

                        eprintln!("Started ffmpeg");

                        let mut ffmpeg_cmd = Command::from(ffmpeg_command(
                            export.duration,
                            countdown_duration,
                            &export.countdown,
                            &export.items,
                            &export.output,
                        ));
                        let child = err_prop!(ffmpeg_cmd
                            .spawn()
                            .map_err(|err| format!("error launching ffmpeg: {}", err)));

                        let stdout = BufReader::new(child.stdout.unwrap());

                        Some((Progress::Started, State::Exporting { stdout }))
                    }
                    State::Exporting { mut stdout } => {
                        let mut line = String::new();

                        loop {
                            line.clear();
                            err_prop!(stdout.read_line(&mut line).await.map_err(|e| {
                                eprintln!("Error in ffmpeg output: {:?}", e);
                                "Unexpected error occured".into()
                            }));

                            let mut split = line.trim_end().split("=");

                            let key = err_prop!(split.next().ok_or_else(|| {
                                eprintln!("ffmpeg output not of the form key = value: {:?}", line);
                                "ffmpeg output error".into()
                            }));

                            let value = err_prop!(split.next().ok_or_else(|| {
                                eprintln!("ffmpeg output not of the form key = value: {:?}", line);
                                "ffmpeg output error".into()
                            }));

                            match key {
                                "frame" => match value.parse() {
                                    Err(e) => {
                                        eprintln!("Could not parse frame count: {:?}", e);
                                        continue;
                                    }
                                    Ok(v) => {
                                        return Some((
                                            Progress::Frame(v),
                                            State::Exporting { stdout },
                                        ))
                                    }
                                },
                                "progress" if value == "end" => {
                                    return Some((Progress::Done, State::Finished))
                                }
                                _ => continue,
                            }
                        }
                    }
                    State::Finished => None,
                }
            },
        ))
    }
}

async fn video_duration(countdown: &Path) -> Result<u32, String> {
    let mut cmd = Command::from(clip_duration_command(countdown));
    let countdown_command = cmd.output().await.map_err(|err| err.to_string())?;

    let countdown_duration: f32 = String::from_utf8(countdown_command.stdout)
        .expect("ffprobe should give utf8")
        .trim_end()
        .parse()
        .expect("ffprobe should give a duration");

    Ok(countdown_duration as _)
}
