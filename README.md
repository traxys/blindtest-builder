<a href="https://github.com/traxys/blindtest-builder/actions">
    <img alt="CI build" src="https://github.com/traxys/blindtest-builder/workflows/Rust/badge.svg" />
</a>

# Blindtest Builder
   
Requires ffmpeg!

This software allows you to create easily a sequence of clip with a countdown to guess the origin of the sound, you just need to provide the sound file and an image file for each clip, and a global countdown video.

## Tools 

There are several additionnal tools that are provided with the builder GUI:
 - bt-archive: tool to bundle a local folder and expand it on another computer
 - bt-export-cli: tool to generate the final output using ffmpeg in CLI form (it is possible from the GUI too)


## Libraries

If you want to build something to expand this tool the on-disk format is available in bt-save, and the library used to generate the export commands is available at bt-export.

### File Descriptions

#### Save File

The save file is a json file of the following schema:
```json
{
	"clips": [{
		"title": "foo",
		"image_path": "/path/to/file.img",
		"music_path": "/path/to/file.music",
		"offset": {"secs": 0, "nanos": 0}
	}],
	timeline: [null, "some title"],
	"settings": {
		"duration": 0,
		"countdown": "/some/path/or/null",
	}
}
```

#### Archive

The archive is a `tar` file, with at the root a `save.bt` file, a `countdown` folder and a `clips` folder. In the clips folder there is a sub folder for each clip with it's title, and in that a `music` and `image` folder, with the music and image in them.
