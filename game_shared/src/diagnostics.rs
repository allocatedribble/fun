use std::collections::VecDeque;

pub const TELEMETRY_BUDGET_CLASS: &str = "SampledRuntime";
pub const TELEMETRY_RETENTION_CLASS: &str = "SummarizeThenDiscardRaw";

const REDACTED_DIAGNOSTIC_VALUE: &str = "<redacted>";

/// Runtime diagnostic severity transported to the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum DiagnosticLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Stable span identity for editor-consumed diagnostic trees.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compactly::v1::Encode,
)]
pub struct DiagnosticSpanId(pub u64);

/// Stable frame identity for editor frame/profile views.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compactly::v1::Encode,
)]
pub struct DiagnosticFrameIndex(pub u64);

/// Runtime time marker for a diagnostic event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum DiagnosticTimestampOrTick {
    TimestampNs { ns: u64 },
    Tick { tick: u64 },
    Frame { frame: DiagnosticFrameIndex },
}

/// Source location captured once by the runtime and formatted by the editor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct DiagnosticSource {
    pub file: String,
    pub line: u32,
    pub module: String,
}

impl DiagnosticSource {
    #[must_use]
    pub fn static_location(file: &'static str, line: u32, module: &'static str) -> Self {
        Self {
            file: file.to_owned(),
            line,
            module: module.to_owned(),
        }
    }
}

/// Structured diagnostic field value. The editor owns final formatting.
#[derive(Debug, Clone, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum DiagnosticValue {
    Bool { value: bool },
    I64 { value: i64 },
    U64 { value: u64 },
    FixedMillis { value: i64 },
    Text { value: String },
    Bytes { value: Vec<u8> },
}

/// Structured key/value field carried by a diagnostic packet.
#[derive(Debug, Clone, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct DiagnosticField {
    pub name: String,
    pub value: DiagnosticValue,
}

impl DiagnosticField {
    #[must_use]
    pub fn u64(name: impl Into<String>, value: u64) -> Self {
        Self {
            name: name.into(),
            value: DiagnosticValue::U64 { value },
        }
    }

    #[must_use]
    pub fn text(name: impl Into<String>, value: impl Into<String>) -> Self {
        let name = name.into();
        let value = if is_sensitive_diagnostic_name(&name) {
            REDACTED_DIAGNOSTIC_VALUE.to_owned()
        } else {
            value.into()
        };
        Self {
            value: DiagnosticValue::Text { value },
            name,
        }
    }

    #[must_use]
    pub fn sensitive_text(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: DiagnosticValue::Text {
                value: REDACTED_DIAGNOSTIC_VALUE.to_owned(),
            },
        }
    }

    #[must_use]
    pub fn bytes(name: impl Into<String>, value: Vec<u8>) -> Self {
        let name = name.into();
        Self {
            value: DiagnosticValue::Bytes {
                value: if is_sensitive_diagnostic_name(&name) {
                    Vec::new()
                } else {
                    value
                },
            },
            name,
        }
    }
}

#[must_use]
pub fn is_sensitive_diagnostic_name(name: &str) -> bool {
    let mut token = [0_u8; 32];
    let mut token_len = 0usize;
    for byte in name.bytes().chain(std::iter::once(b'_')) {
        if byte.is_ascii_alphanumeric() {
            if token_len < token.len() {
                token[token_len] = byte.to_ascii_lowercase();
                token_len += 1;
            }
            continue;
        }
        if token_len > 0 && sensitive_diagnostic_token(&token[..token_len]) {
            return true;
        }
        token_len = 0;
    }
    false
}

fn sensitive_diagnostic_token(token: &[u8]) -> bool {
    [
        b"authorization".as_slice(),
        b"bearer".as_slice(),
        b"cookie".as_slice(),
        b"credential".as_slice(),
        b"jwt".as_slice(),
        b"password".as_slice(),
        b"secret".as_slice(),
        b"session".as_slice(),
        b"token".as_slice(),
        b"ticket".as_slice(),
    ]
    .iter()
    .any(|needle| token.windows(needle.len()).any(|window| window == *needle))
}

