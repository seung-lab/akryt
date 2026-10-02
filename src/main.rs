use std::thread;

use image;
use jpegxl_rs::encoder_builder;
use jpegxl_rs::ThreadsRunner;
use notify::Watcher;
use toml;

mod config;

fn int_to_jxl_effort(effort:u8) -> jpegxl_rs::encode::EncoderSpeed {
	use jpegxl_rs::encode::EncoderSpeed::*;
	match effort {
		1 => Lightning,
		2 => Thunder,
		3 => Falcon,
		4 => Cheetah,
		5 => Hare,
		6 => Wombat,
		7 => Squirrel,
		8 => Kitten,
		9 => Tortoise,
		10 => Glacier,
		_ => Squirrel, // usual default
	}
}

fn transcode(
	src_path: &std::path::Path,
	dest_path: &std::path::Path,
	quality: f32,
	effort: jpegxl_rs::encode::EncoderSpeed,
	codec_threads: usize,
) 
	-> Result<(), Box<dyn std::error::Error>> {

	let img = match image::open(src_path) {
        Ok(img) => img,
        Err(image::ImageError::IoError(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("skipping {}: vanished", src_path.display());
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };

	let rgba_image = img.to_luma8();
	let (width, height) = rgba_image.dimensions();
	let raw_pixels = rgba_image.into_raw(); // Vec<u8>

	println!("width {} height {} quality {}", width, height, quality);

	let runner = ThreadsRunner::new(None, Some(codec_threads))
		.expect("failed to create runner");

	let mut encoder = encoder_builder()
		.parallel_runner(&runner)
		.speed(effort)
		.color_encoding(jpegxl_rs::encode::ColorEncoding::SrgbLuma)
		.quality(quality)
		.lossless(quality == 0.0)
		.uses_original_profile(quality == 0.0)
		.build()
		.unwrap();

	let frame = jpegxl_rs::encode::EncoderFrame::new(&raw_pixels)
		.num_channels(1);

	let jxl_data: jpegxl_rs::encode::EncoderResult<u8> = encoder.encode_frame(&frame, width, height)?;

	std::fs::write(dest_path, &jxl_data.data)?;

	Ok(())
}

fn validate_config(cfg: &config::Config) -> Result<(), Box<dyn std::error::Error>> {
	let src_path = std::path::Path::new(&cfg.pipe.source.path);
	
	if !src_path.exists() {
		return Err(format!("Source directory does not exist: {}", src_path.display()).into());
	}

	if !src_path.is_dir() {
		return Err(format!("Source path is not a directory: {}", src_path.display()).into());
	}

	// NB: This is only for the prototype! The real version writes to S3

	let dest_path = std::path::Path::new(&cfg.pipe.destination.path);

	if !dest_path.exists() {
		return Err(format!("Destination directory does not exist: {}", dest_path.display()).into());
	}

	if !dest_path.is_dir() {
		return Err(format!("Destination path is not a directory: {}", dest_path.display()).into());
	}

	Ok(())
}

fn process_source_directory(
	src_path: &std::path::Path,
	dest_dir: &std::path::Path,
	jxl_effort: jpegxl_rs::encode::EncoderSpeed,
	jxl_quality: f32,
	codec_threads: usize,
) -> Result<(), Box<dyn std::error::Error>> {

	let all_files = std::fs::read_dir(src_path)?;

	for entry in all_files {
		let e = entry?;

		if !e.path().exists() {
			println!("not exists filename: {}", e.path().display());
			continue;
		}
		else if e.path().extension() != Some(std::ffi::OsStr::new("bmp")) {
			println!("skipping: {}", e.path().display());
			continue;
		}

		println!("processing filename: {}", e.path().display());

		let mut dest_path = dest_dir.join(e.file_name());

		// This needs to be more dynamic
		// but it's okay for now.
		dest_path.set_extension("jxl");

		let t_start = std::time::Instant::now();

		transcode(&e.path(), &dest_path, jxl_quality, jxl_effort, codec_threads)?;

		println!("transcoded: {} to {} in {} msec", e.file_name().display(), dest_path.display(), t_start.elapsed().as_millis());

		std::fs::remove_file(e.path())?;
	}

	Ok(())
}

fn start_watching(
	cfg: &config::Config,
) -> Result<(), Box<dyn std::error::Error>> {
	let src_path = std::path::PathBuf::from(&cfg.pipe.source.path);
	let dest_dir = std::path::PathBuf::from(&cfg.pipe.destination.path);

	let jxl_cfg = cfg.pipe.encodings.iter()
		.filter_map(|r| match r {
			config::EncodingRule::Transcode(t) if t.format == "jxl" => Some(t),
			_ => None,
		})
		.next()
		.unwrap();

	let jxl_quality = jxl_cfg.quality;
	let jxl_effort = int_to_jxl_effort(jxl_cfg.effort);

	println!("src: {} dest: {}", cfg.pipe.source.path, cfg.pipe.destination.path);

	let notify_config = notify::Config::default()
		.with_poll_interval(cfg.pipe.source.poll);

	let src_path_closure = src_path.clone();
	let dest_dir_closure = dest_dir.clone();
	let codec_threads_closure = cfg.pipe.threads.codec;

	let mut watcher = notify::PollWatcher::new(
		move |res: Result<notify::Event, notify::Error>| {
			match res {
				Ok(_event) => {
					if let Err(err) = process_source_directory(
						src_path_closure.as_path(),
						dest_dir_closure.as_path(),
						jxl_effort,
						jxl_quality,
						codec_threads_closure,
					) {
						eprintln!("watch error: {:?}", err);	
					}
				},
				Err(err) => eprintln!("watch error: {:?}", err),
			}
		},
		notify_config,
	)?;

	watcher.watch(src_path.as_path(), notify::RecursiveMode::Recursive)?;

	Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let contents = std::fs::read_to_string("config/test.toml")?;

	let cfg: config::Config = toml::from_str(&contents)?;
	// convert cfg into a parsed datastructure

	validate_config(&cfg)?;

	start_watching(&cfg)?;

	println!("akryt: polling every {} msec.", cfg.pipe.source.poll.as_millis());
	std::thread::park();
	
	Ok(())
}
