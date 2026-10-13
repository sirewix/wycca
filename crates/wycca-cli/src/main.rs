use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use chrono::{DateTime, Local, TimeZone};
use wycca_core::{Config, now_ms};

const USAGE: &str = "\
wycca — circular calendar

USAGE
    wycca [OPTIONS]                    write SVG to stdout

OPTIONS
    -c, --config <PATH>    config, .json or .yaml; defaults to the built-in preset
    -e, --events <PATH>    whole-day events from an .ics calendar, merged into
                           whatever the config already lists
    -a, --at <TIME>        unix millis or RFC 3339; defaults to now
    -s, --svg <PATH>       write SVG
    -p, --png <PATH>       write PNG
    -f, --feh              render a PNG to a temp file and open a feh window
    -h, --help             this text
    -V, --version

WALLPAPER
    wycca --png ~/wall.png && feh --bg-fill ~/wall.png
";

fn value(it: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    it.next().ok_or_else(|| format!("{flag} needs a value"))
}

fn path(it: &mut impl Iterator<Item = String>, flag: &str) -> Result<PathBuf, String> {
    value(it, flag).map(PathBuf::from)
}

#[derive(Default)]
struct Args {
    config: Option<PathBuf>,
    events: Option<PathBuf>,
    at: Option<String>,
    svg: Option<PathBuf>,
    png: Option<PathBuf>,
    feh: bool,
    help: bool,
    version: bool,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut args = Self::default();
        let mut it = env::args().skip(1);
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "-h" | "--help" => args.help = true,
                "-V" | "--version" => args.version = true,
                "-f" | "--feh" => args.feh = true,
                "-c" | "--config" => args.config = Some(path(&mut it, "--config")?),
                "-e" | "--events" => args.events = Some(path(&mut it, "--events")?),
                "-a" | "--at" => args.at = Some(value(&mut it, "--at")?),
                "-s" | "--svg" => args.svg = Some(path(&mut it, "--svg")?),
                "-p" | "--png" => args.png = Some(path(&mut it, "--png")?),
                other => return Err(format!("unknown argument {other:?}; try --help")),
            }
        }
        Ok(args)
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("wycca: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse()?;
    if args.help {
        print!("{USAGE}");
        return Ok(());
    }
    if args.version {
        println!("wycca {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let mut cfg = match &args.config {
        Some(path) => read_config(path)?,
        None => Config::default(),
    };

    // Solar events are global instants; the wall clock decides where they land.
    // `local_minus_utc`, not the difference of two timestamps: those are both
    // epoch seconds for the same instant, so their difference is always zero.
    cfg.utc_offset_ms
        .get_or_insert_with(|| i64::from(Local::now().offset().local_minus_utc()) * 1000);

    if let Some(path) = &args.events {
        let ics = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        cfg.events.extend(wycca_core::events::parse(&ics));
        cfg.events.sort_by_key(|e| e.date);
        cfg.events
            .dedup_by(|a, b| a.date == b.date && a.label == b.label);
    }

    let t = match &args.at {
        Some(at) => parse_time(at)?,
        None => now_ms(),
    };
    // The core does calendar maths, not timezone maths: hand it the local wall
    // clock, relabelled as UTC, and every ring reads civil time.
    let civil = Local
        .timestamp_millis_opt(t.try_into().unwrap_or(i64::MAX))
        .single()
        .and_then(|dt| u64::try_from(dt.naive_local().and_utc().timestamp_millis()).ok())
        .unwrap_or(t);
    let svg = wycca_core::render(&cfg, civil);

    if let Some(path) = &args.svg {
        fs::write(path, &svg).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    if let Some(path) = &args.png {
        write(path, &to_png(&svg, &cfg)?)?;
    }
    if args.feh {
        let path = env::temp_dir().join(format!("wycca-{t}.png"));
        write(&path, &to_png(&svg, &cfg)?)?;
        std::process::Command::new("feh")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("feh: {e}"))?;
    }
    if args.svg.is_none() && args.png.is_none() && !args.feh {
        print!("{svg}");
    }
    Ok(())
}

/// Config by extension: `.yaml`/`.yml` go to the YAML reader, everything else to
/// JSON, which is what the core and the browser speak.
#[cfg(feature = "yaml")]
fn read_config(path: &Path) -> Result<Config, String> {
    let src = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let yaml = matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("yaml" | "yml")
    );
    let parsed: Result<Config, String> = if yaml {
        serde_yml::from_str(&src).map_err(|e| e.to_string())
    } else {
        wycca_core::parse(&src).map_err(|e| e.to_string())
    };
    parsed.map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(not(feature = "yaml"))]
fn read_config(path: &Path) -> Result<Config, String> {
    let src = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    wycca_core::parse(&src).map_err(|e| format!("{}: {e}", path.display()))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn parse_time(at: &str) -> Result<u64, String> {
    if at.bytes().all(|b| b.is_ascii_digit()) {
        return at.parse().map_err(|e| format!("--at {at:?}: {e}"));
    }
    DateTime::parse_from_rfc3339(at)
        .map(|t| t.timestamp_millis().max(0) as u64)
        .map_err(|e| format!("--at {at:?}: {e}"))
}

#[cfg(feature = "png")]
fn to_png(svg: &str, cfg: &Config) -> Result<Vec<u8>, String> {
    let mut opts = resvg::usvg::Options::default();
    opts.fontdb_mut().load_system_fonts();
    // A nix shell has no fontconfig paths worth scanning, so honour an explicit file.
    let font = cfg
        .font
        .path
        .clone()
        .or_else(|| env::var_os("WYCCA_FONT").map(PathBuf::from));
    if let Some(path) = font {
        opts.fontdb_mut()
            .load_font_file(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    let tree = resvg::usvg::Tree::from_data(svg.as_bytes(), &opts).map_err(|e| e.to_string())?;
    let size = tree.size().to_int_size();
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(size.width(), size.height()).ok_or_else(|| {
            format!(
                "{}x{} is too large to rasterise",
                size.width(),
                size.height()
            )
        })?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(|e| e.to_string())
}

#[cfg(not(feature = "png"))]
fn to_png(_svg: &str, _cfg: &Config) -> Result<Vec<u8>, String> {
    Err("built without the `png` feature".into())
}