/// Runtime event emitted once and fanned out to tracing, live editor, and ring sinks.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct DiagnosticEvent {
    pub target: String,
    pub level: DiagnosticLevel,
    pub timestamp_or_tick: DiagnosticTimestampOrTick,
    pub fields: Vec<DiagnosticField>,
    pub source: DiagnosticSource,
    pub frame_index: Option<DiagnosticFrameIndex>,
    pub span_id: Option<DiagnosticSpanId>,
}

/// Counter unit for editor-visible numeric diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum DiagnosticUnit {
    Count,
    Bytes,
    Nanoseconds,
    Packets,
    Frames,
    HertzMilli,
    PercentMilli,
}

/// Counter sample window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct DiagnosticWindow {
    pub sample_count: u32,
    pub duration_ns: u64,
}

/// Numeric diagnostic sample.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct DiagnosticCounter {
    pub target: String,
    pub name: String,
    pub value: DiagnosticValue,
    pub unit: DiagnosticUnit,
    pub window: DiagnosticWindow,
}

/// Timed diagnostic span.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct DiagnosticSpan {
    pub target: String,
    pub name: String,
    pub start_ns: u64,
    pub duration_ns: u64,
    pub parent: Option<DiagnosticSpanId>,
}

/// Frame profile summary for editor frame budget views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct FrameProfileSummary {
    pub frame_ns: u64,
    pub main_thread_ns: u64,
    pub render_thread_ns: u64,
    pub gpu_ns: u64,
    pub unattributed_ns: u64,
    pub row_count: u32,
}

/// One row in an editor-consumed frame profile tree.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct FrameProfileRow {
    pub name: String,
    pub target: String,
    pub depth: u16,
    pub start_ns: u64,
    pub duration_ns: u64,
    pub self_ns: u64,
    pub count: u32,
    pub span_id: Option<DiagnosticSpanId>,
    pub parent: Option<DiagnosticSpanId>,
    pub source: Option<DiagnosticSource>,
}

/// Full frame profile payload for the editor.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct FrameProfilePacket {
    pub frame: DiagnosticFrameIndex,
    pub summary: FrameProfileSummary,
    pub rows: Vec<FrameProfileRow>,
}

/// Editor diagnostic payload variants.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum DiagnosticPacket {
    Event { event: DiagnosticEvent },
    Counter { counter: DiagnosticCounter },
    Span { span: DiagnosticSpan },
    FrameProfile { profile: FrameProfilePacket },
}

/// Monotonic sequence attached when a diagnostic enters a runtime sink.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compactly::v1::Encode,
)]
pub struct DiagnosticSequence(pub u64);

/// Diagnostic packet after sink sequencing.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct RecordedDiagnosticPacket {
    pub sequence: DiagnosticSequence,
    pub packet: DiagnosticPacket,
}

/// Runtime sink selection for the one-emission diagnostic model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiagnosticSinkMask {
    pub tracing: bool,
    pub editor_live_stream: bool,
    pub ring: bool,
}

impl DiagnosticSinkMask {
    pub const TRACING_ONLY: Self = Self {
        tracing: true,
        editor_live_stream: false,
        ring: false,
    };

    pub const EDITOR_ATTACHED: Self = Self {
        tracing: true,
        editor_live_stream: true,
        ring: true,
    };
}

/// Bounded in-memory diagnostic ring used for editor reconnect/replay.
#[derive(Debug, Clone)]
pub struct BoundedDiagnosticRing {
    capacity: usize,
    next_sequence: DiagnosticSequence,
    dropped_packets: u64,
    packets: VecDeque<RecordedDiagnosticPacket>,
}

