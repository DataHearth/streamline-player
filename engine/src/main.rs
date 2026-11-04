use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use engine::config::{Config, HwAccel};
use engine::hw::{self, Backend};
use engine::jobs::Jobs;
use engine::session::Sessions;
use engine::{AppState, app, telemetry};
use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{MetricExporter, SpanExporter};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

const SERVICE_NAME: &str = "streamline-engine";

#[derive(Parser)]
#[command(version, about = "Streaming engine for Streamline")]
struct Cli {
    #[arg(long)]
    config: PathBuf,
}

struct Otel {
    tracer: SdkTracerProvider,
    meter: SdkMeterProvider,
}

impl Otel {
    fn shutdown(self) {
        let _ = self.tracer.shutdown();
        let _ = self.meter.shutdown();
    }
}

fn init_otel() -> anyhow::Result<Otel> {
    let resource = Resource::builder().with_service_name(SERVICE_NAME).build();
    let tracer = SdkTracerProvider::builder()
        .with_resource(resource.clone())
        .with_batch_exporter(SpanExporter::builder().with_tonic().build()?)
        .build();
    let meter = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_periodic_exporter(MetricExporter::builder().with_tonic().build()?)
        .build();
    global::set_meter_provider(meter.clone());
    Ok(Otel { tracer, meter })
}

fn init_tracing() -> Option<Otel> {
    let filter = || EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt = tracing_subscriber::fmt::layer().with_filter(filter());
    let registry = tracing_subscriber::registry().with(fmt);

    if std::env::var_os("OTEL_EXPORTER_OTLP_ENDPOINT").is_none() {
        registry.init();
        return None;
    }
    match init_otel() {
        Ok(otel) => {
            let layer = tracing_opentelemetry::layer()
                .with_tracer(otel.tracer.tracer(SERVICE_NAME))
                .with_filter(filter());
            registry.with(layer).init();
            Some(otel)
        }
        Err(e) => {
            registry.init();
            tracing::error!("OTLP export disabled: {e:#}");
            None
        }
    }
}

async fn serve(cfg: Config, secret: Vec<u8>) -> anyhow::Result<()> {
    let cfg = Arc::new(cfg);
    let ffmpeg_version = hw::ffmpeg_version(&cfg.ffmpeg()).await;
    match &ffmpeg_version {
        Some(v) => tracing::info!(version = %v, "ffmpeg found"),
        None => tracing::error!(
            path = %cfg.ffmpeg().display(),
            "ffmpeg not found; serving anyway, session starts will answer ffmpeg_unavailable"
        ),
    }
    let hw = hw::probe(&cfg).await;
    if ffmpeg_version.is_some() && cfg.hw_accel != HwAccel::Auto && hw.backend == Backend::Cpu {
        let none = !(hw.encoders.h264 || hw.encoders.hevc || hw.encoders.av1);
        if none && cfg.hw_accel != HwAccel::None {
            tracing::warn!(
                hw_accel = ?cfg.hw_accel,
                "no working encoder on the requested backend; transcodes answer transcode_unavailable"
            );
        }
    }
    tracing::info!(backend = ?hw.backend, libplacebo = hw.libplacebo, "hardware probed");

    let hw = Arc::new(hw);
    let sessions = Sessions::new(cfg.clone(), hw.clone());
    let jobs = Jobs::new(cfg.clone());
    sessions.sweep_tmp();
    jobs.sweep_boot();
    sessions.spawn_reaper();
    jobs.spawn_ttl_sweeper();
    telemetry::observe_sessions(&sessions);

    let state = AppState {
        cfg: cfg.clone(),
        secret: Arc::new(secret),
        hw,
        ffmpeg_version,
        sessions: sessions.clone(),
        jobs,
    };
    let listener = tokio::net::TcpListener::bind(cfg.listen)
        .await
        .with_context(|| format!("binding {}", cfg.listen))?;
    tracing::info!(listen = %cfg.listen, "serving");
    axum::serve(listener, app(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("shutting down");
        })
        .await?;
    sessions.shutdown().await;
    Ok(())
}

fn run(cli: Cli) -> anyhow::Result<()> {
    let cfg = Config::load(&cli.config)?;
    let secret = cfg.secret_bytes()?;
    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(async {
        let otel = init_tracing();
        let result = serve(cfg, secret).await;
        if let Some(otel) = otel {
            otel.shutdown();
        }
        result
    });
    runtime.shutdown_background();
    result
}

fn main() {
    if let Err(e) = run(Cli::parse()) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
