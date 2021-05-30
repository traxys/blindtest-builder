use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

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

pub fn clip_duration_command(clip: &Path) -> Command {
    let mut command = Command::new("ffprobe");
    command.arg("-i").arg(clip).args(&[
        "-show_entries",
        "format=duration",
        "-v",
        "quiet",
        "-of",
        "csv=p=0",
    ]);
    command
}

pub fn ffmpeg_command(
    clip_duration: u32,
    countdown_duration: u32,
    countdown: &Path,
    items: &[(Duration, PathBuf, PathBuf)],
    output: &Path,
) -> Command {
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
        .arg(output)
        .stdout(Stdio::piped());

    ffmpeg
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
