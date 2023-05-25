use std::{collections::HashMap, fs::File, io::Write, path::PathBuf, time::Duration};

use anyhow::{bail, Result};
use clap::{ArgAction, Parser, ValueEnum};
use clap_verbosity_flag::tracing::{InfoLevel, Verbosity};
use common::{
    sqlx::{
        get_biggest_channels,
        get_channels,
        get_oldest_channels,
        get_smallest_channels,
        get_tagless_videos,
        get_tags,
        get_unset_channels,
        get_videos,
        upsert_channel,
        upsert_video,
        Channel,
        MySql,
        MySqlPoolOptions,
        PlaylistWrapper,
        Pool,
        PoolConnection,
        Video,
    },
    youtube_dl::{get_playlist, get_single_video, get_youtube_dl_path},
};
use indicatif::ProgressBar;
use manager::Entries::{Channels, Videos};
use thirtyfour::prelude::*;
use tracing_subscriber::EnvFilter;

#[cfg(feature = "read-write")]
const DATABASE_URL: &str = dotenvy_macro::dotenv!("DATABASE_URL");

#[cfg(not(feature = "read-write"))]
const DATABASE_URL: &str = env!("DATABASE_URL");

#[derive(Clone, Debug, PartialEq, ValueEnum)]
enum Mode {
    /// Add channels to the database
    Add,

    /// Fetch videos from channels with the largest video_count
    Big,

    /// Fetch videos from channels with the smallest video_count
    Few,

    /// Generate a ProTV custom playlist text file
    Gen,

    /// Fetch videos from channels with the oldest update_at
    Old,

    /// Update channels with no playlist set
    Set,

    /// Fetch videos with no tags set
    Tag,
}

#[derive(Clone, Debug, Parser)]
#[command()]
struct Args {
    /// Select a mode of operation.
    #[arg(value_enum, short, long)]
    mode: Mode,

    /// Select a channel to fetch. (Add mode only)
    #[arg(short, long)]
    channel: Option<String>,

    /// Limit of channels/videos to fetch. (Big, Few, Old, and Tag modes only)
    #[arg(short, long, default_value_t = 1)]
    limit: u32,

    /// Output directory path for the ProTV playlist files. (Gen mode only)
    #[arg(short, long, default_value = ".")]
    output_dir: PathBuf,

    /// True = Fetch just the playlist pages, False = Fetch all videos and metadata.
    #[arg(short, long, default_value_t = true, action = ArgAction::Set)]
    flat_playlist: bool,

    /// Name of the playlist to generate from the database. (Example: General, Music, Misc, FNF, etc)
    #[arg(short, long, default_value = "General")]
    playlist: String,

    /// Chromium based browser binary path
    #[arg(long, default_value = None)]
    chromium_binary: Option<PathBuf>,

    #[command(flatten)]
    verbose: Verbosity<InfoLevel>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("sqlx=ERROR"))
        .with_max_level(args.verbose.tracing_level_filter())
        .init();

    let ytdl = get_youtube_dl_path().await?;
    let pool = MySqlPoolOptions::new().connect(DATABASE_URL).await?;

    #[cfg(not(feature = "read-write"))]
    if args.mode != Mode::Gen {
        args.mode = Mode::Gen;
        println!("This binary was compiled read-only");
        println!("You may only use playlist generation mode");
    }

    match args.mode {
        Mode::Add => add(pool, ytdl, args).await,
        Mode::Big => get(pool, ytdl, args).await,
        Mode::Few => get(pool, ytdl, args).await,
        Mode::Gen => gen(pool, ytdl, args).await,
        Mode::Old => get(pool, ytdl, args).await,
        Mode::Set => set(pool, ytdl, args).await,
        Mode::Tag => get(pool, ytdl, args).await,
    }
}

async fn add(pool: Pool<MySql>, ytdl: PathBuf, args: Args) -> Result<()> {
    let mut conn = pool.acquire().await?;
    let Some(channel) = args.channel else {
        bail!("Channel required");
    };

    try_update_channel(&mut conn, &ytdl, channel, args.flat_playlist).await?;

    Ok(())
}

async fn gen(pool: Pool<MySql>, _ytdl: PathBuf, args: Args) -> Result<()> {
    let filename = format!("{}.txt", args.playlist);
    let mut file = File::create(args.output_dir.join(filename))?;
    let mut conn = pool.acquire().await?;

    println!("Fetching channels");
    let channels = get_channels(&mut conn, args.playlist).await?;
    let pb = ProgressBar::new(channels.len() as u64);

    println!("Generating file");
    for channel in channels {
        pb.inc(1);

        let videos = get_videos(&mut conn, channel.id).await?;
        for video in videos {
            let channel_name = channel.name.clone().unwrap_or(video.channel_id);
            let mut tags = video.tags.0.unwrap_or_default();
            tags.insert(0, video.id.to_owned());

            writeln!(file, "@https://shay.loan/{}", video.id)?;
            writeln!(file, "#{}", tags.join(" "))?;
            writeln!(file, "{} - {}", channel_name, video.title)?;
            writeln!(file)?;
        }
    }

    pb.finish_with_message("Done");

    Ok(())
}

