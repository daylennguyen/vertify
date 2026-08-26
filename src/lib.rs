use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

pub mod gui;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Auto,
    Vertical,
    Horizontal,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Fill {
    Blur,
    Color,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AudioMode {
    Copy,
    Aac,
    None,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Quiet,
    Error,
    Warning,
    Info,
}

impl LogLevel {
    fn as_ffmpeg(self) -> &'static str {
        match self {
            Self::Quiet => "quiet",
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ConvertOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub output_dir: Option<PathBuf>,
    pub suffix: Option<String>,
    pub to: Target,
    pub fill: Fill,
    pub size: u32,
    pub color: String,
    pub blur: u32,
    pub fast: bool,
    pub preset: Option<String>,
    pub crf: u32,
    pub overwrite: bool,
    pub dry_run: bool,
    pub ffmpeg_args: Vec<String>,
    pub audio_mode: AudioMode,
    pub audio_bitrate: String,
    pub map_metadata: bool,
    pub start: Option<String>,
    pub duration: Option<String>,
    pub loglevel: LogLevel,
    pub no_faststart: bool,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            input: PathBuf::new(),
            output: None,
            output_dir: None,
            suffix: None,
            to: Target::Auto,
            fill: Fill::Blur,
            size: 1920,
            color: "black".into(),
            blur: 40,
            fast: false,
            preset: None,
            crf: 21,
            overwrite: false,
            dry_run: false,
            ffmpeg_args: Vec::new(),
            audio_mode: AudioMode::Copy,
            audio_bitrate: "192k".into(),
            map_metadata: false,
            start: None,
            duration: None,
            loglevel: LogLevel::Warning,
            no_faststart: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProbeInfo {
    pub width: u32,
    pub height: u32,
    pub duration_secs: f64,
}

impl ProbeInfo {
    pub fn orientation(&self) -> Orientation {
        match self.width.cmp(&self.height) {
            std::cmp::Ordering::Greater => Orientation::Landscape,
            std::cmp::Ordering::Less => Orientation::Portrait,
            std::cmp::Ordering::Equal => Orientation::Square,
        }
    }

    pub fn is_square(&self) -> bool {
        self.width == self.height
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Orientation {
    Landscape,
    Portrait,
    Square,
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub target: Target,
    pub out_w: u32,
    pub out_h: u32,
    pub output: PathBuf,
    pub already_oriented: bool,
    pub filter: String,
}

pub fn ensure_tools() -> Result<()> {
    let _ = ffmpeg_bin()?;
    let _ = ffprobe_bin()?;
    Ok(())
}

pub fn probe(path: &Path) -> Result<ProbeInfo> {
    let (width, height) = probe_dimensions(path)?;
    let duration_secs = probe_duration(path).unwrap_or(0.0);
    Ok(ProbeInfo {
        width,
        height,
        duration_secs,
    })
}

pub fn resolve_target(to: Target, probe: &ProbeInfo) -> Result<Target> {
    match to {
        Target::Auto => {
            if probe.width > probe.height {
                Ok(Target::Vertical)
            } else if probe.height > probe.width {
                Ok(Target::Horizontal)
            } else {
                bail!(
                    "input is square ({}x{}); choose 9:16 or 16:9 explicitly",
                    probe.width,
                    probe.height
                );
            }
        }
        t => Ok(t),
    }
}

pub fn default_output(
    input: &Path,
    target: Target,
    output_dir: Option<&Path>,
    suffix: Option<&str>,
) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".into());
    let orientation_suffix = match target {
        Target::Vertical => "vertical",
        _ => "horizontal",
    };
    let extra = suffix.filter(|s| !s.trim().is_empty());
    let file_name = match extra {
        Some(extra) => format!("{stem}_{orientation_suffix}_{extra}.mp4"),
        None => format!("{stem}_{orientation_suffix}.mp4"),
    };
    output_dir
        .map(|dir| dir.join(&file_name))
        .unwrap_or_else(|| input.with_file_name(file_name))
}

pub fn canvas_size(size: u32, target: Target) -> (u32, u32) {
    let long = size & !1;
    let short = ((long * 9 + 8) / 16) & !1;
    match target {
        Target::Vertical => (short, long),
        Target::Horizontal => (long, short),
        Target::Auto => unreachable!("resolve target before canvas_size"),
    }
}

pub fn build_plan(opts: &ConvertOptions, probe: &ProbeInfo) -> Result<Plan> {
    let target = resolve_target(opts.to, probe)?;
    let (out_w, out_h) = canvas_size(opts.size, target);

    let already_oriented = match target {
        Target::Vertical => probe.width * 16 == probe.height * 9,
        Target::Horizontal => probe.width * 9 == probe.height * 16,
        Target::Auto => unreachable!(),
    };

    let output = opts.output.clone().unwrap_or_else(|| {
        default_output(
            &opts.input,
            target,
            opts.output_dir.as_deref(),
            opts.suffix.as_deref(),
        )
    });

    if output == opts.input {
        bail!("output path must differ from input path");
    }

    let filter = match opts.fill {
        Fill::Blur => format!(
            "[0:v]split=2[bg][fg];\
             [bg]scale={w}:{h}:force_original_aspect_ratio=increase,\
                 crop={w}:{h},\
                 boxblur=luma_radius={blur}:luma_power=2:chroma_radius={cblur}:chroma_power=2[bgb];\
             [fg]scale={w}:{h}:force_original_aspect_ratio=decrease[fgs];\
             [bgb][fgs]overlay=(W-w)/2:(H-h)/2:format=auto,format=yuv420p",
            w = out_w,
            h = out_h,
            blur = opts.blur,
            cblur = (opts.blur / 2).max(1),
        ),
        Fill::Color => format!(
            "scale={w}:{h}:force_original_aspect_ratio=decrease,\
             pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:{color},format=yuv420p",
            w = out_w,
            h = out_h,
            color = opts.color,
        ),
    };

    Ok(Plan {
        target,
        out_w,
        out_h,
        output,
        already_oriented,
        filter,
    })
}

pub fn render_command(opts: &ConvertOptions, plan: &Plan) -> Result<String> {
    let preset = selected_preset(opts);
    let mut cmd = base_command(opts, &plan.filter, preset)?;
    push_audio_args(&mut cmd, opts, opts.audio_mode);
    cmd.arg(&plan.output);
    Ok(render(&cmd))
}

pub fn convert(opts: &ConvertOptions) -> Result<PathBuf> {
    if !opts.input.is_file() {
        bail!("input file not found: {}", opts.input.display());
    }
    ensure_tools()?;

    let probe = probe(&opts.input)?;
    let plan = build_plan(opts, &probe)?;

    if plan.output.exists() && !opts.overwrite && !opts.dry_run {
        bail!(
            "output already exists: {} (enable overwrite to replace)",
            plan.output.display()
        );
    }

    if plan.already_oriented {
        eprintln!(
            "warning: input already matches {} ({}×{}); re-encoding",
            match plan.target {
                Target::Vertical => "9:16",
                Target::Horizontal => "16:9",
                Target::Auto => "auto",
            },
            plan.out_w,
            plan.out_h
        );
    }

    if opts.dry_run {
        println!("{}", render_command(opts, &plan)?);
        return Ok(plan.output);
    }

    if let Some(parent) = plan.output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create output directory: {}", parent.display())
            })?;
        }
    }

    let preset = selected_preset(opts);
    let mut cmd = base_command(opts, &plan.filter, preset)?;
    push_audio_args(&mut cmd, opts, opts.audio_mode);
    cmd.arg(&plan.output);

    let status = cmd.status().context("failed to launch ffmpeg")?;
    if !status.success() {
        if opts.audio_mode == AudioMode::Copy {
            let mut retry = base_command(opts, &plan.filter, preset)?;
            push_audio_args(&mut retry, opts, AudioMode::Aac);
            retry.arg("-y").arg(&plan.output);
            let status = retry.status().context("failed to launch ffmpeg")?;
            if !status.success() {
                bail!("ffmpeg exited with {status}");
            }
        } else {
            bail!("ffmpeg exited with {status}");
        }
    }

    Ok(plan.output)
}