impl BoundedDiagnosticRing {
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            next_sequence: DiagnosticSequence(1),
            dropped_packets: 0,
            packets: VecDeque::with_capacity(capacity),
        }
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    #[must_use]
    pub const fn dropped_packets(&self) -> u64 {
        self.dropped_packets
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.packets.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    pub fn push(&mut self, packet: DiagnosticPacket) -> Option<RecordedDiagnosticPacket> {
        if self.capacity == 0 {
            self.dropped_packets = self.dropped_packets.saturating_add(1);
            return None;
        }

        if self.packets.len() == self.capacity {
            self.packets.pop_front();
            self.dropped_packets = self.dropped_packets.saturating_add(1);
        }

        let recorded = RecordedDiagnosticPacket {
            sequence: self.next_sequence,
            packet,
        };
        self.next_sequence.0 = self.next_sequence.0.saturating_add(1);
        self.packets.push_back(recorded.clone());
        Some(recorded)
    }

    #[must_use]
    pub fn packets_since(
        &self,
        sequence: Option<DiagnosticSequence>,
    ) -> Vec<RecordedDiagnosticPacket> {
        match sequence {
            Some(sequence) => self
                .packets
                .iter()
                .filter(|packet| packet.sequence > sequence)
                .cloned()
                .collect(),
            None => self.packets.iter().cloned().collect(),
        }
    }
}

/// Backpressure-limited live diagnostic queue for attached editors.
#[derive(Debug, Clone)]
pub struct EditorLiveDiagnosticQueue {
    capacity: usize,
    dropped_packets: u64,
    packets: VecDeque<RecordedDiagnosticPacket>,
}

impl EditorLiveDiagnosticQueue {
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            dropped_packets: 0,
            packets: VecDeque::with_capacity(capacity),
        }
    }

    #[must_use]
    pub const fn dropped_packets(&self) -> u64 {
        self.dropped_packets
    }

    pub fn push(&mut self, packet: RecordedDiagnosticPacket) {
        if self.capacity == 0 {
            self.dropped_packets = self.dropped_packets.saturating_add(1);
            return;
        }
        if self.packets.len() == self.capacity {
            self.packets.pop_front();
            self.dropped_packets = self.dropped_packets.saturating_add(1);
        }
        self.packets.push_back(packet);
    }

    #[must_use]
    pub fn drain(&mut self, max_packets: usize) -> Vec<RecordedDiagnosticPacket> {
        let take = max_packets.min(self.packets.len());
        self.packets.drain(..take).collect()
    }
}

/// Runtime-owned state for the editor diagnostic sinks.
#[derive(Debug, Clone)]
pub struct RuntimeDiagnosticSinks {
    pub sinks: DiagnosticSinkMask,
    pub ring: BoundedDiagnosticRing,
    pub editor_live_stream: EditorLiveDiagnosticQueue,
}

impl RuntimeDiagnosticSinks {
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            sinks: DiagnosticSinkMask::TRACING_ONLY,
            ring: BoundedDiagnosticRing::with_capacity(0),
            editor_live_stream: EditorLiveDiagnosticQueue::with_capacity(0),
        }
    }

    #[must_use]
    pub fn editor_attached(ring_capacity: usize, live_capacity: usize) -> Self {
        Self {
            sinks: DiagnosticSinkMask::EDITOR_ATTACHED,
            ring: BoundedDiagnosticRing::with_capacity(ring_capacity),
            editor_live_stream: EditorLiveDiagnosticQueue::with_capacity(live_capacity),
        }
    }

    pub fn emit(&mut self, packet: DiagnosticPacket) -> Option<RecordedDiagnosticPacket> {
        if !self.sinks.ring && !self.sinks.editor_live_stream {
            return None;
        }

        let recorded = if self.sinks.ring {
            self.ring.push(packet)
        } else {
            Some(RecordedDiagnosticPacket {
                sequence: DiagnosticSequence(0),
                packet,
            })
        }?;

        if self.sinks.editor_live_stream {
            self.editor_live_stream.push(recorded.clone());
        }

        Some(recorded)
    }
}

#[macro_export]
macro_rules! fun_diag_enabled {
    () => {
        cfg!(all(feature = "diagnostics", debug_assertions))
    };
}

#[macro_export]
macro_rules! fun_diag_block {
    ({ $($body:tt)* }) => {{
        #[cfg(all(feature = "diagnostics", debug_assertions))]
        {
            $($body)*
        }
    }};
}

#[macro_export]
macro_rules! fun_diag_block_if {
    ($condition:expr, { $($body:tt)* }) => {{
        #[cfg(all(feature = "diagnostics", debug_assertions))]
        {
            if $condition {
                $($body)*
            }
        }
    }};
}

