use bt_save::SaveFile;
use color_eyre::eyre::{eyre, WrapErr};
use std::{
    borrow::Cow,
    ffi::OsString,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};
use structopt::StructOpt;
use tar::Header;

#[derive(StructOpt, Debug)]
enum Args {
    Archive {
        #[structopt(short = "i", long = "input")]
        save_file: PathBuf,
        #[structopt(short = "o", long = "output")]
        archive: Option<PathBuf>,
    },
    Open {
        #[structopt(short = "i", long = "input")]
        archive: PathBuf,
        #[structopt(short = "o", long = "output", default_value = "bt_archive")]
        folder: PathBuf,
    },
}

fn archive_save<I: AsRef<Path>, O: AsRef<Path>>(input: I, output: O) -> color_eyre::Result<()> {
    let mut save = bt_save::load(input)?;
    let mut tar = tar::Builder::new(File::create(output).wrap_err("could not create output file")?);

    for clip in &mut save.clips {
        let path = PathBuf::from(&clip.title);

        let mut new_music = path.clone();
        new_music.push("music");
        new_music.push(
            clip.music_path
                .file_name()
                .ok_or(eyre!("music is not a file"))?,
        );

        let mut new_image = path;
        new_image.push("image");
        new_image.push(
            clip.image_path
                .file_name()
                .ok_or(eyre!("image is not a file"))?,
        );

        tar.append_path_with_name(&clip.music_path, &new_music)
            .wrap_err("could not add music to archive")?;
        tar.append_path_with_name(&clip.image_path, &new_image)
            .wrap_err("could not add image to archive")?;

        clip.music_path = new_music;
        clip.image_path = new_image;
    }

    if let Some(countdown) = &mut save.settings.countdown {
        let mut countdown_path = PathBuf::from("countdown");
        countdown_path.push(
            countdown
                .file_name()
                .ok_or(eyre!("countdown is not a file"))?,
        );

        tar.append_path_with_name(countdown, &countdown_path)
            .wrap_err("could not add countdown to archive")?;
        *countdown = countdown_path;
    }

    let (len, save_file) = save.data().wrap_err("could not generate edited save")?;
    let mut header = Header::new_gnu();
    header.set_cksum();
    header.set_size(len as u64);
    header.set_mode(0o644);

    tar.append_data(&mut header, "save.bt", save_file)
        .wrap_err("error writing save to archive")?;

    Ok(())
}

fn load_archive<I: AsRef<Path>, O: AsRef<Path>>(input: I, output: O) -> color_eyre::Result<()> {
    let mut tar = tar::Archive::new(BufReader::new(File::open(input)?));
    tar.unpack(output.as_ref())
        .wrap_err("Could not unpack archive")?;

    let mut path = output.as_ref().to_owned();
    path.push("save.bt");

    let mut save_file = SaveFile::load(&path).wrap_err("could not load save file")?;
    let base_path = output
        .as_ref()
        .canonicalize()
        .wrap_err("could not canonicalize path")?;
    for clip in &mut save_file.clips {
        let mut music_path = base_path.clone();
        music_path.push(&clip.music_path);
        clip.music_path = music_path;

        let mut image_path = base_path.clone();
        image_path.push(&clip.image_path);
        clip.image_path = image_path;
    }

    if let Some(countdown) = &mut save_file.settings.countdown {
        let mut countdown_path = base_path.clone();
        countdown_path.push(&countdown);
        *countdown = countdown_path;
    }

    save_file.store(&path)?;

    Ok(())
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::from_args();
    match args {
        Args::Archive { save_file, archive } => {
            let archive = match &archive {
                Some(a) => Cow::Borrowed(a.as_os_str()),
                None => {
                    let mut output = PathBuf::from(save_file.file_stem().unwrap());
                    output.set_extension("bta");
                    Cow::Owned(OsString::from(output))
                }
            };
            archive_save(&save_file, &archive)
        }
        Args::Open { archive, folder } => load_archive(archive, folder),
    }
}
