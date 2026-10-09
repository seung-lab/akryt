use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::vec::Vec;
use tempfile;

use crossbeam_channel;
use image;
use jpegxl_rs::ThreadsRunner;
use jpegxl_rs::encoder_builder;
use notify::Watcher;
use toml;

mod config;

type InFlightSet = Arc<Mutex<HashSet<PathBuf>>>;

const QUEUE_CAPACITY: usize = 1_000_000;
const CONFIG_DIR: &str = "config/";

struct PipeHandle {
    _watcher: notify::PollWatcher,
    cfg: config::Config,
    _tx: crossbeam_channel::Sender<PathBuf>,
    _rx: crossbeam_channel::Receiver<PathBuf>,
    workers: Vec<std::thread::JoinHandle<()>>,
}

fn int_to_jxl_effort(effort: u8) -> jpegxl_rs::encode::EncoderSpeed {
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
    src_path: &Path,
    dest_path: &Path,
    quality: f32,
    effort: jpegxl_rs::encode::EncoderSpeed,
    codec_threads: usize,
) -> Result<(), Box<dyn std::error::Error>> {
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

    let runner = ThreadsRunner::new(None, Some(codec_threads)).expect("failed to create runner");

    let mut encoder = encoder_builder()
        .parallel_runner(&runner)
        .speed(effort)
        .color_encoding(jpegxl_rs::encode::ColorEncoding::SrgbLuma)
        .quality(quality)
        .lossless(quality == 0.0)
        .uses_original_profile(quality == 0.0)
        .build()
        .unwrap();

    let frame = jpegxl_rs::encode::EncoderFrame::new(&raw_pixels).num_channels(1);

    let jxl_data: jpegxl_rs::encode::EncoderResult<u8> =
        encoder.encode_frame(&frame, width, height)?;

    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp = tempfile::NamedTempFile::new()?;

    std::fs::write(tmp.path(), &jxl_data.data)?;
    tmp.persist(&dest_path)?;

    Ok(())
}

fn validate_config(cfg: &config::Config) -> Result<(), Box<dyn std::error::Error>> {
    let src_path = Path::new(&cfg.pipe.source.path);

    if !src_path.exists() {
        return Err(format!("Source directory does not exist: {}", src_path.display()).into());
    }

    if !src_path.is_dir() {
        return Err(format!("Source path is not a directory: {}", src_path.display()).into());
    }

    // NB: This is only for the prototype! The real version writes to S3

    let dest_path = Path::new(&cfg.pipe.destination.path);

    if !dest_path.exists() {
        return Err(format!(
            "Destination directory does not exist: {}",
            dest_path.display()
        )
        .into());
    }

    if !dest_path.is_dir() {
        return Err(format!(
            "Destination path is not a directory: {}",
            dest_path.display()
        )
        .into());
    }

    Ok(())
}

fn process_file(
    src_path: &Path,
    src_dir: &Path,
    dest_dir: &Path,
    jxl_quality: f32,
    jxl_effort: jpegxl_rs::encode::EncoderSpeed,
    codec_threads: usize,
    in_flight_set: &InFlightSet,
) -> Result<(), Box<dyn std::error::Error>> {
    if !src_path.exists() {
        println!("not exists filename: {}", src_path.display());
        return Ok(());
    } else if src_path.extension() != Some(std::ffi::OsStr::new("bmp")) {
        println!("skipping: {}", src_path.display());
        return Ok(());
    }

    println!("processing filename: {}", src_path.display());
    println!("dest_dir: {}", dest_dir.display());

    let relative_src_path = src_path.strip_prefix(src_dir)?;
    let mut dest_path = dest_dir.join(relative_src_path);

    // This needs to be more dynamic
    // but it's okay for now.
    dest_path.set_extension("jxl");

    let t_start = std::time::Instant::now();

    match transcode(
        &src_path,
        &dest_path,
        jxl_quality,
        jxl_effort,
        codec_threads,
    ) {
        Ok(_) => {
            println!(
                "transcoded: {} to {} in {} msec",
                src_path.display(),
                dest_path.display(),
                t_start.elapsed().as_millis()
            );
            std::fs::remove_file(src_path)?;
        }
        Err(err) => {
            eprintln!("ERROR transcoding {}: {}", src_path.display(), err);
        }
    }

    let mut set = in_flight_set.lock().unwrap();
    set.remove(src_path);

    Ok(())
}

