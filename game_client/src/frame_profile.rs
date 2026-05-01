#[cfg(all(feature = "render_diagnostics", debug_assertions))]
macro_rules! frame_profile_start {
    ($started:ident) => {
        let $started = std::time::Instant::now();
    };
}

#[cfg(not(all(feature = "render_diagnostics", debug_assertions)))]
macro_rules! frame_profile_start {
    ($started:ident) => {};
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
macro_rules! frame_profile_scope {
    ($guard:ident, $profiler:expr, $($path:expr),+ $(,)?) => {
        let $guard = $crate::frame_profile::FrameScope::new(
            ($profiler).as_mut(),
            &[$($path),+],
        );
    };
}

#[cfg(not(all(feature = "render_diagnostics", debug_assertions)))]
macro_rules! frame_profile_scope {
    ($guard:ident, $profiler:expr, $($path:expr),+ $(,)?) => {};
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
macro_rules! frame_profile_elapsed {
    ($profiler:expr, $started:expr, $($path:expr),+ $(,)?) => {
        ($profiler).record_elapsed(&[$($path),+], $started);
    };
}

#[cfg(not(all(feature = "render_diagnostics", debug_assertions)))]
macro_rules! frame_profile_elapsed {
    ($profiler:expr, $started:expr, $($path:expr),+ $(,)?) => {};
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
macro_rules! frame_profile_ns {
    ($profiler:expr, $ns:expr, $($path:expr),+ $(,)?) => {
        ($profiler).record_ns(&[$($path),+], $ns);
    };
}

#[cfg(not(all(feature = "render_diagnostics", debug_assertions)))]
macro_rules! frame_profile_ns {
    ($profiler:expr, $ns:expr, $($path:expr),+ $(,)?) => {};
}

pub(crate) use frame_profile_elapsed;
pub(crate) use frame_profile_ns;
pub(crate) use frame_profile_scope;
pub(crate) use frame_profile_start;

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
mod enabled {
    use std::{borrow::Cow, collections::BTreeMap, panic::Location, time::Instant};

    use bevy::{
        app::{FixedMainScheduleOrder, MainScheduleOrder, SpawnScene},
        diagnostic::{DiagnosticPath, DiagnosticsStore, FrameTimeDiagnosticsPlugin},
        ecs::schedule::{Schedule, ScheduleLabel},
        prelude::*,
    };

    const MAIN_THREAD_NAME: &str = "main-thread";
    const GPU_RENDER_THREAD_NAME: &str = "gpu-render";
    const DIAGNOSTICS_THREAD_NAME: &str = "diagnostics";
    const GPU_SAMPLE_STATUS_CODE_PATH: &str = "render/gpu_sample_status_code";
    const RENDER_FRAME_INDEX_PATH: &str = "render/render_frame_index";
    const GPU_QUERY_FRAME_INDEX_PATH: &str = "render/gpu_query_frame_index";
    const SAMPLE_LATENCY_FRAMES_PATH: &str = "render/sample_latency_frames";
    const DEFAULT_INTERVAL_FRAMES: u64 = 60;
    const DEFAULT_MAX_DEPTH: usize = 10;
    const DEFAULT_TOP_CHILDREN: usize = 16;
    const DEFAULT_TOP_SPANS: usize = 32;

    #[derive(Debug, Clone)]
    struct ProfileSource {
        function: &'static str,
        file: &'static str,
        line: u32,
    }

    #[derive(Debug, Clone)]
    struct FrameProfileNode {
        name: String,
        source: Option<ProfileSource>,
        recorded_ns: u64,
        first_start_ns: Option<u64>,
        last_end_ns: Option<u64>,
        count: u32,
        children: BTreeMap<String, FrameProfileNode>,
    }

    impl FrameProfileNode {
        fn new(name: impl Into<String>) -> Self {
            Self {
                name: name.into(),
                source: None,
                recorded_ns: 0,
                first_start_ns: None,
                last_end_ns: None,
                count: 0,
                children: BTreeMap::new(),
            }
        }

        fn add_duration(
            &mut self,
            path: &[&str],
            source: Option<ProfileSource>,
            ns: u64,
            start_ns: Option<u64>,
        ) {
            if path.is_empty() {
                self.recorded_ns = self.recorded_ns.saturating_add(ns);
                self.count = self.count.saturating_add(1);
                if let Some(start_ns) = start_ns {
                    self.first_start_ns = Some(
                        self.first_start_ns
                            .map_or(start_ns, |value| value.min(start_ns)),
                    );
                    self.last_end_ns = Some(
                        self.last_end_ns
                            .unwrap_or(0)
                            .max(start_ns.saturating_add(ns)),
                    );
                }
                if self.source.is_none() {
                    self.source = source;
                }
                return;
            }

            let (name, rest) = path.split_first().expect("path is not empty");

            self.children
                .entry((*name).to_owned())
                .or_insert_with(|| FrameProfileNode::new(*name))
                .add_duration(rest, source, ns, start_ns);
        }

        fn child_total_ns(&self) -> u64 {
            self.children
                .values()
                .map(FrameProfileNode::display_ns)
                .fold(0u64, u64::saturating_add)
        }

        fn display_ns(&self) -> u64 {
            self.recorded_ns.max(self.child_total_ns())
        }

        fn self_ns(&self) -> u64 {
            self.recorded_ns.saturating_sub(self.child_total_ns())
        }
    }

    #[derive(Debug, Clone)]
    struct ThreadProfile {
        root: FrameProfileNode,
    }

    impl ThreadProfile {
        fn new(name: impl Into<String>) -> Self {
            let name = name.into();
            Self {
                root: FrameProfileNode::new(name),
            }
        }

        fn add_timed_duration(
            &mut self,
            path: &[&str],
            source: Option<ProfileSource>,
            ns: u64,
            start_ns: Option<u64>,
        ) {
            self.root.add_duration(path, source, ns, start_ns);
        }

        fn display_ns(&self) -> u64 {
            self.root.child_total_ns()
        }
    }

    #[derive(Debug, Clone)]
    struct FrameSpan {
        thread: String,
        path: String,
        source: Option<ProfileSource>,
        start_ns: Option<u64>,
        ns: u64,
    }

    #[derive(Debug, Clone)]
    struct FrameMarker {
        name: Cow<'static, str>,
        offset_ns: u64,
    }

    #[derive(Debug, Resource)]
    pub(crate) struct DetailedFrameProfiler {
        enabled: bool,
        frame_index: u64,
        interval_frames: u64,
        min_frame_ns: u64,
        max_depth: usize,
        top_children: usize,
        top_spans: usize,
        row_events: bool,
        text_reports: bool,
        startup_logged: bool,
        frame_started: Instant,
        schedule_starts: BTreeMap<&'static str, Instant>,
        markers: Vec<FrameMarker>,
        spans: Vec<FrameSpan>,
        threads: BTreeMap<String, ThreadProfile>,
        profiler_record_ns: u64,
        gpu_sample_status: &'static str,
        app_frame_index: Option<u64>,
        render_frame_index: Option<u64>,
        gpu_query_frame_index: Option<u64>,
        sample_latency_frames: Option<u64>,
    }

    impl Default for DetailedFrameProfiler {
        fn default() -> Self {
            Self::from_env()
        }
    }

    impl DetailedFrameProfiler {
        fn from_env() -> Self {
            let row_events = std::env::var_os("FUN_FRAME_TIME_DIAGNOSTIC_ROW_EVENTS").is_some();
            let text_reports =
                !row_events || std::env::var_os("FUN_FRAME_TIME_DIAGNOSTIC_TEXT_REPORTS").is_some();
            Self {
                enabled: std::env::var_os("FUN_FRAME_TIME_DIAGNOSTICS").is_some(),
                frame_index: 0,
                interval_frames: env_u64(
                    "FUN_FRAME_TIME_DIAGNOSTIC_INTERVAL",
                    DEFAULT_INTERVAL_FRAMES,
                )
                .max(1),
                min_frame_ns: env_u64("FUN_FRAME_TIME_DIAGNOSTIC_MIN_NS", 0),
                max_depth: env_usize("FUN_FRAME_TIME_DIAGNOSTIC_MAX_DEPTH", DEFAULT_MAX_DEPTH)
                    .max(1),
                top_children: env_usize(
                    "FUN_FRAME_TIME_DIAGNOSTIC_TOP_CHILDREN",
                    DEFAULT_TOP_CHILDREN,
                )
                .max(1),
                top_spans: env_usize("FUN_FRAME_TIME_DIAGNOSTIC_TOP_SPANS", DEFAULT_TOP_SPANS)
                    .max(1),
                row_events,
                text_reports,
                startup_logged: false,
                frame_started: Instant::now(),
                schedule_starts: BTreeMap::new(),
                markers: Vec::new(),
                spans: Vec::new(),
                threads: BTreeMap::new(),
                profiler_record_ns: 0,
                gpu_sample_status: "warming_up",
                app_frame_index: None,
                render_frame_index: None,
                gpu_query_frame_index: None,
                sample_latency_frames: None,
            }
        }

        pub(crate) fn enabled(&self) -> bool {
            self.enabled
        }

        fn begin_frame(&mut self) {
            self.frame_index = self.frame_index.saturating_add(1);
            self.frame_started = Instant::now();
            self.schedule_starts.clear();
            self.markers.clear();
            self.spans.clear();
            self.threads.clear();
            self.profiler_record_ns = 0;
            self.gpu_sample_status = "query_pending";
            self.app_frame_index = None;
            self.render_frame_index = None;
            self.gpu_query_frame_index = None;
            self.sample_latency_frames = None;
            if self.enabled && !self.startup_logged {
                self.startup_logged = true;
                game_shared::fun_diag_info!(
                    target: "fun::frame_time",
                    interval_frames = self.interval_frames,
                    min_frame_ns = self.min_frame_ns,
                    max_depth = self.max_depth,
                    top_children = self.top_children,
                    top_spans = self.top_spans,
                    row_events = self.row_events,
                    text_reports = self.text_reports,
                    "detailed frame profiler enabled"
                );
            }
        }

        fn end_frame(&mut self) {
            if !self.enabled {
                return;
            }

            let frame_ns = elapsed_ns(self.frame_started);
            let should_emit = self.frame_index % self.interval_frames == 0
                || (self.min_frame_ns > 0 && frame_ns >= self.min_frame_ns);
            if !should_emit {
                return;
            }

            let profiler_record_ns = self.profiler_record_ns;
            self.record_diagnostics_ns(&["frame_profiler", "record_ns"], profiler_record_ns);
            let summary = self.summary(frame_ns);
            let serialize_started = Instant::now();
            let report = self
                .text_reports
                .then(|| self.format_report(frame_ns, summary));
            let profiler_serialize_ns = elapsed_ns(serialize_started);
            self.record_diagnostics_ns(&["frame_profiler", "serialize_ns"], profiler_serialize_ns);
            let emit_started = Instant::now();
            if let Some(report) = report.as_deref() {
                game_shared::fun_diag_info!(
                    target: "fun::frame_time",
                    frame = self.frame_index,
                    frame_ns,
                    main_profiled_ns = summary.main_profiled_ns,
                    main_unattributed_ns = summary.main_unattributed_ns,
                    gpu_render_ns = summary.gpu_render_ns,
                    render_cpu_ns = summary.render_cpu_ns,
                    total_profiled_ns = summary.total_profiled_ns,
                    thread_count = summary.thread_count,
                    gpu_sample_status = summary.gpu_sample_status,
                    app_frame_index = ?summary.app_frame_index,
                    render_frame_index = ?summary.render_frame_index,
                    gpu_query_frame_index = ?summary.gpu_query_frame_index,
                    sample_latency_frames = ?summary.sample_latency_frames,
                    diagnostics_frame_profiler_record_ns = profiler_record_ns,
                    diagnostics_frame_profiler_serialize_ns = profiler_serialize_ns,
                    "{report}"
                );
            } else {
                game_shared::fun_diag_info!(
                    target: "fun::frame_time",
                    frame = self.frame_index,
                    frame_ns,
                    main_profiled_ns = summary.main_profiled_ns,
                    main_unattributed_ns = summary.main_unattributed_ns,
                    gpu_render_ns = summary.gpu_render_ns,
                    render_cpu_ns = summary.render_cpu_ns,
                    total_profiled_ns = summary.total_profiled_ns,
                    thread_count = summary.thread_count,
                    gpu_sample_status = summary.gpu_sample_status,
                    app_frame_index = ?summary.app_frame_index,
                    render_frame_index = ?summary.render_frame_index,
                    gpu_query_frame_index = ?summary.gpu_query_frame_index,
                    sample_latency_frames = ?summary.sample_latency_frames,
                    diagnostics_frame_profiler_record_ns = profiler_record_ns,
                    diagnostics_frame_profiler_serialize_ns = profiler_serialize_ns,
                    "frame profiler sampled frame"
                );
            }
            let profiler_emit_ns = elapsed_ns(emit_started);
            self.record_diagnostics_ns(&["frame_profiler", "emit_ns"], profiler_emit_ns);
            game_shared::fun_diag_info!(
                target: "fun::frame_time::diagnostics",
                frame = self.frame_index,
                gpu_sample_status = summary.gpu_sample_status,
                app_frame_index = ?summary.app_frame_index,
                render_frame_index = ?summary.render_frame_index,
                gpu_query_frame_index = ?summary.gpu_query_frame_index,
                sample_latency_frames = ?summary.sample_latency_frames,
                diagnostics_frame_profiler_record_ns = profiler_record_ns,
                diagnostics_frame_profiler_serialize_ns = profiler_serialize_ns,
                diagnostics_frame_profiler_emit_ns = profiler_emit_ns,
                "frame profiler diagnostics overhead"
            );

            if self.row_events {
                self.emit_row_events(frame_ns, summary);
            }
        }

        #[track_caller]
        pub(crate) fn record_elapsed(&mut self, path: &[&'static str], started: Instant) {
            let start_ns = self.offset_ns(started);
            self.record_thread_ns(
                MAIN_THREAD_NAME,
                path,
                caller_source(path),
                elapsed_ns(started),
                start_ns,
            );
        }

        #[track_caller]
        pub(crate) fn record_ns(&mut self, path: &[&'static str], ns: u64) {
            self.record_thread_ns(MAIN_THREAD_NAME, path, caller_source(path), ns, None);
        }

        fn begin_schedule(&mut self, name: &'static str) {
            if !self.enabled {
                return;
            }

            let started = Instant::now();
            self.markers.push(FrameMarker {
                name: Cow::Owned(format!("{name}:begin")),
                offset_ns: self.offset_ns(started).unwrap_or_default(),
            });
            self.schedule_starts.insert(name, started);
        }

        fn end_schedule(&mut self, name: &'static str) {
            if !self.enabled {
                return;
            }

            let ended = Instant::now();
            self.markers.push(FrameMarker {
                name: Cow::Owned(format!("{name}:end")),
                offset_ns: self.offset_ns(ended).unwrap_or_default(),
            });

            let Some(started) = self.schedule_starts.remove(name) else {
                return;
            };

            let ns = elapsed_ns(started);
            let start_ns = self.offset_ns(started);
            let schedule_path = schedule_profile_path(name);
            self.record_thread_ns(MAIN_THREAD_NAME, &schedule_path, None, ns, start_ns);
        }

        fn record_thread_ns(
            &mut self,
            thread: &str,
            path: &[&str],
            source: Option<ProfileSource>,
            ns: u64,
            start_ns: Option<u64>,
        ) {
            if !self.enabled || ns == 0 || path.is_empty() {
                return;
            }

            let record_started = Instant::now();
            self.record_thread_ns_inner(thread, path, source, ns, start_ns);
            self.profiler_record_ns = self
                .profiler_record_ns
                .saturating_add(elapsed_ns(record_started));
        }

        pub(crate) fn record_diagnostics_ns(&mut self, path: &[&'static str], ns: u64) {
            self.record_thread_ns_inner(DIAGNOSTICS_THREAD_NAME, path, None, ns, None);
        }

        fn record_thread_ns_inner(
            &mut self,
            thread: &str,
            path: &[&str],
            source: Option<ProfileSource>,
            ns: u64,
            start_ns: Option<u64>,
        ) {
            if !self.enabled || ns == 0 || path.is_empty() {
                return;
            }

            let normalized_path = normalized_thread_path(thread, path);
            let path = normalized_path.as_slice();
            self.threads
                .entry(thread.to_owned())
                .or_insert_with(|| ThreadProfile::new(thread))
                .add_timed_duration(path, source.clone(), ns, start_ns);
            self.spans.push(FrameSpan {
                thread: thread.to_owned(),
                path: path.join("/"),
                source,
                start_ns,
                ns,
            });
        }

        pub(crate) fn record_gpu_render_diagnostic(&mut self, path: &str, value_ms: f64) {
            if !self.enabled || !path.starts_with("render/") {
                return;
            }

            let Some(kind) = path.rsplit('/').next() else {
                return;
            };
            if kind != "elapsed_gpu" && kind != "elapsed_cpu" {
                return;
            }

            let ns = (value_ms * 1_000_000.0).round().max(0.0) as u64;
            if ns == 0 {
                return;
            }

            let pieces = path
                .split('/')
                .filter(|piece| !piece.is_empty() && *piece != "render" && *piece != kind)
                .collect::<Vec<_>>();
            if pieces.is_empty() {
                return;
            }
            let thread_name = if kind == "elapsed_gpu" {
                GPU_RENDER_THREAD_NAME
            } else {
                "render-cpu"
            };
            self.record_thread_ns(thread_name, &pieces, None, ns, None);
        }

        pub(crate) fn record_gpu_sample_metadata(
            &mut self,
            gpu_sample_status: &'static str,
            app_frame_index: Option<u64>,
            render_frame_index: Option<u64>,
            gpu_query_frame_index: Option<u64>,
            sample_latency_frames: Option<u64>,
        ) {
            if !self.enabled {
                return;
            }

            self.gpu_sample_status = gpu_sample_status;
            self.app_frame_index = app_frame_index;
            self.render_frame_index = render_frame_index;
            self.gpu_query_frame_index = gpu_query_frame_index;
            self.sample_latency_frames = sample_latency_frames;
        }

        fn offset_ns(&self, instant: Instant) -> Option<u64> {
            instant
                .checked_duration_since(self.frame_started)
                .map(|duration| duration.as_nanos().min(u128::from(u64::MAX)) as u64)
        }

        fn summary(&self, frame_ns: u64) -> FrameProfileSummary {
            let main_profiled_ns = self
                .threads
                .get(MAIN_THREAD_NAME)
                .map(ThreadProfile::display_ns)
                .unwrap_or(0);
            let gpu_render_ns = self
                .threads
                .get(GPU_RENDER_THREAD_NAME)
                .map(ThreadProfile::display_ns)
                .unwrap_or(0);
            let render_cpu_ns = self
                .threads
                .get("render-cpu")
                .map(ThreadProfile::display_ns)
                .unwrap_or(0);
            let total_profiled_ns = self
                .threads
                .values()
                .map(ThreadProfile::display_ns)
                .fold(0u64, u64::saturating_add);

            FrameProfileSummary {
                main_profiled_ns,
                main_unattributed_ns: frame_ns.saturating_sub(main_profiled_ns),
                gpu_render_ns,
                render_cpu_ns,
                total_profiled_ns,
                thread_count: self.threads.len(),
                gpu_sample_status: self.gpu_sample_status,
                app_frame_index: self.app_frame_index,
                render_frame_index: self.render_frame_index,
                gpu_query_frame_index: self.gpu_query_frame_index,
                sample_latency_frames: self.sample_latency_frames,
            }
        }

        fn format_report(&self, frame_ns: u64, summary: FrameProfileSummary) -> String {
            let mut output = format!("GAME FRAME {}: {} ns", self.frame_index, frame_ns);
            output.push_str(&format!(
                "\nsummary: main_profiled_ns={} main_unattributed_ns={} gpu_render_ns={} render_cpu_ns={} total_profiled_ns={} thread_count={} span_count={} marker_count={} gpu_sample_status={} app_frame_index={} render_frame_index={} gpu_query_frame_index={} sample_latency_frames={}",
                summary.main_profiled_ns,
                summary.main_unattributed_ns,
                summary.gpu_render_ns,
                summary.render_cpu_ns,
                summary.total_profiled_ns,
                summary.thread_count,
                self.spans.len(),
                self.markers.len(),
                summary.gpu_sample_status,
                format_optional_u64(summary.app_frame_index),
                format_optional_u64(summary.render_frame_index),
                format_optional_u64(summary.gpu_query_frame_index),
                format_optional_u64(summary.sample_latency_frames),
            ));

            self.append_timeline(&mut output);

            let mut threads = self.threads.iter().collect::<Vec<_>>();
            threads.sort_by(|(_, left), (_, right)| right.display_ns().cmp(&left.display_ns()));

            if !self.threads.contains_key(MAIN_THREAD_NAME) {
                output.push_str(&format!("\n{MAIN_THREAD_NAME} -"));
                append_line(
                    &mut output,
                    LineRender {
                        depth: 1,
                        name: "unattributed",
                        source: None,
                        ns: frame_ns,
                        parent_total_ns: frame_ns,
                        count: 0,
                        self_ns: None,
                        start_ns: None,
                    },
                );
            }

            for (thread_name, thread) in threads {
                let thread_total_ns = if thread_name.as_str() == MAIN_THREAD_NAME {
                    frame_ns
                } else {
                    thread.display_ns()
                };
                output.push_str(&format!("\n{thread_name} -"));
                append_children(
                    &mut output,
                    &thread.root,
                    1,
                    thread_total_ns,
                    self.max_depth,
                    self.top_children,
                );

                if thread_name.as_str() == MAIN_THREAD_NAME {
                    let unattributed = frame_ns.saturating_sub(thread.display_ns());
                    if unattributed > 0 {
                        append_line(
                            &mut output,
                            LineRender {
                                depth: 1,
                                name: "unattributed",
                                source: None,
                                ns: unattributed,
                                parent_total_ns: thread_total_ns,
                                count: 0,
                                self_ns: None,
                                start_ns: None,
                            },
                        );
                    }
                }
            }

            self.append_slow_spans(&mut output, frame_ns);

            output
        }

        fn append_timeline(&self, output: &mut String) {
            if self.markers.is_empty() {
                return;
            }

            output.push_str("\ntimeline -");
            let mut markers = self.markers.iter().collect::<Vec<_>>();
            markers.sort_by_key(|marker| marker.offset_ns);
            for marker in markers {
                output.push_str(&format!("\n@{}ns {}", marker.offset_ns, marker.name));
            }
        }

        fn append_slow_spans(&self, output: &mut String, frame_ns: u64) {
            if self.spans.is_empty() {
                return;
            }

            output.push_str("\nslow-spans inclusive -");
            let mut spans = self.spans.iter().collect::<Vec<_>>();
            spans.sort_by(|left, right| right.ns.cmp(&left.ns));
            for (rank, span) in spans.into_iter().take(self.top_spans).enumerate() {
                let pct = if frame_ns > 0 {
                    (span.ns as f64 / frame_ns as f64) * 100.0
                } else {
                    0.0
                };
                let source = span
                    .source
                    .as_ref()
                    .map(|source| {
                        format!(
                            " [fn={} @ {}:{}]",
                            source.function, source.file, source.line
                        )
                    })
                    .unwrap_or_default();
                let start = span
                    .start_ns
                    .map(|value| format!(" start_ns={value}"))
                    .unwrap_or_default();
                output.push_str(&format!(
                    "\n#{} {} {}{source} {pct:.2}% {} ns{start}",
                    rank + 1,
                    span.thread,
                    span.path,
                    span.ns
                ));
            }
        }

        fn emit_row_events(&self, frame_ns: u64, summary: FrameProfileSummary) {
            for (thread, profile) in &self.threads {
                game_shared::fun_diag_info!(
                    target: "fun::frame_time::thread",
                    frame = self.frame_index,
                    thread = %thread,
                    ns = profile.display_ns(),
                    frame_pct = percent(profile.display_ns(), frame_ns),
                    "frame profiler thread row"
                );
            }

            let mut spans = self.spans.iter().collect::<Vec<_>>();
            spans.sort_by(|left, right| right.ns.cmp(&left.ns));
            for (rank, span) in spans.into_iter().take(self.top_spans).enumerate() {
                game_shared::fun_diag_info!(
                    target: "fun::frame_time::span",
                    frame = self.frame_index,
                    rank = rank + 1,
                    thread = %span.thread,
                    path = %span.path,
                    ns = span.ns,
                    frame_pct = percent(span.ns, frame_ns),
                    start_ns = ?span.start_ns,
                    source_file = ?span.source.as_ref().map(|source| source.file),
                    source_line = ?span.source.as_ref().map(|source| source.line),
                    source_function = ?span.source.as_ref().map(|source| source.function),
                    "frame profiler slow span"
                );
            }

            game_shared::fun_diag_info!(
                target: "fun::frame_time::summary",
                frame = self.frame_index,
                frame_ns,
                main_profiled_ns = summary.main_profiled_ns,
                main_unattributed_ns = summary.main_unattributed_ns,
                gpu_render_ns = summary.gpu_render_ns,
                render_cpu_ns = summary.render_cpu_ns,
                total_profiled_ns = summary.total_profiled_ns,
                thread_count = summary.thread_count,
                gpu_sample_status = summary.gpu_sample_status,
                app_frame_index = ?summary.app_frame_index,
                render_frame_index = ?summary.render_frame_index,
                gpu_query_frame_index = ?summary.gpu_query_frame_index,
                sample_latency_frames = ?summary.sample_latency_frames,
                span_count = self.spans.len(),
                marker_count = self.markers.len(),
                "frame profiler summary row"
            );
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct FrameProfileSummary {
        main_profiled_ns: u64,
        main_unattributed_ns: u64,
        gpu_render_ns: u64,
        render_cpu_ns: u64,
        total_profiled_ns: u64,
        thread_count: usize,
        gpu_sample_status: &'static str,
        app_frame_index: Option<u64>,
        render_frame_index: Option<u64>,
        gpu_query_frame_index: Option<u64>,
        sample_latency_frames: Option<u64>,
    }

    pub(crate) struct FrameScope<'a> {
        profiler: &'a mut DetailedFrameProfiler,
        path: &'static [&'static str],
        source: Option<ProfileSource>,
        started: Instant,
    }

    impl<'a> FrameScope<'a> {
        #[track_caller]
        pub(crate) fn new(
            profiler: &'a mut DetailedFrameProfiler,
            path: &'static [&'static str],
        ) -> Self {
            Self {
                profiler,
                path,
                source: caller_source(path),
                started: Instant::now(),
            }
        }
    }

    impl Drop for FrameScope<'_> {
        fn drop(&mut self) {
            self.profiler.record_thread_ns(
                MAIN_THREAD_NAME,
                self.path,
                self.source.clone(),
                elapsed_ns(self.started),
                self.profiler.offset_ns(self.started),
            );
        }
    }

    fn append_children(
        output: &mut String,
        node: &FrameProfileNode,
        depth: usize,
        parent_total_ns: u64,
        max_depth: usize,
        top_children: usize,
    ) {
        let mut children = node.children.values().collect::<Vec<_>>();
        children.sort_by(|left, right| right.display_ns().cmp(&left.display_ns()));
        let omitted_count = children.len().saturating_sub(top_children);
        let omitted_ns = children
            .iter()
            .skip(top_children)
            .map(|node| node.display_ns())
            .fold(0u64, u64::saturating_add);

        for child in children.into_iter().take(top_children) {
            let child_ns = child.display_ns();
            append_line(
                output,
                LineRender {
                    depth,
                    name: &child.name,
                    source: child.source.as_ref(),
                    ns: child_ns,
                    parent_total_ns,
                    count: child.count,
                    self_ns: (child.self_ns() > 0 && !child.children.is_empty())
                        .then_some(child.self_ns()),
                    start_ns: child.first_start_ns,
                },
            );
            if depth < max_depth {
                append_children(
                    output,
                    child,
                    depth + 1,
                    child_ns.max(1),
                    max_depth,
                    top_children,
                );
            } else if !child.children.is_empty() {
                append_line(
                    output,
                    LineRender {
                        depth: depth + 1,
                        name: "children omitted by max depth",
                        source: None,
                        ns: child.child_total_ns(),
                        parent_total_ns: child_ns.max(1),
                        count: child.children.len() as u32,
                        self_ns: None,
                        start_ns: None,
                    },
                );
            }
        }

        if omitted_count > 0 {
            append_line(
                output,
                LineRender {
                    depth,
                    name: "children omitted by top limit",
                    source: None,
                    ns: omitted_ns,
                    parent_total_ns,
                    count: omitted_count as u32,
                    self_ns: None,
                    start_ns: None,
                },
            );
        }
    }

    struct LineRender<'a> {
        depth: usize,
        name: &'a str,
        source: Option<&'a ProfileSource>,
        ns: u64,
        parent_total_ns: u64,
        count: u32,
        self_ns: Option<u64>,
        start_ns: Option<u64>,
    }

    fn append_line(output: &mut String, line: LineRender<'_>) {
        let pct = if line.parent_total_ns > 0 {
            (line.ns as f64 / line.parent_total_ns as f64) * 100.0
        } else {
            0.0
        };
        let dashes = "-".repeat(line.depth);
        let source = line
            .source
            .map(|source| {
                format!(
                    " [fn={} @ {}:{}]",
                    source.function, source.file, source.line
                )
            })
            .unwrap_or_default();
        let self_suffix = line
            .self_ns
            .map(|self_ns| format!(" self_ns={self_ns}"))
            .unwrap_or_default();
        let start_suffix = line
            .start_ns
            .map(|start_ns| format!(" start_ns={start_ns}"))
            .unwrap_or_default();
        if line.count > 1 {
            output.push_str(&format!(
                "\n{dashes}{}{source} {pct:.2}% {} ns count={}{}{}",
                line.name, line.ns, line.count, self_suffix, start_suffix
            ));
        } else {
            output.push_str(&format!(
                "\n{dashes}{}{source} {pct:.2}% {} ns{}{}",
                line.name, line.ns, self_suffix, start_suffix
            ));
        }
    }

    fn percent(ns: u64, total_ns: u64) -> f64 {
        if total_ns > 0 {
            (ns as f64 / total_ns as f64) * 100.0
        } else {
            0.0
        }
    }

    fn format_optional_u64(value: Option<u64>) -> String {
        value
            .map(|value| value.to_string())
            .unwrap_or_else(|| "pending".to_owned())
    }

    fn diagnostic_value(diagnostics: &DiagnosticsStore, path: &'static str) -> Option<f64> {
        diagnostics
            .get(&DiagnosticPath::new(path))
            .and_then(|diagnostic| diagnostic.value().or_else(|| diagnostic.average()))
    }

    fn diagnostic_value_path(diagnostics: &DiagnosticsStore, path: &DiagnosticPath) -> Option<f64> {
        diagnostics
            .get(path)
            .and_then(|diagnostic| diagnostic.value().or_else(|| diagnostic.average()))
    }

    fn gpu_sample_status_from_code(code: f64) -> Option<&'static str> {
        match code.round() as u64 {
            1 => Some("ready"),
            2 => Some("query_pending"),
            3 => Some("warming_up"),
            4 => Some("not_rendered"),
            5 => Some("unsupported"),
            6 => Some("device_lost"),
            7 => Some("correlation_missing"),
            _ => None,
        }
    }

    fn schedule_profile_path(name: &'static str) -> Vec<&'static str> {
        if is_fixed_schedule_name(name) {
            vec!["main-schedule", "RunFixedMainLoop", name]
        } else {
            vec!["main-schedule", name]
        }
    }

    fn normalized_thread_path<'a>(thread: &str, path: &'a [&'a str]) -> Vec<&'a str> {
        if thread != MAIN_THREAD_NAME || path.first() == Some(&"main-schedule") {
            return path.to_vec();
        }

        let Some(schedule_name) = path.first().copied() else {
            return Vec::new();
        };

        if is_fixed_schedule_name(schedule_name) {
            let mut normalized = Vec::with_capacity(path.len() + 2);
            normalized.push("main-schedule");
            normalized.push("RunFixedMainLoop");
            normalized.extend_from_slice(path);
            normalized
        } else if is_main_schedule_name(schedule_name) {
            let mut normalized = Vec::with_capacity(path.len() + 1);
            normalized.push("main-schedule");
            normalized.extend_from_slice(path);
            normalized
        } else {
            path.to_vec()
        }
    }

    fn is_main_schedule_name(name: &str) -> bool {
        matches!(
            name,
            "First"
                | "PreUpdate"
                | "RunFixedMainLoop"
                | "Update"
                | "SpawnScene"
                | "PostUpdate"
                | "Last"
        )
    }

    fn is_fixed_schedule_name(name: &str) -> bool {
        matches!(
            name,
            "FixedFirst" | "FixedPreUpdate" | "FixedUpdate" | "FixedPostUpdate" | "FixedLast"
        )
    }

    fn elapsed_ns(started: Instant) -> u64 {
        started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
    }

    #[track_caller]
    fn caller_source(path: &[&'static str]) -> Option<ProfileSource> {
        let function = path.last().copied()?;
        let location = Location::caller();
        Some(ProfileSource {
            function,
            file: location.file(),
            line: location.line(),
        })
    }

    fn env_u64(name: &str, default_value: u64) -> u64 {
        let Ok(value) = std::env::var(name) else {
            return default_value;
        };
        value.parse::<u64>().unwrap_or(default_value)
    }

    fn env_usize(name: &str, default_value: usize) -> usize {
        let Ok(value) = std::env::var(name) else {
            return default_value;
        };
        value.parse::<usize>().unwrap_or(default_value)
    }

    pub(crate) fn end_detailed_frame_profile(mut profiler: ResMut<DetailedFrameProfiler>) {
        profiler.end_frame();
    }

    macro_rules! schedule_marker_labels {
        ($($before:ident, $after:ident, $name:literal;)*) => {
            $(
                #[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
                struct $before;

                #[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
                struct $after;
            )*
        };
    }

    schedule_marker_labels! {
        FrameProfileBeforeFirst, FrameProfileAfterFirst, "First";
        FrameProfileBeforePreUpdate, FrameProfileAfterPreUpdate, "PreUpdate";
        FrameProfileBeforeRunFixedMainLoop, FrameProfileAfterRunFixedMainLoop, "RunFixedMainLoop";
        FrameProfileBeforeUpdate, FrameProfileAfterUpdate, "Update";
        FrameProfileBeforeSpawnScene, FrameProfileAfterSpawnScene, "SpawnScene";
        FrameProfileBeforePostUpdate, FrameProfileAfterPostUpdate, "PostUpdate";
        FrameProfileBeforeLast, FrameProfileAfterLast, "Last";
        FrameProfileBeforeFixedFirst, FrameProfileAfterFixedFirst, "FixedFirst";
        FrameProfileBeforeFixedPreUpdate, FrameProfileAfterFixedPreUpdate, "FixedPreUpdate";
        FrameProfileBeforeFixedUpdate, FrameProfileAfterFixedUpdate, "FixedUpdate";
        FrameProfileBeforeFixedPostUpdate, FrameProfileAfterFixedPostUpdate, "FixedPostUpdate";
        FrameProfileBeforeFixedLast, FrameProfileAfterFixedLast, "FixedLast";
    }

    macro_rules! schedule_marker_systems {
        ($($begin_fn:ident, $end_fn:ident, $name:literal;)*) => {
            $(
                fn $begin_fn(mut profiler: ResMut<DetailedFrameProfiler>) {
                    profiler.begin_schedule($name);
                }

                fn $end_fn(mut profiler: ResMut<DetailedFrameProfiler>) {
                    profiler.end_schedule($name);
                }
            )*
        };
    }

    schedule_marker_systems! {
        begin_pre_update_schedule, end_pre_update_schedule, "PreUpdate";
        begin_run_fixed_main_loop_schedule, end_run_fixed_main_loop_schedule, "RunFixedMainLoop";
        begin_update_schedule, end_update_schedule, "Update";
        begin_spawn_scene_schedule, end_spawn_scene_schedule, "SpawnScene";
        begin_post_update_schedule, end_post_update_schedule, "PostUpdate";
        begin_fixed_first_schedule, end_fixed_first_schedule, "FixedFirst";
        begin_fixed_pre_update_schedule, end_fixed_pre_update_schedule, "FixedPreUpdate";
        begin_fixed_update_schedule, end_fixed_update_schedule, "FixedUpdate";
        begin_fixed_post_update_schedule, end_fixed_post_update_schedule, "FixedPostUpdate";
        begin_fixed_last_schedule, end_fixed_last_schedule, "FixedLast";
    }

    fn begin_first_schedule(mut profiler: ResMut<DetailedFrameProfiler>) {
        profiler.begin_frame();
        profiler.begin_schedule("First");
    }

    fn end_first_schedule(mut profiler: ResMut<DetailedFrameProfiler>) {
        profiler.end_schedule("First");
    }

    fn begin_last_schedule(mut profiler: ResMut<DetailedFrameProfiler>) {
        profiler.begin_schedule("Last");
    }

    fn end_last_schedule(mut profiler: ResMut<DetailedFrameProfiler>) {
        profiler.end_schedule("Last");
    }

    fn record_render_diagnostics_for_frame(
        render_diagnostics: Res<DiagnosticsStore>,
        mut profiler: ResMut<DetailedFrameProfiler>,
    ) {
        if !profiler.enabled() {
            return;
        }

        let started = Instant::now();
        let drain_started = Instant::now();
        for diagnostic in render_diagnostics.iter() {
            let path = diagnostic.path().as_str();
            let Some(value) = diagnostic.value().or_else(|| diagnostic.average()) else {
                continue;
            };
            profiler.record_gpu_render_diagnostic(path, value);
        }
        let app_frame_index = diagnostic_value_path(
            &render_diagnostics,
            &FrameTimeDiagnosticsPlugin::FRAME_COUNT,
        )
        .map(|value| value as u64);
        let render_frame_index = diagnostic_value(&render_diagnostics, RENDER_FRAME_INDEX_PATH)
            .map(|value| value as u64);
        let gpu_query_frame_index =
            diagnostic_value(&render_diagnostics, GPU_QUERY_FRAME_INDEX_PATH)
                .map(|value| value as u64);
        let sample_latency_frames =
            diagnostic_value(&render_diagnostics, SAMPLE_LATENCY_FRAMES_PATH)
                .map(|value| value as u64);
        let gpu_sample_status = diagnostic_value(&render_diagnostics, GPU_SAMPLE_STATUS_CODE_PATH)
            .and_then(gpu_sample_status_from_code)
            .unwrap_or_else(|| {
                if app_frame_index.is_some_and(|frame| frame < 3) {
                    "warming_up"
                } else {
                    "query_pending"
                }
            });
        profiler.record_gpu_sample_metadata(
            gpu_sample_status,
            app_frame_index,
            render_frame_index,
            gpu_query_frame_index,
            sample_latency_frames,
        );
        let drain_ns = elapsed_ns(drain_started);
        profiler.record_diagnostics_ns(&["render_diagnostics", "drain_ns"], drain_ns);
        profiler.record_elapsed(&["Last", "record_render_diagnostics_for_frame"], started);
    }

    pub(crate) fn install_detailed_frame_profiler(app: &mut App) {
        app.init_resource::<DetailedFrameProfiler>()
            .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default());

        install_profile_schedules(app);
        insert_profile_schedules_into_main_order(app);
        insert_profile_schedules_into_fixed_order(app);
    }

    fn install_profile_schedules(app: &mut App) {
        app.add_schedule(Schedule::new(FrameProfileBeforeFirst))
            .add_schedule(Schedule::new(FrameProfileAfterFirst))
            .add_schedule(Schedule::new(FrameProfileBeforePreUpdate))
            .add_schedule(Schedule::new(FrameProfileAfterPreUpdate))
            .add_schedule(Schedule::new(FrameProfileBeforeRunFixedMainLoop))
            .add_schedule(Schedule::new(FrameProfileAfterRunFixedMainLoop))
            .add_schedule(Schedule::new(FrameProfileBeforeUpdate))
            .add_schedule(Schedule::new(FrameProfileAfterUpdate))
            .add_schedule(Schedule::new(FrameProfileBeforeSpawnScene))
            .add_schedule(Schedule::new(FrameProfileAfterSpawnScene))
            .add_schedule(Schedule::new(FrameProfileBeforePostUpdate))
            .add_schedule(Schedule::new(FrameProfileAfterPostUpdate))
            .add_schedule(Schedule::new(FrameProfileBeforeLast))
            .add_schedule(Schedule::new(FrameProfileAfterLast))
            .add_schedule(Schedule::new(FrameProfileBeforeFixedFirst))
            .add_schedule(Schedule::new(FrameProfileAfterFixedFirst))
            .add_schedule(Schedule::new(FrameProfileBeforeFixedPreUpdate))
            .add_schedule(Schedule::new(FrameProfileAfterFixedPreUpdate))
            .add_schedule(Schedule::new(FrameProfileBeforeFixedUpdate))
            .add_schedule(Schedule::new(FrameProfileAfterFixedUpdate))
            .add_schedule(Schedule::new(FrameProfileBeforeFixedPostUpdate))
            .add_schedule(Schedule::new(FrameProfileAfterFixedPostUpdate))
            .add_schedule(Schedule::new(FrameProfileBeforeFixedLast))
            .add_schedule(Schedule::new(FrameProfileAfterFixedLast))
            .add_systems(FrameProfileBeforeFirst, begin_first_schedule)
            .add_systems(FrameProfileAfterFirst, end_first_schedule)
            .add_systems(FrameProfileBeforePreUpdate, begin_pre_update_schedule)
            .add_systems(FrameProfileAfterPreUpdate, end_pre_update_schedule)
            .add_systems(
                FrameProfileBeforeRunFixedMainLoop,
                begin_run_fixed_main_loop_schedule,
            )
            .add_systems(
                FrameProfileAfterRunFixedMainLoop,
                end_run_fixed_main_loop_schedule,
            )
            .add_systems(FrameProfileBeforeUpdate, begin_update_schedule)
            .add_systems(FrameProfileAfterUpdate, end_update_schedule)
            .add_systems(FrameProfileBeforeSpawnScene, begin_spawn_scene_schedule)
            .add_systems(FrameProfileAfterSpawnScene, end_spawn_scene_schedule)
            .add_systems(FrameProfileBeforePostUpdate, begin_post_update_schedule)
            .add_systems(FrameProfileAfterPostUpdate, end_post_update_schedule)
            .add_systems(FrameProfileBeforeLast, begin_last_schedule)
            .add_systems(
                FrameProfileAfterLast,
                (
                    end_last_schedule,
                    record_render_diagnostics_for_frame,
                    end_detailed_frame_profile,
                )
                    .chain(),
            )
            .add_systems(FrameProfileBeforeFixedFirst, begin_fixed_first_schedule)
            .add_systems(FrameProfileAfterFixedFirst, end_fixed_first_schedule)
            .add_systems(
                FrameProfileBeforeFixedPreUpdate,
                begin_fixed_pre_update_schedule,
            )
            .add_systems(
                FrameProfileAfterFixedPreUpdate,
                end_fixed_pre_update_schedule,
            )
            .add_systems(FrameProfileBeforeFixedUpdate, begin_fixed_update_schedule)
            .add_systems(FrameProfileAfterFixedUpdate, end_fixed_update_schedule)
            .add_systems(
                FrameProfileBeforeFixedPostUpdate,
                begin_fixed_post_update_schedule,
            )
            .add_systems(
                FrameProfileAfterFixedPostUpdate,
                end_fixed_post_update_schedule,
            )
            .add_systems(FrameProfileBeforeFixedLast, begin_fixed_last_schedule)
            .add_systems(FrameProfileAfterFixedLast, end_fixed_last_schedule);
    }

    fn insert_profile_schedules_into_main_order(app: &mut App) {
        let mut order = app.world_mut().resource_mut::<MainScheduleOrder>();
        order.insert_before(First, FrameProfileBeforeFirst);
        order.insert_after(First, FrameProfileAfterFirst);
        order.insert_before(PreUpdate, FrameProfileBeforePreUpdate);
        order.insert_after(PreUpdate, FrameProfileAfterPreUpdate);
        order.insert_before(RunFixedMainLoop, FrameProfileBeforeRunFixedMainLoop);
        order.insert_after(RunFixedMainLoop, FrameProfileAfterRunFixedMainLoop);
        order.insert_before(Update, FrameProfileBeforeUpdate);
        order.insert_after(Update, FrameProfileAfterUpdate);
        order.insert_before(SpawnScene, FrameProfileBeforeSpawnScene);
        order.insert_after(SpawnScene, FrameProfileAfterSpawnScene);
        order.insert_before(PostUpdate, FrameProfileBeforePostUpdate);
        order.insert_after(PostUpdate, FrameProfileAfterPostUpdate);
        order.insert_before(Last, FrameProfileBeforeLast);
        order.insert_after(Last, FrameProfileAfterLast);
    }

    fn insert_profile_schedules_into_fixed_order(app: &mut App) {
        let mut order = app.world_mut().resource_mut::<FixedMainScheduleOrder>();
        order.insert_before(FixedFirst, FrameProfileBeforeFixedFirst);
        order.insert_after(FixedFirst, FrameProfileAfterFixedFirst);
        order.insert_before(FixedPreUpdate, FrameProfileBeforeFixedPreUpdate);
        order.insert_after(FixedPreUpdate, FrameProfileAfterFixedPreUpdate);
        order.insert_before(FixedUpdate, FrameProfileBeforeFixedUpdate);
        order.insert_after(FixedUpdate, FrameProfileAfterFixedUpdate);
        order.insert_before(FixedPostUpdate, FrameProfileBeforeFixedPostUpdate);
        order.insert_after(FixedPostUpdate, FrameProfileAfterFixedPostUpdate);
        order.insert_before(FixedLast, FrameProfileBeforeFixedLast);
        order.insert_after(FixedLast, FrameProfileAfterFixedLast);
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn nested_records_do_not_double_count_parent_time() {
            let mut profiler = DetailedFrameProfiler::from_env();
            profiler.enabled = true;
            profiler.frame_index = 64;
            profiler.max_depth = 10;
            profiler.top_children = 16;

            profiler.record_thread_ns(MAIN_THREAD_NAME, &["Update", "outer"], None, 100, Some(10));
            profiler.record_thread_ns(
                MAIN_THREAD_NAME,
                &["Update", "outer", "inner"],
                None,
                40,
                Some(20),
            );

            let summary = profiler.summary(200);
            let report = profiler.format_report(200, summary);

            assert!(report.contains("GAME FRAME 64: 200 ns"));
            assert!(report.contains(
                "summary: main_profiled_ns=100 main_unattributed_ns=100 gpu_render_ns=0"
            ));
            assert!(report.contains("-main-schedule 50.00% 100 ns"));
            assert!(report.contains("--Update 100.00% 100 ns"));
            assert!(report.contains("---outer 100.00% 100 ns self_ns=60"));
            assert!(report.contains("----inner 40.00% 40 ns"));
            assert!(report.contains("-unattributed 50.00% 100 ns"));
        }

        #[test]
        fn summary_separates_render_threads_from_main_thread() {
            let mut profiler = DetailedFrameProfiler::from_env();
            profiler.enabled = true;

            profiler.record_thread_ns(MAIN_THREAD_NAME, &["Update", "client"], None, 25, Some(0));
            profiler.record_thread_ns(
                GPU_RENDER_THREAD_NAME,
                &["solari", "direct"],
                None,
                40,
                None,
            );
            profiler.record_thread_ns("render-cpu", &["meshlet", "prepare"], None, 10, None);

            let summary = profiler.summary(100);

            assert_eq!(summary.main_profiled_ns, 25);
            assert_eq!(summary.main_unattributed_ns, 75);
            assert_eq!(summary.gpu_render_ns, 40);
            assert_eq!(summary.render_cpu_ns, 10);
            assert_eq!(summary.total_profiled_ns, 75);
            assert_eq!(summary.thread_count, 3);
        }

        #[test]
        fn fixed_schedules_nest_under_run_fixed_main_loop() {
            let mut profiler = DetailedFrameProfiler::from_env();
            profiler.enabled = true;
            profiler.frame_index = 64;
            profiler.max_depth = 10;
            profiler.top_children = 16;

            profiler.record_thread_ns(
                MAIN_THREAD_NAME,
                &["main-schedule", "RunFixedMainLoop"],
                None,
                100,
                Some(10),
            );
            profiler.record_thread_ns(
                MAIN_THREAD_NAME,
                &["FixedUpdate", "move_player"],
                None,
                40,
                Some(20),
            );

            let summary = profiler.summary(150);
            let report = profiler.format_report(150, summary);

            assert_eq!(summary.main_profiled_ns, 100);
            assert!(report.contains("--RunFixedMainLoop 100.00% 100 ns self_ns=60"));
            assert!(report.contains("---FixedUpdate 40.00% 40 ns"));
            assert!(report.contains("----move_player 100.00% 40 ns"));
        }
    }
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
pub(crate) use enabled::*;
