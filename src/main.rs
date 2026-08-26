use anyhow::{bail, Result};
use clap::{CommandFactory, Parser, ValueEnum};
use clap_complete::{generate, Shell};
use std::io;
use std::path::PathBuf;
use vertify::{
    build_plan, convert, probe, render_json_plan, AudioMode, ConvertOptions, Fill, LogLevel, Target,
};

/// Convert video between 16:9 and 9:16 without cropping any of the frame.
///
/// The whole original picture is preserved and letterboxed into the target
/// canvas, with either a blurred copy of the video or a solid color filling
/// the empty space. By default the target is auto-detected: landscape input
/// converts to 9:16, portrait input converts to 16:9.
#[derive(Parser, Debug)]
#[command(
    name = "vertify",
    version,
    about = "Convert 16:9 ↔ 9:16 video without cropping",
    arg_required_else_help = true,
    after_help = "Examples:\n  vertify talk.mp4\n  vertify talk.mp4 --to 9:16 --fill blur\n  vertify talk.mp4 --fill color --color '#101010'\n  vertify talk.mp4 --dry-run\n  vertify --completions bash > vertify.bash"
)]
struct Args {
    /// Input video file
    #[arg(required_unless_present = "completions")]
    input: Option<PathBuf>,

    /// Output video file (defaults to "<input>_vertical.mp4" or "<input>_horizontal.mp4")
    output: Option<PathBuf>,

    /// Output directory used when OUTPUT is omitted
    #[arg(long)]
    output_dir: Option<PathBuf>,

    /// Extra suffix for auto-generated output filenames
    #[arg(long)]
    suffix: Option<String>,

    /// Generate shell completions and exit (bash, zsh, fish, powershell, elvish)
    #[arg(long, value_enum, value_name = "SHELL")]
    completions: Option<Shell>,

    /// Target aspect ratio (auto = flip the input's orientation)
    #[arg(short, long, value_enum, default_value_t = CliTarget::Auto)]
    to: CliTarget,

    /// How to fill the empty space around the video
    #[arg(short, long, value_enum, default_value_t = CliFill::Blur)]
    fill: CliFill,

    /// Length of the output's long edge in pixels (1920 = 1080p-class)
    #[arg(long, default_value_t = 1920)]
    size: u32,

    /// Solid fill color (only used with --fill color), e.g. black, white, #101010
    #[arg(long, default_value = "black")]
    color: String,

    /// Blur strength (only used with --fill blur)
    #[arg(long, default_value_t = 40)]
    blur: u32,

    /// Encode as fast as possible (larger file, lower quality-per-bit)
    #[arg(long)]
    fast: bool,

    /// x264 preset (overrides --fast), e.g. veryfast, medium, slow
    #[arg(long, value_enum)]
    preset: Option<CliPreset>,

    /// x264 CRF quality (lower = better, 18-28 is sane)
    #[arg(long, default_value_t = 21)]
    crf: u32,

    /// Overwrite the output file if it exists
    #[arg(short = 'y', long)]
    overwrite: bool,

    /// Print the ffmpeg command instead of running it
    #[arg(long)]
    dry_run: bool,

    /// Extra ffmpeg argument (repeatable)
    #[arg(long = "ffmpeg-arg")]
    ffmpeg_arg: Vec<String>,

    /// Audio handling mode
    #[arg(long, value_enum, default_value_t = CliAudioMode::Copy)]
    audio_mode: CliAudioMode,

    /// AAC bitrate used by --audio-mode aac (and copy fallback), e.g. 192k
    #[arg(long, default_value = "192k")]
    audio_bitrate: String,

    /// Copy source metadata to the output
    #[arg(long)]
    map_metadata: bool,

    /// Input seek time (ffmpeg format), e.g. 00:00:03.5
    #[arg(long)]
    start: Option<String>,

    /// Output duration (ffmpeg format), e.g. 12 or 00:00:12
    #[arg(long)]
    duration: Option<String>,

    /// Print conversion plan as JSON and exit
    #[arg(long)]
    json_plan: bool,

    /// ffmpeg log level
    #[arg(long, value_enum, default_value_t = CliLogLevel::Warning)]
    loglevel: CliLogLevel,

    /// Disable +faststart on output MP4
    #[arg(long)]
    no_faststart: bool,

    /// Open output file after successful encode
    #[arg(long)]
    open: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, ValueEnum)]
enum CliTarget {
    Auto,
    #[value(name = "9:16", alias = "vertical")]
    Vertical,
    #[value(name = "16:9", alias = "horizontal")]
    Horizontal,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum CliFill {
    Blur,
    Color,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum CliAudioMode {
    Copy,
    Aac,
    None,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum CliLogLevel {
    Quiet,
    Error,
    Warning,
    Info,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum CliPreset {
    Ultrafast,
    Superfast,
    Veryfast,
    Faster,
    Fast,
    Medium,
    Slow,
    Slower,
    Veryslow,
    Placebo,
}

impl CliPreset {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ultrafast => "ultrafast",
            Self::Superfast => "superfast",
            Self::Veryfast => "veryfast",
            Self::Faster => "faster",
            Self::Fast => "fast",
            Self::Medium => "medium",
            Self::Slow => "slow",
            Self::Slower => "slower",
            Self::Veryslow => "veryslow",
            Self::Placebo => "placebo",
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    if let Some(shell) = args.completions {
        generate(shell, &mut Args::command(), "vertify", &mut io::stdout());
        return Ok(());
    }

    let Some(input) = args.input else {
        bail!("input video file is required");
    };

    let opts = ConvertOptions {
        input,
        output: args.output,
        output_dir: args.output_dir,
        suffix: args.suffix,
        to: match args.to {
            CliTarget::Auto => Target::Auto,
            CliTarget::Vertical => Target::Vertical,
            CliTarget::Horizontal => Target::Horizontal,
        },
        fill: match args.fill {
            CliFill::Blur => Fill::Blur,
            CliFill::Color => Fill::Color,
        },
        size: args.size,
        color: args.color,
        blur: args.blur,
        fast: args.fast,
        preset: args.preset.map(|p| p.as_str().to_string()),
        crf: args.crf,
        overwrite: args.overwrite,
        dry_run: args.dry_run,
        ffmpeg_args: args.ffmpeg_arg,
        audio_mode: match args.audio_mode {
            CliAudioMode::Copy => AudioMode::Copy,
            CliAudioMode::Aac => AudioMode::Aac,
            CliAudioMode::None => AudioMode::None,
        },
        audio_bitrate: args.audio_bitrate,
        map_metadata: args.map_metadata,
        start: args.start,
        duration: args.duration,
        loglevel: match args.loglevel {
            CliLogLevel::Quiet => LogLevel::Quiet,
            CliLogLevel::Error => LogLevel::Error,
            CliLogLevel::Warning => LogLevel::Warning,
            CliLogLevel::Info => LogLevel::Info,
        },
        no_faststart: args.no_faststart,
    };

    if args.json_plan {
        let probe = probe(&opts.input)?;
        let plan = build_plan(&opts, &probe)?;
        println!("{}", render_json_plan(&plan, opts.fill));
        return Ok(());
    }

    let output = convert(&opts)?;
    if !opts.dry_run {
        eprintln!("Done: {}", output.display());
        if args.open {
            let _ = open::that(&output);
        }
    }
    Ok(())
}