fn start_watching(
    cfg: &config::Config,
    tx: &crossbeam_channel::Sender<PathBuf>,
    in_flight_set: &InFlightSet,
) -> Result<notify::PollWatcher, Box<dyn std::error::Error>> {
    let src_path = PathBuf::from(&cfg.pipe.source.path);
    let dest_dir = PathBuf::from(&cfg.pipe.destination.path);

    println!(
        "src: {} dest: {}",
        cfg.pipe.source.path, cfg.pipe.destination.path
    );

    let notify_config = notify::Config::default().with_poll_interval(cfg.pipe.source.poll);

    let all_files = std::fs::read_dir(&src_path)?;

    let mut set = in_flight_set.lock().unwrap();
    for (i, entry) in all_files.enumerate() {
        if i >= QUEUE_CAPACITY {
            break;
        }

        let e = entry?;
        let path = e.path();
        tx.send(path.clone())?;
        set.insert(path.clone());
    }
    drop(set);

    let closure_in_flight_set = Arc::clone(in_flight_set);
    let closure_tx = tx.clone();
    let expected_size = cfg.pipe.source.expected_file_size;

    let mut watcher = notify::PollWatcher::new(
        move |res: Result<notify::Event, notify::Error>| match res {
            Ok(event) => {
                let mut closure_set = closure_in_flight_set.lock().unwrap();

                for path in &event.paths {
                    println!("considering: {}", path.display());
                    if path.extension() != Some(std::ffi::OsStr::new("bmp")) {
                        println!("not a bmp: {}", path.display());
                        continue;
                    } else if closure_set.contains(path) {
                        println!("already in paths: {}", path.display());
                        continue;
                    }

                    if let Ok(meta) = std::fs::metadata(&path) {
                        if meta.len() == expected_size {
                            closure_tx.send(path.to_path_buf());
                            closure_set.insert(path.to_path_buf());
                            println!("added: {}", path.display());
                        } else {
                            println!("incorrect file size: {}: {}", path.display(), meta.len());
                        }
                    }
                }
            }
            Err(err) => eprintln!("watch error: {:?}", err),
        },
        notify_config,
    )?;

    watcher.watch(src_path.as_path(), notify::RecursiveMode::Recursive)?;

    Ok(watcher)
}

fn start_workers_for_pipe(
    config_filename: &PathBuf,
) -> Result<PipeHandle, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string(config_filename)?;

    let cfg: config::Config = toml::from_str(&contents)?;
    // convert cfg into a parsed datastructure

    let in_flight_set: InFlightSet = Arc::new(Mutex::new(HashSet::new()));

    validate_config(&cfg)?;

    let (tx, rx) = crossbeam_channel::bounded(QUEUE_CAPACITY);

    let watcher = start_watching(&cfg, &tx, &in_flight_set)?;

    let src_dir = PathBuf::from(&cfg.pipe.source.path);
    let dest_dir = PathBuf::from(&cfg.pipe.destination.path);

    let jxl_cfg = cfg
        .pipe
        .encodings
        .iter()
        .filter_map(|r| match r {
            config::EncodingRule::Transcode(t) if t.format == "jxl" => Some(t),
            _ => None,
        })
        .next()
        .unwrap();

    let jxl_quality = jxl_cfg.quality;
    let jxl_effort = int_to_jxl_effort(jxl_cfg.effort);

    let codec_threads = cfg.pipe.threads.codec;
    let mut workers = Vec::new();
    let num_workers = cfg.pipe.threads.worker;

    println!(
        "Starting {} pipe workers with {} codec threads each.",
        num_workers, codec_threads
    );

    for t in 0..num_workers {
        let rx = rx.clone();
        let src_dir = src_dir.clone();
        let dest_dir = dest_dir.clone();
        let in_flight_set = Arc::clone(&in_flight_set);

        let worker = std::thread::spawn(move || {
            for src_path in rx.iter() {
                println!("Thread {}", t);
                if let Err(err) = process_file(
                    &src_path,
                    &src_dir,
                    &dest_dir,
                    jxl_quality,
                    jxl_effort,
                    codec_threads,
                    &in_flight_set,
                ) {
                    eprintln!("Error processing {}: {err}", src_path.display());
                }
            }
        });

        workers.push(worker);
    }

    println!(
        "akryt: polling every {} msec: {}",
        cfg.pipe.source.poll.as_millis(),
        cfg.pipe.source.path
    );

    Ok(PipeHandle {
        _watcher: watcher,
        cfg: cfg,
        _tx: tx,
        _rx: rx,
        workers: workers,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_dir = Path::new(CONFIG_DIR);

    if !config_dir.is_dir() {
        return Err(format!("config dir does not exist: {}", config_dir.display()).into());
    }

    let mut pipes: HashMap<String, PipeHandle> = HashMap::new();

    let mut entries: Vec<PathBuf> = std::fs::read_dir(config_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("toml"))
                    .unwrap_or(false)
        })
        .collect();
    entries.sort();

    for path in entries {
        println!("Read config file: {}", path.display());

        match start_workers_for_pipe(&path) {
            Ok(handle) => {
                println!("Opened pipe: {}", handle.cfg.pipe.name);
                pipes.insert(handle.cfg.pipe.name.clone(), handle);
            }
            Err(err) => {
                eprintln!("Encountered an error opening {:?}: {err}", path.file_name());
            }
        }
    }

    if pipes.is_empty() {
        return Err("no pipes started".into());
    }

    println!("akryt running: {} pipe(s)", pipes.len());

    std::thread::park();

    Ok(())
}