async fn get(pool: Pool<MySql>, ytdl: PathBuf, args: Args) -> Result<()> {
    let mut conn = pool.acquire().await?;

    let entries = match args.mode {
        Mode::Add => unreachable!(),
        Mode::Big => Channels(get_biggest_channels(&mut conn, args.limit).await?),
        Mode::Few => Channels(get_smallest_channels(&mut conn, args.limit).await?),
        Mode::Gen => unreachable!(),
        Mode::Old => Channels(get_oldest_channels(&mut conn, args.limit).await?),
        Mode::Set => unreachable!(),
        Mode::Tag => Videos(get_tagless_videos(&mut conn, args.limit).await?),
    };

    match entries {
        Channels(channels) => {
            for channel in channels {
                if let Err(error) =
                    try_update_channel(&mut conn, &ytdl, channel.id, args.flat_playlist).await
                {
                    println!("Error updating channel: {error}");
                };
            }
        }
        Videos(videos) => {
            for video in videos {
                if let Err(error) =
                    try_update_video(&mut conn, &ytdl, video, args.flat_playlist).await
                {
                    println!("Error updating video: {error}");
                };
            }
        }
    };

    Ok(())
}

async fn set(pool: Pool<MySql>, _ytdl: PathBuf, args: Args) -> Result<()> {
    let mut conn = pool.acquire().await?;
    let channels = get_unset_channels(&mut conn, args.limit).await?;

    if channels.is_empty() {
        println!("No channels to set");
        return Ok(());
    }

    let mut capabilities = DesiredCapabilities::chrome();
    if let Some(chromium_binary) = args.chromium_binary {
        if let Some(path) = chromium_binary.to_str() {
            capabilities.set_binary(path)?;
        }
    }
    let driver = WebDriver::new("http://localhost:9515", capabilities)
        .await
        .expect("Did you forget to start chromedriver?");

    println!("Pre-loading channels, please wait...");
    let pb = ProgressBar::new(channels.len() as u64);
    let mut channel_window_handles: HashMap<Channel, WindowHandle> = HashMap::new();
    for channel in channels {
        let window_handle = driver.new_tab().await?;
        std::thread::sleep(Duration::from_secs(1)); // This is required for some reason

        let url = format!("https://youtube.com/channel/{}/videos", channel.id);
        driver.switch_to_window(window_handle.to_owned()).await?;
        driver.goto(url).await?;

        channel_window_handles
            .entry(channel)
            .or_insert(window_handle);

        pb.inc(1);
    }

    let pb = ProgressBar::new(channel_window_handles.len() as u64);
    for (mut channel, window_handle) in channel_window_handles {
        println!("\n{}", channel.name.clone().unwrap_or(channel.id.clone()));
        driver.switch_to_window(window_handle).await?;

        let mut playlist = String::new();
        let stdin = std::io::stdin();
        stdin.read_line(&mut playlist)?;

        // Strip newline
        playlist = playlist
            .strip_suffix("\r\n")
            .or(playlist.strip_suffix('\n'))
            .unwrap_or(&playlist)
            .parse()?;

        channel.playlist = Some(playlist);

        upsert_channel(&mut conn, channel).await?;
        pb.inc(1);
    }

    driver.quit().await?;

    Ok(())
}

async fn try_update_channel(
    pool: &mut PoolConnection<MySql>,
    ytdl: &PathBuf,
    channel: String,
    flat_playlist: bool,
) -> Result<()> {
    let url = format!("https://youtube.com/channel/{channel}/videos");
    println!("Fetching {url}");

    let playlist = get_playlist(ytdl, &url, flat_playlist)?;
    let Ok(channel) = Channel::try_from(*playlist.to_owned()) else {
        bail!("Channel not found");
    };

    let videos: Vec<Video> = PlaylistWrapper::from(*playlist).into();
    for video in videos {
        println!("Video: {}", video.title);
        upsert_video(pool, video).await?;
    }

    upsert_channel(pool, channel).await?;

    Ok(())
}

async fn try_update_video(
    pool: &mut PoolConnection<MySql>,
    ytdl: &PathBuf,
    mut video: Video,
    flat_playlist: bool,
) -> Result<()> {
    let url = format!("https://youtube.com/watch?v={}", video.id);
    let single_video = get_single_video(ytdl, url, flat_playlist)?;

    video.tags = get_tags(single_video.tags, Some(vec![]));

    println!("Video: {}", video.title);
    let _ = upsert_video(pool, video).await;

    Ok(())
}
