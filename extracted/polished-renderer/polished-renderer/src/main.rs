use std::path::PathBuf;

use clap::{Args, CommandFactory, Parser, Subcommand};
use polished_renderer::{
    bench_proxy_random_access, run, MotionBlurConfig, MotionBlurQuality, ProxyMode,
    ProxyRandomAccessBenchConfig, RenderConfig,
};

#[derive(Parser, Debug)]
#[command(author, version, about = "High-performance recording renderer")]
struct CliArgs {
    #[command(subcommand)]
    command: Option<Command>,

    #[arg(long)]
    session_dir: Option<PathBuf>,
    #[arg(long)]
    plan: Option<PathBuf>,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long)]
    output_width: Option<u32>,
    #[arg(long, default_value = "auto", value_enum)]
    proxy: ProxyMode,
    #[arg(long, default_value_t = false)]
    realtime: bool,

    #[arg(long, default_value_t = 360.0)]
    motion_blur_shutter_angle: f64,
    #[arg(long, default_value_t = 1.0)]
    motion_blur_max_blur_fraction: f64,
    #[arg(long, default_value_t = 0.4)]
    motion_blur_cursor_reduction: f64,
    #[arg(long, default_value_t = 0.001)]
    motion_blur_velocity_threshold: f64,
    #[arg(long, default_value = "high", value_enum)]
    motion_blur_quality: MotionBlurQuality,

    /// If set, writes a JSON render metrics file to this path.
    #[arg(long)]
    metrics_json: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Command {
    Bench {
        #[command(subcommand)]
        command: BenchCommand,
    },
}

#[derive(Subcommand, Debug)]
enum BenchCommand {
    /// Decode N random timestamps/sec from a proxy video.
    ProxyRandomAccess(ProxyRandomAccessArgs),
}

#[derive(Args, Debug)]
struct ProxyRandomAccessArgs {
    /// Path to the proxy video file to benchmark.
    #[arg(long)]
    input: PathBuf,

    /// Output width to decode to (preserves aspect, clamped to source width).
    #[arg(long)]
    output_width: Option<u32>,

    /// Number of random timestamps to decode (measured).
    #[arg(long, default_value_t = 200)]
    samples: usize,

    /// Number of warmup decodes (not measured).
    #[arg(long, default_value_t = 5)]
    warmup: usize,

    /// RNG seed for deterministic timestamp selection.
    #[arg(long, default_value_t = 1)]
    seed: u64,

    /// Output JSON only (no human-readable summary).
    #[arg(long, default_value_t = false)]
    json: bool,
}

fn main() {
    let args = CliArgs::parse();

    match args.command {
        Some(Command::Bench {
            command: BenchCommand::ProxyRandomAccess(bench_args),
        }) => {
            let result = match bench_proxy_random_access(ProxyRandomAccessBenchConfig {
                input_path: bench_args.input,
                output_width: bench_args.output_width,
                samples: bench_args.samples,
                warmup: bench_args.warmup,
                seed: bench_args.seed,
            }) {
                Ok(result) => result,
                Err(error) => {
                    eprintln!("bench failed: {error}");
                    std::process::exit(1);
                }
            };

            if !bench_args.json {
                eprintln!(
                    "proxy random access: {:.1} seeks/s (p50={:.2}ms p95={:.2}ms)",
                    result.seeks_per_second, result.stats_ms.p50, result.stats_ms.p95
                );
            }
            match serde_json::to_string_pretty(&result) {
                Ok(json) => println!("{json}"),
                Err(err) => {
                    eprintln!("failed to serialize JSON: {err}");
                    std::process::exit(1);
                }
            }
        }
        None => {
            let session_dir = match args.session_dir {
                Some(v) => v,
                None => CliArgs::command()
                    .error(
                        clap::error::ErrorKind::MissingRequiredArgument,
                        "--session-dir is required",
                    )
                    .exit(),
            };
            let output_path = match args.output {
                Some(v) => v,
                None => CliArgs::command()
                    .error(
                        clap::error::ErrorKind::MissingRequiredArgument,
                        "--output is required",
                    )
                    .exit(),
            };

            let config = RenderConfig {
                session_dir,
                plan_path: args.plan,
                output_path,
                output_width: args.output_width,
                proxy_mode: args.proxy,
                realtime: args.realtime,
                metrics_json: args.metrics_json,
                motion_blur: MotionBlurConfig {
                    shutter_angle: args.motion_blur_shutter_angle,
                    max_blur_fraction: args.motion_blur_max_blur_fraction,
                    cursor_blur_reduction: args.motion_blur_cursor_reduction,
                    velocity_threshold: args.motion_blur_velocity_threshold,
                    quality: args.motion_blur_quality,
                },
            };

            if let Err(error) = run(config) {
                eprintln!("render failed: {error}");
                std::process::exit(1);
            }
        }
    }
}