#[macro_export]
macro_rules! fun_diag_info {
    (target: $target:expr, $($args:tt)+) => {
        $crate::fun_diag_block!({
            ::tracing::info!(
                target: $target,
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
    ($($args:tt)+) => {
        $crate::fun_diag_block!({
            ::tracing::info!(
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
}

#[macro_export]
macro_rules! fun_diag_info_if {
    ($condition:expr, target: $target:expr, $($args:tt)+) => {
        $crate::fun_diag_block_if!($condition, {
            ::tracing::info!(
                target: $target,
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
    ($condition:expr, $($args:tt)+) => {
        $crate::fun_diag_block_if!($condition, {
            ::tracing::info!(
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
}

#[macro_export]
macro_rules! fun_diag_debug {
    (target: $target:expr, $($args:tt)+) => {
        $crate::fun_diag_block!({
            ::tracing::debug!(
                target: $target,
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
    ($($args:tt)+) => {
        $crate::fun_diag_block!({
            ::tracing::debug!(
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
}

#[macro_export]
macro_rules! fun_diag_debug_if {
    ($condition:expr, target: $target:expr, $($args:tt)+) => {
        $crate::fun_diag_block_if!($condition, {
            ::tracing::debug!(
                target: $target,
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
    ($condition:expr, $($args:tt)+) => {
        $crate::fun_diag_block_if!($condition, {
            ::tracing::debug!(
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
}

#[macro_export]
macro_rules! fun_diag_trace {
    (target: $target:expr, $($args:tt)+) => {
        $crate::fun_diag_block!({
            ::tracing::trace!(
                target: $target,
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
    ($($args:tt)+) => {
        $crate::fun_diag_block!({
            ::tracing::trace!(
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
}

#[macro_export]
macro_rules! fun_diag_trace_if {
    ($condition:expr, target: $target:expr, $($args:tt)+) => {
        $crate::fun_diag_block_if!($condition, {
            ::tracing::trace!(
                target: $target,
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
    ($condition:expr, $($args:tt)+) => {
        $crate::fun_diag_block_if!($condition, {
            ::tracing::trace!(
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
}

#[macro_export]
macro_rules! fun_diag_warn {
    (target: $target:expr, $($args:tt)+) => {
        $crate::fun_diag_block!({
            ::tracing::warn!(
                target: $target,
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
    ($($args:tt)+) => {
        $crate::fun_diag_block!({
            ::tracing::warn!(
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
}

#[macro_export]
macro_rules! fun_diag_warn_if {
    ($condition:expr, target: $target:expr, $($args:tt)+) => {
        $crate::fun_diag_block_if!($condition, {
            ::tracing::warn!(
                target: $target,
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
    ($condition:expr, $($args:tt)+) => {
        $crate::fun_diag_block_if!($condition, {
            ::tracing::warn!(
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(frame: u64) -> DiagnosticPacket {
        DiagnosticPacket::Event {
            event: DiagnosticEvent {
                target: "fun::net".to_owned(),
                level: DiagnosticLevel::Info,
                timestamp_or_tick: DiagnosticTimestampOrTick::Frame {
                    frame: DiagnosticFrameIndex(frame),
                },
                fields: vec![
                    DiagnosticField::u64("packet_bytes", 512),
                    DiagnosticField::text("channel", "control"),
                ],
                source: DiagnosticSource::static_location(file!(), line!(), module_path!()),
                frame_index: Some(DiagnosticFrameIndex(frame)),
                span_id: Some(DiagnosticSpanId(7)),
            },
        }
    }

    #[test]
    fn diagnostic_packet_shapes_roundtrip_through_compactly() {
        let packets = vec![
            event(1),
            DiagnosticPacket::Counter {
                counter: DiagnosticCounter {
                    target: "fun::render".to_owned(),
                    name: "fps".to_owned(),
                    value: DiagnosticValue::FixedMillis { value: 144_000 },
                    unit: DiagnosticUnit::HertzMilli,
                    window: DiagnosticWindow {
                        sample_count: 60,
                        duration_ns: 1_000_000_000,
                    },
                },
            },
            DiagnosticPacket::Span {
                span: DiagnosticSpan {
                    target: "fun::world".to_owned(),
                    name: "world_stream_apply".to_owned(),
                    start_ns: 100,
                    duration_ns: 200,
                    parent: Some(DiagnosticSpanId(1)),
                },
            },
            DiagnosticPacket::FrameProfile {
                profile: FrameProfilePacket {
                    frame: DiagnosticFrameIndex(2),
                    summary: FrameProfileSummary {
                        frame_ns: 16_600_000,
                        main_thread_ns: 8_000_000,
                        render_thread_ns: 6_000_000,
                        gpu_ns: 7_000_000,
                        unattributed_ns: 1_000_000,
                        row_count: 1,
                    },
                    rows: vec![FrameProfileRow {
                        name: "material_prepare".to_owned(),
                        target: "fun::render".to_owned(),
                        depth: 1,
                        start_ns: 10,
                        duration_ns: 20,
                        self_ns: 15,
                        count: 1,
                        span_id: Some(DiagnosticSpanId(2)),
                        parent: None,
                        source: None,
                    }],
                },
            },
        ];

        for packet in packets {
            let encoded = compactly::v1::encode(&packet);
            let decoded: DiagnosticPacket =
                compactly::v1::decode(&encoded).expect("packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn bounded_ring_keeps_recent_packets_and_counts_drops() {
        let mut ring = BoundedDiagnosticRing::with_capacity(2);

        let first = ring.push(event(1)).expect("first packet is stored");
        let second = ring.push(event(2)).expect("second packet is stored");
        let third = ring.push(event(3)).expect("third packet is stored");

        assert_eq!(first.sequence, DiagnosticSequence(1));
        assert_eq!(second.sequence, DiagnosticSequence(2));
        assert_eq!(third.sequence, DiagnosticSequence(3));
        assert_eq!(ring.len(), 2);
        assert_eq!(ring.dropped_packets(), 1);
        assert_eq!(
            ring.packets_since(None)
                .into_iter()
                .map(|packet| packet.sequence)
                .collect::<Vec<_>>(),
            vec![DiagnosticSequence(2), DiagnosticSequence(3)]
        );
    }

    #[test]
    fn live_queue_applies_backpressure() {
        let mut sinks = RuntimeDiagnosticSinks::editor_attached(4, 1);

        sinks.emit(event(1));
        sinks.emit(event(2));

        assert_eq!(sinks.ring.len(), 2);
        assert_eq!(sinks.editor_live_stream.dropped_packets(), 1);
        let drained = sinks.editor_live_stream.drain(8);
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].sequence, DiagnosticSequence(2));
    }

    #[test]
    fn disabled_sink_does_not_store_packets() {
        let mut sinks = RuntimeDiagnosticSinks::disabled();

        assert!(sinks.emit(event(1)).is_none());
        assert!(sinks.ring.is_empty());
        assert!(sinks.editor_live_stream.drain(1).is_empty());
    }

    #[test]
    fn diagnostic_field_constructors_redact_secret_named_fields() {
        assert_eq!(
            DiagnosticField::text("session_token", "raw-secret").value,
            DiagnosticValue::Text {
                value: REDACTED_DIAGNOSTIC_VALUE.to_owned()
            }
        );
        assert_eq!(
            DiagnosticField::text("sessiontoken", "raw-secret").value,
            DiagnosticValue::Text {
                value: REDACTED_DIAGNOSTIC_VALUE.to_owned()
            }
        );
        assert_eq!(
            DiagnosticField::sensitive_text("opaque").value,
            DiagnosticValue::Text {
                value: REDACTED_DIAGNOSTIC_VALUE.to_owned()
            }
        );
        assert_eq!(
            DiagnosticField::bytes("auth_ticket_bytes", vec![1, 2, 3]).value,
            DiagnosticValue::Bytes { value: Vec::new() }
        );
        assert_eq!(
            DiagnosticField::text("stream", "mutation_transactions").value,
            DiagnosticValue::Text {
                value: "mutation_transactions".to_owned()
            }
        );
    }
}