/// Extract a single preview frame (PNG bytes) for the Flip Stage.
pub fn extract_preview_png(input: &Path, at_secs: f64) -> Result<Vec<u8>> {
    let ffmpeg = ffmpeg_bin()?;
    let out = Command::new(&ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-ss",
            &format!("{at_secs:.3}"),
            "-i",
        ])
        .arg(input)
        .args(["-frames:v", "1", "-f", "image2pipe", "-vcodec", "png", "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("failed to launch ffmpeg for preview")?;

    if !out.status.success() || out.stdout.is_empty() {
        // Retry from the start if the seek overshot.
        let out = Command::new(&ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-i"])
            .arg(input)
            .args(["-frames:v", "1", "-f", "image2pipe", "-vcodec", "png", "-"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("failed to launch ffmpeg for preview")?;
        if !out.status.success() || out.stdout.is_empty() {
            bail!("could not extract a preview frame");
        }
        return Ok(out.stdout);
    }
    Ok(out.stdout)
}

pub fn is_valid_fill_color(name_or_hex: &str) -> bool {
    parse_fill_color_impl(name_or_hex).is_some()
}

pub fn parse_fill_color(name_or_hex: &str) -> [u8; 3] {
    parse_fill_color_impl(name_or_hex).unwrap_or([0, 0, 0])
}

fn parse_fill_color_impl(name_or_hex: &str) -> Option<[u8; 3]> {
    let s = name_or_hex.trim().to_ascii_lowercase();
    match s.as_str() {
        "black" => Some([0, 0, 0]),
        "white" => Some([255, 255, 255]),
        "red" => Some([255, 0, 0]),
        "green" => Some([0, 128, 0]),
        "blue" => Some([0, 0, 255]),
        "gray" | "grey" => Some([128, 128, 128]),
        _ => {
            let hex = s.trim_start_matches('#');
            if hex.len() == 6 {
                if let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&hex[0..2], 16),
                    u8::from_str_radix(&hex[2..4], 16),
                    u8::from_str_radix(&hex[4..6], 16),
                ) {
                    return Some([r, g, b]);
                }
            }
            None
        }
    }
}

fn probe_dimensions(path: &Path) -> Result<(u32, u32)> {
    let out = Command::new(ffprobe_bin()?)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .context("failed to launch ffprobe")?;
    if !out.status.success() {
        bail!("ffprobe could not read {}", path.display());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.trim().split(',');
    let w: u32 = parts
        .next()
        .and_then(|s| s.trim().parse().ok())
        .context("could not parse video width")?;
    let h: u32 = parts
        .next()
        .and_then(|s| s.trim().parse().ok())
        .context("could not parse video height")?;
    Ok((w, h))
}

fn probe_duration(path: &Path) -> Result<f64> {
    let out = Command::new(ffprobe_bin()?)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .context("failed to launch ffprobe for duration")?;
    if !out.status.success() {
        bail!("ffprobe could not read duration for {}", path.display());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.trim()
        .parse::<f64>()
        .context("could not parse duration")
}

fn ffmpeg_bin() -> Result<PathBuf> {
    cached_tool(&FFMPEG, "ffmpeg")
}

fn ffprobe_bin() -> Result<PathBuf> {
    cached_tool(&FFPROBE, "ffprobe")
}

static FFMPEG: OnceLock<PathBuf> = OnceLock::new();
static FFPROBE: OnceLock<PathBuf> = OnceLock::new();

fn cached_tool(slot: &OnceLock<PathBuf>, name: &str) -> Result<PathBuf> {
    if let Some(p) = slot.get() {
        return Ok(p.clone());
    }
    let resolved = resolve_tool(name)?;
    Ok(slot.get_or_init(|| resolved).clone())
}

fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Places vertify looks for `ffmpeg` / `ffprobe`, in order:
/// `VERTIFY_FFMPEG_DIR`, next to this executable (release bundle / installer),
/// `./ffmpeg/` and `./bin/` beside the executable, then PATH.
pub fn tool_candidates(name: &str) -> Vec<PathBuf> {
    let filename = exe_name(name);
    let mut out = Vec::new();
    if let Ok(dir) = std::env::var("VERTIFY_FFMPEG_DIR") {
        out.push(PathBuf::from(dir).join(&filename));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            out.push(dir.join(&filename));
            out.push(dir.join("ffmpeg").join(&filename));
            out.push(dir.join("bin").join(&filename));
        }
    }
    out.push(PathBuf::from(filename));
    out
}

fn tool_runs(path: &Path) -> bool {
    Command::new(path)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn resolve_tool(name: &str) -> Result<PathBuf> {
    for candidate in tool_candidates(name) {
        if tool_runs(&candidate) {
            return Ok(candidate);
        }
    }
    bail!(
        "{name} not found. Official releases bundle ffmpeg next to vertify — keep those files together. \
         Developers: install ffmpeg (https://ffmpeg.org) or set VERTIFY_FFMPEG_DIR."
    )
}

fn base_command(opts: &ConvertOptions, filter: &str, preset: &str) -> Result<Command> {
    let mut cmd = Command::new(ffmpeg_bin()?);
    if let Some(start) = opts.start.as_deref() {
        cmd.args(["-ss", start]);
    }
    if let Some(duration) = opts.duration.as_deref() {
        cmd.args(["-t", duration]);
    }
    cmd.arg(if opts.overwrite { "-y" } else { "-n" })
        .args([
            "-hide_banner",
            "-loglevel",
            opts.loglevel.as_ffmpeg(),
            "-stats",
        ])
        .arg("-i")
        .arg(&opts.input)
        .args(["-vf", filter])
        .args(["-c:v", "libx264"])
        .args(["-preset", preset])
        .args(["-crf", &opts.crf.to_string()]);
    if opts.map_metadata {
        cmd.args(["-map_metadata", "0"]);
    }
    if !opts.no_faststart {
        cmd.args(["-movflags", "+faststart"]);
    }
    if !opts.ffmpeg_args.is_empty() {
        cmd.args(&opts.ffmpeg_args);
    }
    Ok(cmd)
}

fn selected_preset(opts: &ConvertOptions) -> &str {
    if let Some(preset) = opts.preset.as_deref() {
        return preset;
    }
    if opts.fast {
        "ultrafast"
    } else {
        "veryfast"
    }
}

fn push_audio_args(cmd: &mut Command, opts: &ConvertOptions, mode: AudioMode) {
    match mode {
        AudioMode::Copy => {
            cmd.args(["-c:a", "copy"]);
        }
        AudioMode::Aac => {
            cmd.args(["-c:a", "aac", "-b:a", opts.audio_bitrate.as_str()]);
        }
        AudioMode::None => {
            cmd.arg("-an");
        }
    }
}

pub fn render_json_plan(plan: &Plan, fill: Fill) -> String {
    let target = match plan.target {
        Target::Vertical => "9:16",
        Target::Horizontal => "16:9",
        Target::Auto => "auto",
    };
    let fill = match fill {
        Fill::Blur => "blur",
        Fill::Color => "color",
    };
    format!(
        "{{\"target\":\"{target}\",\"output_width\":{},\"output_height\":{},\"output_path\":\"{}\",\"fill\":\"{fill}\"}}",
        plan.out_w,
        plan.out_h,
        json_escape(&plan.output.to_string_lossy())
    )
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn render(cmd: &Command) -> String {
    let mut s = cmd.get_program().to_string_lossy().into_owned();
    for a in cmd.get_args() {
        let a = a.to_string_lossy();
        if a.contains(' ') || a.contains(';') || a.contains('[') {
            s.push_str(&format!(" '{a}'"));
        } else {
            s.push_str(&format!(" {a}"));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_size_vertical_1080p_class() {
        assert_eq!(canvas_size(1920, Target::Vertical), (1080, 1920));
    }

    #[test]
    fn canvas_size_horizontal_1080p_class() {
        assert_eq!(canvas_size(1920, Target::Horizontal), (1920, 1080));
    }

    #[test]
    fn canvas_size_keeps_even_dimensions() {
        let (w, h) = canvas_size(1921, Target::Vertical);
        assert_eq!(w % 2, 0);
        assert_eq!(h % 2, 0);
    }

    #[test]
    fn auto_target_flips_landscape_to_vertical() {
        let probe = ProbeInfo {
            width: 1920,
            height: 1080,
            duration_secs: 1.0,
        };
        assert_eq!(
            resolve_target(Target::Auto, &probe).unwrap(),
            Target::Vertical
        );
    }

    #[test]
    fn auto_target_flips_portrait_to_horizontal() {
        let probe = ProbeInfo {
            width: 1080,
            height: 1920,
            duration_secs: 1.0,
        };
        assert_eq!(
            resolve_target(Target::Auto, &probe).unwrap(),
            Target::Horizontal
        );
    }

    #[test]
    fn auto_target_rejects_square() {
        let probe = ProbeInfo {
            width: 1080,
            height: 1080,
            duration_secs: 1.0,
        };
        assert!(resolve_target(Target::Auto, &probe).is_err());
    }

    #[test]
    fn default_output_uses_orientation_suffix() {
        assert_eq!(
            default_output(Path::new("clip.mov"), Target::Vertical, None, None),
            PathBuf::from("clip_vertical.mp4")
        );
        assert_eq!(
            default_output(Path::new("clip.mov"), Target::Horizontal, None, None),
            PathBuf::from("clip_horizontal.mp4")
        );
    }

    #[test]
    fn default_output_supports_output_dir_and_custom_suffix() {
        assert_eq!(
            default_output(
                Path::new("/tmp/in/clip.mov"),
                Target::Vertical,
                Some(Path::new("/tmp/out")),
                Some("social")
            ),
            PathBuf::from("/tmp/out/clip_vertical_social.mp4")
        );
    }

    #[test]
    fn parse_named_and_hex_colors() {
        assert_eq!(parse_fill_color("white"), [255, 255, 255]);
        assert_eq!(parse_fill_color("#101010"), [16, 16, 16]);
        assert_eq!(parse_fill_color("nope"), [0, 0, 0]);
        assert!(is_valid_fill_color("gray"));
        assert!(!is_valid_fill_color("not-a-color"));
    }

    #[test]
    fn already_oriented_16x9_is_detected() {
        let opts = ConvertOptions {
            input: PathBuf::from("in.mp4"),
            to: Target::Horizontal,
            ..Default::default()
        };
        let probe = ProbeInfo {
            width: 1920,
            height: 1080,
            duration_secs: 1.0,
        };
        let plan = build_plan(&opts, &probe).unwrap();
        assert!(plan.already_oriented);
        assert_eq!((plan.out_w, plan.out_h), (1920, 1080));
    }

    #[test]
    fn build_plan_rejects_identical_output_path() {
        let opts = ConvertOptions {
            input: PathBuf::from("same.mp4"),
            output: Some(PathBuf::from("same.mp4")),
            to: Target::Vertical,
            ..Default::default()
        };
        let probe = ProbeInfo {
            width: 1920,
            height: 1080,
            duration_secs: 1.0,
        };
        assert!(build_plan(&opts, &probe).is_err());
    }

    #[test]
    fn tool_candidates_include_executable_dir() {
        let candidates = tool_candidates("ffmpeg");
        let exe_dir = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        assert!(
            candidates
                .iter()
                .any(|p| p.parent() == Some(exe_dir.as_path())),
            "expected a candidate next to the running binary, got {candidates:?}"
        );
        let expected_name = exe_name("ffmpeg");
        assert!(candidates.iter().all(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n == expected_name)
        }));
    }
}
