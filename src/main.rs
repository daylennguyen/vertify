use anyhow::{bail, Result};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use std::io;
use std::path::PathBuf;
use vertify::update::{
    current_version, download_and_replace, fetch_latest_release, is_newer, select_asset,
};
use vertify::{convert, ConvertOptions, Fill, Target};

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
    after_help = "Examples:\n  vertify talk.mp4\n  vertify talk.mp4 --to 9:16 --fill blur\n  vertify talk.mp4 --fill color --color '#101010'\n  vertify talk.mp4 --dry-run\n  vertify update\n  vertify --completions bash > vertify.bash"
)]
struct Args {
    /// Input video file
    input: Option<PathBuf>,

    /// Output video file (defaults to "<input>_vertical.mp4" or "<input>_horizontal.mp4")
    output: Option<PathBuf>,

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

    /// x264 CRF quality (lower = better, 18-28 is sane)
    #[arg(long, default_value_t = 21)]
    crf: u32,

    /// Overwrite the output file if it exists
    #[arg(short = 'y', long)]
    overwrite: bool,

    /// Print the ffmpeg command instead of running it
    #[arg(long)]
    dry_run: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Check for and apply the latest vertify release
    ///
    /// Downloads the release archive for the current platform, extracts the
    /// vertify binary, and replaces the running executable.  Pass --check to
    /// only print whether an update is available without downloading.
    Update {
        /// Only check whether an update is available; do not download or replace
        #[arg(long)]
        check: bool,
    },
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

fn main() -> Result<()> {
    let args = Args::parse();

    // Subcommands are handled before anything else.
    if let Some(Commands::Update { check }) = args.command {
        return run_update(check);
    }

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
        crf: args.crf,
        overwrite: args.overwrite,
        dry_run: args.dry_run,
    };

    let output = convert(&opts)?;
    if !opts.dry_run {
        eprintln!("Done: {}", output.display());
    }
    Ok(())
}

fn run_update(check_only: bool) -> Result<()> {
    eprintln!("Checking for updates…");

    let info = fetch_latest_release()?;

    if !is_newer(&info.version, current_version()) {
        eprintln!("vertify {} is already up to date.", current_version());
        return Ok(());
    }

    eprintln!(
        "vertify {} is available (current: {}).",
        info.version,
        current_version()
    );

    if check_only {
        eprintln!("Run `vertify update` (without --check) to install it.");
        return Ok(());
    }

    let asset = select_asset(&info.assets).ok_or_else(|| {
        anyhow::anyhow!(
            "no release asset found for this platform — download manually from \
             https://github.com/daylennguyen/vertify/releases"
        )
    })?;

    // Interactive prompt.
    eprint!("Download and install {}? [y/N] ", asset.name);
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        eprintln!("Update cancelled.");
        return Ok(());
    }

    eprintln!("Downloading {}…", asset.name);
    download_and_replace(asset)?;
    eprintln!(
        "Updated to vertify {}. Restart vertify to apply.",
        info.version
    );
    Ok(())
}
