use iced_futures::futures;
use std::{
    collections::HashMap,
    ffi::OsString,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{ChildStdout, Command},
};

use crate::Clip;

fn fade_scale_stream(input: usize, output: usize, duration: u32) -> String {
    format!("[{}:v]scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2,setsar=1,fade=t=out:st={}:d=1[v{}]",input, duration - 1, output)
}

fn fade_audio_stream(input: usize, output: usize, duration: u32) -> String {
    format!(
        "[{}:a]afade=t=out:st={}:d=1[a{}]",
        input,
        duration - 1,
        output
    )
}

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

                        let stdout = BufReader::new(err_prop!(ffmpeg_command(
                            export.duration,
                            countdown_duration,
                            &export.countdown,
                            &export.items,
                            &export.output
                        )));

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
                                    println!("DONE");
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

fn ffmpeg_command(
    clip_duration: u32,
    countdown_duration: u32,
    countdown: &Path,
    items: &[(Duration, PathBuf, PathBuf)],
    output: &Path,
) -> Result<ChildStdout, String> {
    let loop_dur = clip_duration - countdown_duration;
    let looping_duration: OsString = loop_dur.to_string().into();
    let clip_duration_str: OsString = clip_duration.to_string().into();

    let mut ffmpeg = Command::new("ffmpeg");
    ffmpeg.arg("-i").arg(countdown);

    let mut filter = String::new();

    for (index, (offset, music_path, image_path)) in items.iter().enumerate() {
        ffmpeg
            .arg("-loop")
            .arg("1")
            .arg("-t")
            .arg(&looping_duration)
            .arg("-i")
            .arg(image_path)
            .arg("-ss")
            .arg(offset.as_secs().to_string())
            .arg("-t")
            .arg(&clip_duration_str)
            .arg("-i")
            .arg(music_path);

        filter += &fade_scale_stream(0, 2 * index, countdown_duration);
        filter += ";";
        filter += &fade_scale_stream(2 * index + 1, 2 * index + 1, loop_dur);
        filter += ";";
        filter += &fade_audio_stream((index + 1) * 2, index, clip_duration);
        filter += ";";
    }

    let video_streams: String = (0..items.len() * 2).map(|i| format!("[v{}]", i)).collect();
    filter += &video_streams;
    filter += &format!("concat=n={}:v=1:a=0[v];", items.len() * 2);

    let audio_streams: String = (0..items.len()).map(|i| format!("[a{}]", i)).collect();
    filter += &audio_streams;
    filter += &format!("concat=n={}:v=0:a=1[a]", items.len());

    ffmpeg
        .arg("-filter_complex")
        .arg(&filter)
        .arg("-map")
        .arg("[v]")
        .arg("-map")
        .arg("[a]")
        .arg("-v")
        .arg("error")
        .arg("-progress")
        .arg("-")
        .arg("-shortest")
        .arg("-y")
        .arg(output);

    let child = ffmpeg
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|err| format!("ffmpeg error: {}", err))?;

    Ok(child.stdout.unwrap())
}

async fn video_duration(countdown: &Path) -> Result<u32, String> {
    let countdown_command = Command::new("ffprobe")
        .arg("-i")
        .arg(countdown)
        .args(vec![
            "-show_entries",
            "format=duration",
            "-v",
            "quiet",
            "-of",
            "csv=p=0",
        ])
        .output()
        .await
        .map_err(|err| err.to_string())?;

    let countdown_duration: f32 = String::from_utf8(countdown_command.stdout)
        .expect("ffprobe should give utf8")
        .trim_end()
        .parse()
        .expect("ffprobe should give a duration");

    Ok(countdown_duration as _)
}
