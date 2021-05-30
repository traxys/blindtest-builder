use bt_export::{clip_duration_command, ffmpeg_command};
use bt_save::SaveFile;
use color_eyre::eyre::{self, eyre, WrapErr};
use indicatif::ProgressStyle;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    path::PathBuf,
};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
struct Args {
    #[structopt(long = "save", short = "-i")]
    save_file: PathBuf,
    #[structopt(long = "output", short = "-o", default_value = "output.mp4")]
    output: PathBuf,
    #[structopt(long = "threads", short = "-t")]
    threads: Option<u64>,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::from_args();

    let save_file = SaveFile::load(args.save_file).wrap_err("could not open save file")?;
    let countdown = &save_file
        .settings
        .countdown
        .ok_or(eyre!("save file has no coutdown"))?;

    let mut cmd = clip_duration_command(countdown);
    let countdown_command = cmd.output().wrap_err("could not fetch countdown length")?;

    let countdown_duration: f32 = String::from_utf8(countdown_command.stdout)
        .expect("ffprobe should give utf8")
        .trim_end()
        .parse()
        .expect("ffprobe should give a duration");
    let countdown_duration = countdown_duration as u32;
    if countdown_duration > save_file.settings.duration {
        eyre::bail!("countdown is longer than the clip length");
    }

    let clips: HashMap<_, _> = save_file
        .clips
        .iter()
        .map(|clip| (&clip.title, clip))
        .collect();

    let items: Vec<_> = save_file
        .timeline
        .iter()
        .filter_map(|v| v.as_ref())
        .map(|name| {
            clips
                .get(name)
                .ok_or_else(|| eyre!("Clip '{}' does not exist", name))
                .map(|clip| {
                    (
                        clip.offset,
                        clip.music_path.clone(),
                        clip.image_path.clone(),
                    )
                })
        })
        .collect::<Result<_, _>>()?;

    let mut ffmpeg_cmd = ffmpeg_command(
        save_file.settings.duration,
        countdown_duration,
        countdown,
        &items,
        &args.output,
    );
    if let Some(threads) = args.threads {
        ffmpeg_cmd.arg("-threads").arg(threads.to_string());
    }

    let output = BufReader::new(
        ffmpeg_cmd
            .spawn()
            .wrap_err("Could not spawn ffmpeg")?
            .stdout
            .unwrap(),
    );

    let progress_bar = indicatif::ProgressBar::new(
        (25 * save_file.settings.duration as usize * items.len()) as u64,
    );
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("[elasped:{elapsed_precise} eta:{eta_precise}] {wide_bar} {pos:>7}/{len:7}"),
    );

    for line in output.lines() {
        let line = line.wrap_err("Error reading line")?;
        let line = line.trim_end();
        let eq = line
            .find("=")
            .ok_or(eyre!("line is not of the form key=value: {}", line))?;

        let key = &line[0..eq];
        let value = &line[(eq + 1)..];

        match key {
            "frame" => {
                let value: u64 = value
                    .parse()
                    .wrap_err(eyre!("frame was not an integer: {}", value))?;
                progress_bar.set_position(value);
            }
            "progress" => {
                if value == "end" {
                    progress_bar.finish();
                    break;
                }
            }
            _ => continue,
        };
    }

    Ok(())
}
