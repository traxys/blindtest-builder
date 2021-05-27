use std::{
    collections::HashMap,
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
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

pub(crate) fn export(
    path: PathBuf,
    items: Vec<&str>,
    clips: &HashMap<String, Clip>,
    countdown: &Path,
    duration: u32,
) -> Result<(), String> {
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
        .map_err(|err| err.to_string())?;

    let countdown_duration: f32 = String::from_utf8(countdown_command.stdout)
        .expect("ffprobe should give utf8")
        .trim_end()
        .parse()
        .expect("ffprobe should give a duration");
    let countdown_duration: u32 = countdown_duration as _;
    if countdown_duration > duration {
        return Err("Countdown can't be longer than the duration".into());
    }

    let loop_dur = duration - countdown_duration;
    let looping_duration: OsString = loop_dur.to_string().into();
    let clip_duration: OsString = duration.to_string().into();

    let mut ffmpeg = Command::new("ffmpeg");
    ffmpeg.arg("-i").arg(countdown);

    let mut filter = String::new();

    for (index, &item) in items.iter().enumerate() {
        let clip_data = clips.get(item).expect("clip does not exist");
        ffmpeg
            .arg("-loop")
            .arg("1")
            .arg("-t")
            .arg(&looping_duration)
            .arg("-i")
            .arg(&clip_data.image_path)
            .arg("-ss")
            .arg(clip_data.offset.as_secs().to_string())
            .arg("-t")
            .arg(&clip_duration)
            .arg("-i")
            .arg(&clip_data.music_path);

        filter += &fade_scale_stream(0, 2 * index, countdown_duration);
        filter += ";";
        filter += &fade_scale_stream(2 * index + 1, 2 * index + 1, loop_dur);
        filter += ";";
        filter += &fade_audio_stream((index + 1) * 2, index, duration);
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
        .arg("-shortest")
        .arg("-y")
        .arg(&path);

    let status = ffmpeg.status();
    println!("Done!");
    if let Err(e) = status {
        Err(e.to_string())
    } else {
        Ok(())
    }
}
