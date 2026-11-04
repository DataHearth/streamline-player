use std::sync::LazyLock;

use opentelemetry::global;
use opentelemetry::metrics::{Counter, Histogram};

use crate::session::Sessions;

pub struct Metrics {
    pub sessions: Counter<u64>,
    pub session_duration: Histogram<f64>,
    pub segment_wait: Histogram<f64>,
    pub jobs: Counter<u64>,
    pub job_duration: Histogram<f64>,
}

/// Instruments bind to the global meter provider on first use, so touch this only after `main` has installed one.
pub fn metrics() -> &'static Metrics {
    static METRICS: LazyLock<Metrics> = LazyLock::new(|| {
        let meter = global::meter("streamline-engine");
        Metrics {
            sessions: meter.u64_counter("streamline_engine.sessions").build(),
            session_duration: meter
                .f64_histogram("streamline_engine.session_duration")
                .with_unit("s")
                .build(),
            segment_wait: meter
                .f64_histogram("streamline_engine.segment_wait")
                .with_unit("ms")
                .build(),
            jobs: meter.u64_counter("streamline_engine.jobs").build(),
            job_duration: meter
                .f64_histogram("streamline_engine.job_duration")
                .with_unit("s")
                .build(),
        }
    });
    &METRICS
}

/// `transcode_active` and `encode_speed` are read from the registry at export time instead of being tracked on every start and stop.
pub fn observe_sessions(sessions: &Sessions) {
    let meter = global::meter("streamline-engine");
    let s = sessions.clone();
    meter
        .i64_observable_up_down_counter("streamline_engine.transcode_active")
        .with_callback(move |o| {
            let n = s
                .list()
                .iter()
                .filter(|i| i.hw != "none" && i.video.action == "transcode")
                .count();
            o.observe(n as i64, &[]);
        })
        .build();
    let s = sessions.clone();
    meter
        .f64_observable_gauge("streamline_engine.encode_speed")
        .with_callback(move |o| {
            let speeds: Vec<f64> = s.list().iter().filter_map(|i| i.speed).collect();
            if !speeds.is_empty() {
                o.observe(speeds.iter().sum::<f64>() / speeds.len() as f64, &[]);
            }
        })
        .build();
}
