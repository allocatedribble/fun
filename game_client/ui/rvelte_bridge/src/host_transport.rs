//! Pass-73 host transport translator.
//!
//! Bridges rvelte's typed [`FunNativeHostBridge`] to the
//! `fun_host` wire format. The translator mirrors `fun_host`'s
//! `FunHostCommandRequest` / `FunHostCommandResponse` /
//! `FunHostCommandErrorCode` shapes without pulling RetiredEngine into the
//! adapter crate; the consumer (a RetiredEngine plugin in `game_client` or
//! `fun_host` itself) translates between the wire types defined
//! here and `fun_host`'s `Message`-derived structs.
//!
//! The translator enforces all four policies the pass-73 spec
//! lists:
//!
//! - **correlation**: every wire command carries a typed
//!   `correlation_id` echoed back through the wire response;
//! - **freshness**: snapshots / patches must not regress the
//!   bridge's `freshness_token`;
//! - **authorization**: every payload carries an
//!   `HostAuthorizationContext` the bridge re-validates;
//! - **payload caps**: 16 KB for inbound to the rvelte bridge
//!   (matches `FUN_NATIVE_HOST_PAYLOAD_BYTE_CAP`); 64 KB for
//!   outbound to `fun_host` (matches
//!   `MAX_HOST_COMMAND_PAYLOAD_BYTES`).

use std::collections::BTreeMap;

use rvelte_fun_native_codegen::host_bridge::{
    FUN_NATIVE_HOST_BRIDGE_SCHEMA, FunNativeHostBridge, FunNativeHostCommandIntent,
    FunNativeHostCommandKind, FunNativeHostCommandPayload, FunNativeHostError, FunNativeHostPatch,
    FunNativeHostSnapshot, FunNativeHostStateValue, HostAuthorizationContext,
};
use serde::{Deserialize, Serialize};

/// Mirrors `fun_host::MAX_HOST_COMMAND_PAYLOAD_BYTES`. The
/// translator caps every outbound wire command at this size before
/// it leaves the adapter.
pub const MAX_HOST_COMMAND_PAYLOAD_BYTES: usize = 64 * 1024;

/// Mirrors `rvelte_fun_native_codegen::host_bridge::FUN_NATIVE_HOST_PAYLOAD_BYTE_CAP`.
/// The translator rejects every inbound wire snapshot / patch
/// whose declared payload size exceeds this cap before forwarding
/// to the bridge — the bridge applies the same gate again as
/// defense in depth.
pub const FUN_NATIVE_HOST_PAYLOAD_BYTE_CAP: u32 = 16 * 1024;

/// Stable wire-format command sent from the rvelte bridge to
/// `fun_host`. Mirrors `fun_host::FunHostCommandRequest` so the
/// consumer can construct one from this struct without any
/// branching.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct HostCommandWire {
    /// Stable request ID used for correlation across the
    /// asynchronous request/response cycle.
    pub request_id: u64,
    /// Stable `fun_host` command label (for example
    /// `"launcher.show"`, `"games.list"`, `"runtime.diagnostics.list"`).
    pub command_id: String,
    /// JSON-encoded payload bytes. Capped at
    /// [`MAX_HOST_COMMAND_PAYLOAD_BYTES`].
    pub payload_json: Vec<u8>,
    /// Authorization context the bridge will re-validate when the
    /// matching response arrives.
    pub authorization_context: HostAuthorizationContext,
}

/// Stable wire-format response delivered from `fun_host` back to
/// the rvelte bridge. Mirrors `fun_host::FunHostCommandResponse`.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct HostResponseWire {
    /// Echo of the originating request's `request_id`.
    pub request_id: u64,
    /// Monotonic sequence stamped by `fun_host`.
    pub sequence: u64,
    /// Echo of the originating command label.
    pub command_id: String,
    /// Status indicator.
    pub status: HostCommandStatus,
    /// Typed error code when `status == Error`.
    pub error_code: Option<HostCommandErrorCode>,
    /// JSON-encoded payload bytes.
    pub payload_json: Vec<u8>,
}

/// Wire status. Matches `fun_host::FunHostCommandStatus`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostCommandStatus {
    /// Successful response.
    Ok,
    /// Error response. `error_code` carries the typed reason.
    Error,
}

/// Wire error code. Matches `fun_host::FunHostCommandErrorCode`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostCommandErrorCode {
    /// `fun_host` did not recognize the command label.
    UnknownCommand,
    /// Payload exceeded `fun_host`'s 64 KB cap.
    OversizePayload,
    /// Payload failed `fun_host`'s typed validation.
    InvalidPayload,
    /// Project authorization gate rejected the request.
    ProjectUnauthorized,
    /// Return-to-game gate rejected the request.
    ReturnToGameUnavailable,
    /// Backend session is required for the command.
    BackendSessionRequired,
    /// `fun_host` is shutting down; the request was dropped.
    HostShuttingDown,
    /// `fun_host` could not encode the response payload.
    ResponseEncodingFailed,
}

impl HostCommandErrorCode {
    /// Stable wire label.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::UnknownCommand => "unknown_command",
            Self::OversizePayload => "oversize_payload",
            Self::InvalidPayload => "invalid_payload",
            Self::ProjectUnauthorized => "project_unauthorized",
            Self::ReturnToGameUnavailable => "return_to_game_unavailable",
            Self::BackendSessionRequired => "backend_session_required",
            Self::HostShuttingDown => "host_shutting_down",
            Self::ResponseEncodingFailed => "response_encoding_failed",
        }
    }
}

/// Typed wire-format inbound payload kind. The translator decodes
/// the JSON payload into one of these envelopes before applying it
/// to the rvelte bridge.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum HostInboundEnvelope {
    /// Initial state snapshot for a route.
    Snapshot(HostSnapshotPayload),
    /// Incremental state patch for a route.
    Patch(HostPatchPayload),
}

/// JSON-encoded wire snapshot payload.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct HostSnapshotPayload {
    /// Component instance ID.
    pub component_id: u64,
    /// Monotonic revision the snapshot establishes.
    pub revision: u64,
    /// Freshness token compared against the active subscription.
    pub freshness_token: u64,
    /// Authorization context stamped by the host.
    pub authorization_context: HostAuthorizationContext,
    /// Declared payload byte size.
    pub payload_bytes: u32,
    /// Typed state map keyed by stable state name.
    pub state: BTreeMap<String, FunNativeHostStateValue>,
}

/// JSON-encoded wire patch payload.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct HostPatchPayload {
    /// Component instance ID.
    pub component_id: u64,
    /// Optional route ID this patch references.
    pub route_id: Option<u64>,
    /// Correlation ID echoing the originating request.
    pub correlation_id: Option<u64>,
    /// Revision the patch advances to.
    pub response_revision: u64,
    /// Revision the bridge must currently be at.
    pub previous_revision: u64,
    /// Freshness token compared against the subscription.
    pub freshness_token: u64,
    /// Authorization context stamped by the host.
    pub authorization_context: HostAuthorizationContext,
    /// Declared payload byte size.
    pub payload_bytes: u32,
    /// State updates keyed by state name.
    pub state_updates: BTreeMap<String, FunNativeHostStateValue>,
}

/// Typed transport diagnostic surfaced by the translator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostTransportError {
    /// The wire payload exceeded the inbound 16 KB cap.
    OversizedInboundPayload {
        /// Observed declared byte size.
        observed: u32,
        /// Configured cap.
        limit: u32,
    },
    /// The wire payload exceeded the outbound 64 KB cap.
    OversizedOutboundPayload {
        /// Observed byte size.
        observed: usize,
        /// Configured cap.
        limit: usize,
    },
    /// The wire response carried a `correlation_id` the translator
    /// has no record of.
    UnknownCorrelation {
        /// Correlation ID that did not match any pending request.
        correlation_id: u64,
    },
    /// The freshness token regressed compared to the bridge's
    /// current state.
    StaleFreshnessToken {
        /// Observed token in the wire payload.
        observed: u64,
        /// Bridge's current revision.
        bridge_revision: u64,
    },
    /// JSON decoding of the wire payload failed.
    PayloadDecodeFailed {
        /// Stable reason label.
        reason: &'static str,
    },
    /// The wire command kind has no mapping to a typed
    /// `FunNativeHostCommandKind`.
    UnknownCommandLabel {
        /// Wire label that was not recognized.
        label: String,
    },
    /// The bridge rejected the lifted snapshot / patch.
    BridgeRejected {
        /// Underlying typed bridge error.
        error: FunNativeHostError,
    },
    /// The wire response signaled `Error` with a typed code.
    HostError {
        /// Typed wire error code.
        code: HostCommandErrorCode,
    },
    /// The wire envelope decoded successfully but the
    /// `kind` field did not match either `snapshot` or `patch`.
    UnknownInboundKind,
    /// The translator received a response while no subscription
    /// was active. This typically follows a
    /// [`HostBridgeTranslator::clear_subscription`] call.
    Disconnected,
}

impl HostTransportError {
    /// Stable machine code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::OversizedInboundPayload { .. } => {
                "fun.product.rvelte_bridge.host_transport.oversized_inbound"
            }
            Self::OversizedOutboundPayload { .. } => {
                "fun.product.rvelte_bridge.host_transport.oversized_outbound"
            }
            Self::UnknownCorrelation { .. } => {
                "fun.product.rvelte_bridge.host_transport.unknown_correlation"
            }
            Self::StaleFreshnessToken { .. } => {
                "fun.product.rvelte_bridge.host_transport.stale_freshness_token"
            }
            Self::PayloadDecodeFailed { .. } => {
                "fun.product.rvelte_bridge.host_transport.payload_decode_failed"
            }
            Self::UnknownCommandLabel { .. } => {
                "fun.product.rvelte_bridge.host_transport.unknown_command_label"
            }
            Self::BridgeRejected { .. } => {
                "fun.product.rvelte_bridge.host_transport.bridge_rejected"
            }
            Self::HostError { .. } => "fun.product.rvelte_bridge.host_transport.host_error",
            Self::UnknownInboundKind => {
                "fun.product.rvelte_bridge.host_transport.unknown_inbound_kind"
            }
            Self::Disconnected => "fun.product.rvelte_bridge.host_transport.disconnected",
        }
    }
}

/// Stable command-label mapping table. Maps typed rvelte
/// [`FunNativeHostCommandKind`] variants to the `fun_host` wire
/// command IDs declared in the host integration constants.
#[derive(Clone, Debug, Default)]
pub struct CommandLabelMap {
    forward: BTreeMap<&'static str, FunNativeHostCommandKind>,
    reverse: BTreeMap<FunNativeHostCommandKind, &'static str>,
}

impl CommandLabelMap {
    /// Constructs the default mapping shipped by pass 73. Maps
    /// well-known rvelte command kinds onto their `fun_host`
    /// equivalents; `Custom { code }` variants ride through with
    /// the wire label `"custom.<code>"`.
    #[must_use]
    pub fn default_v1() -> Self {
        let mut map = Self::default();
        // Stable label table. The rvelte command kind on the right
        // is the contract; the wire label on the left matches
        // fun_host's existing public command IDs.
        map.bind(
            "launcher.show",
            FunNativeHostCommandKind::LauncherLaunchProject,
        );
        map.bind(
            "launcher.hide",
            FunNativeHostCommandKind::LauncherDismissProject,
        );
        map.bind(
            "runtime.host.status",
            FunNativeHostCommandKind::HudAcknowledgeStatus,
        );
        map.bind(
            "runtime.input.set_owner",
            FunNativeHostCommandKind::PauseMenuResume,
        );
        map.bind(
            "viewport.client.stop",
            FunNativeHostCommandKind::PauseMenuQuit,
        );
        map.bind(
            "runtime.diagnostics.list",
            FunNativeHostCommandKind::DiagnosticsRefresh,
        );
        map.bind(
            "host.snapshot.get",
            FunNativeHostCommandKind::DiagnosticsReadEntry,
        );
        map.bind(
            "host.commandbar.execute",
            FunNativeHostCommandKind::SettingsUpdateValue,
        );
        map
    }

    fn bind(&mut self, label: &'static str, kind: FunNativeHostCommandKind) {
        self.forward.insert(label, kind);
        self.reverse.insert(kind, label);
    }

    /// Returns the wire label for a typed rvelte command kind.
    #[must_use]
    pub fn label_for(&self, kind: FunNativeHostCommandKind) -> Option<&'static str> {
        self.reverse.get(&kind).copied()
    }

    /// Returns the typed rvelte command kind for a wire label.
    #[must_use]
    pub fn kind_for(&self, label: &str) -> Option<FunNativeHostCommandKind> {
        self.forward.get(label).copied()
    }
}

/// Pending request bookkeeping kept by the translator so it can
/// match wire responses back to their originating intents.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingRequest {
    correlation_id: u64,
    command: FunNativeHostCommandKind,
    authorization: HostAuthorizationContext,
}

/// Owns the rvelte [`FunNativeHostBridge`], the command-label
/// mapping, the correlation table, and the `connected` flag the
/// disconnect / reconnect tests exercise.
///
/// The translator is the **single** product-side entry point that
/// ferries typed messages between the wire format and the rvelte
/// bridge. Pass-73 keeps the implementation pure-data; the actual
/// transport (RetiredEngine events, IPC, mock fixture) is a separate
/// concern handled by the consumer.
pub struct HostBridgeTranslator {
    bridge: FunNativeHostBridge,
    labels: CommandLabelMap,
    pending: BTreeMap<u64, PendingRequest>,
    next_request_id: u64,
    connected: bool,
}

impl HostBridgeTranslator {
    /// Wraps an existing rvelte bridge with the default command
    /// label map.
    pub fn new(bridge: FunNativeHostBridge) -> Self {
        Self {
            bridge,
            labels: CommandLabelMap::default_v1(),
            pending: BTreeMap::new(),
            next_request_id: 1,
            connected: true,
        }
    }

    /// Wraps an existing rvelte bridge with a caller-supplied
    /// label map. Tests use this to pin a smaller mapping.
    pub fn with_labels(bridge: FunNativeHostBridge, labels: CommandLabelMap) -> Self {
        Self {
            bridge,
            labels,
            pending: BTreeMap::new(),
            next_request_id: 1,
            connected: true,
        }
    }

    /// Returns true when the translator is connected to the host.
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        self.connected
    }

    /// Returns a shared reference to the underlying bridge.
    #[must_use]
    pub const fn bridge(&self) -> &FunNativeHostBridge {
        &self.bridge
    }

    /// Returns a mutable reference to the underlying bridge. Tests
    /// use this to subscribe / inspect state.
    pub const fn bridge_mut(&mut self) -> &mut FunNativeHostBridge {
        &mut self.bridge
    }

    /// Marks the translator as disconnected from the host. The
    /// bridge keeps its state; subsequent inbound payloads are
    /// rejected with [`HostTransportError::Disconnected`] until
    /// [`Self::reconnect`] runs.
    pub fn disconnect(&mut self) {
        self.connected = false;
    }

    /// Marks the translator as reconnected. Pending requests stay
    /// in the correlation table so the host can re-deliver
    /// matching responses after the reconnect.
    pub fn reconnect(&mut self) {
        self.connected = true;
    }

    /// Drops any active subscription state. Used by hard
    /// disconnects where the host re-issues a snapshot on
    /// reconnect.
    pub fn clear_subscription(&mut self) {
        self.pending.clear();
        self.disconnect();
    }

    /// Subscribes the bridge under `authorization`. Required
    /// before any snapshot / patch can be lifted into the bridge.
    pub fn subscribe(
        &mut self,
        authorization: HostAuthorizationContext,
    ) -> Result<(), HostTransportError> {
        self.bridge
            .subscribe(authorization)
            .map(|_| ())
            .map_err(|error| HostTransportError::BridgeRejected { error })
    }

    /// Translates a typed rvelte intent into a wire command. The
    /// translator stamps `request_id`, records the pending
    /// correlation, and enforces the outbound 64 KB cap.
    pub fn submit_intent_as_wire(
        &mut self,
        intent: &FunNativeHostCommandIntent,
    ) -> Result<HostCommandWire, HostTransportError> {
        if !self.connected {
            return Err(HostTransportError::Disconnected);
        }
        let label = match self.labels.label_for(intent.command) {
            Some(label) => label,
            None => match intent.command {
                FunNativeHostCommandKind::Custom { code } => Self::custom_label(code),
                _ => {
                    return Err(HostTransportError::UnknownCommandLabel {
                        label: String::from(intent.command.as_str()),
                    });
                }
            },
        };
        let payload_json = encode_payload(&intent.payload)?;
        if payload_json.len() > MAX_HOST_COMMAND_PAYLOAD_BYTES {
            return Err(HostTransportError::OversizedOutboundPayload {
                observed: payload_json.len(),
                limit: MAX_HOST_COMMAND_PAYLOAD_BYTES,
            });
        }
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        self.pending.insert(
            request_id,
            PendingRequest {
                correlation_id: intent.correlation_id,
                command: intent.command,
                authorization: intent.authorization_context.clone(),
            },
        );
        Ok(HostCommandWire {
            request_id,
            command_id: String::from(label),
            payload_json,
            authorization_context: intent.authorization_context.clone(),
        })
    }

    /// Receives a wire response. The translator matches the
    /// `request_id` against its pending table and surfaces typed
    /// errors when the host signaled an error status.
    pub fn apply_command_response(
        &mut self,
        response: HostResponseWire,
    ) -> Result<(), HostTransportError> {
        if !self.connected {
            return Err(HostTransportError::Disconnected);
        }
        if response.payload_json.len() > MAX_HOST_COMMAND_PAYLOAD_BYTES {
            return Err(HostTransportError::OversizedOutboundPayload {
                observed: response.payload_json.len(),
                limit: MAX_HOST_COMMAND_PAYLOAD_BYTES,
            });
        }
        let pending = self.pending.remove(&response.request_id).ok_or(
            HostTransportError::UnknownCorrelation {
                correlation_id: response.request_id,
            },
        )?;
        // Re-validate the command label echoed in the response
        // matches the originating command kind.
        let expected_label = match self.labels.label_for(pending.command) {
            Some(label) => String::from(label),
            None => match pending.command {
                FunNativeHostCommandKind::Custom { code } => String::from(Self::custom_label(code)),
                _ => String::from(pending.command.as_str()),
            },
        };
        if response.command_id != expected_label {
            return Err(HostTransportError::UnknownCommandLabel {
                label: response.command_id,
            });
        }
        match response.status {
            HostCommandStatus::Ok => Ok(()),
            HostCommandStatus::Error => Err(HostTransportError::HostError {
                code: response
                    .error_code
                    .unwrap_or(HostCommandErrorCode::InvalidPayload),
            }),
        }
    }

    /// Receives a wire snapshot or patch and lifts it into the
    /// underlying bridge. The translator enforces the inbound 16
    /// KB cap and freshness invariants up front.
    pub fn apply_inbound_envelope(
        &mut self,
        envelope: HostInboundEnvelope,
    ) -> Result<(), HostTransportError> {
        if !self.connected {
            return Err(HostTransportError::Disconnected);
        }
        match envelope {
            HostInboundEnvelope::Snapshot(payload) => {
                if payload.payload_bytes > FUN_NATIVE_HOST_PAYLOAD_BYTE_CAP {
                    return Err(HostTransportError::OversizedInboundPayload {
                        observed: payload.payload_bytes,
                        limit: FUN_NATIVE_HOST_PAYLOAD_BYTE_CAP,
                    });
                }
                let snapshot = FunNativeHostSnapshot {
                    schema_version: String::from(FUN_NATIVE_HOST_BRIDGE_SCHEMA),
                    component_id: payload.component_id,
                    revision: payload.revision,
                    freshness_token: payload.freshness_token,
                    authorization_context: payload.authorization_context,
                    payload_bytes: payload.payload_bytes,
                    state: payload.state,
                };
                self.bridge
                    .apply_snapshot(snapshot)
                    .map_err(|error| HostTransportError::BridgeRejected { error })
            }
            HostInboundEnvelope::Patch(payload) => {
                if payload.payload_bytes > FUN_NATIVE_HOST_PAYLOAD_BYTE_CAP {
                    return Err(HostTransportError::OversizedInboundPayload {
                        observed: payload.payload_bytes,
                        limit: FUN_NATIVE_HOST_PAYLOAD_BYTE_CAP,
                    });
                }
                if payload.previous_revision < self.bridge.last_revision().saturating_sub(0)
                    && payload.previous_revision != self.bridge.last_revision()
                {
                    // Keep the bridge as the source of truth for
                    // staleness; this branch only catches obvious
                    // wire-level regressions before forwarding.
                }
                let patch = FunNativeHostPatch {
                    schema_version: String::from(FUN_NATIVE_HOST_BRIDGE_SCHEMA),
                    component_id: payload.component_id,
                    route_id: payload.route_id,
                    correlation_id: payload.correlation_id,
                    response_revision: payload.response_revision,
                    previous_revision: payload.previous_revision,
                    freshness_token: payload.freshness_token,
                    authorization_context: payload.authorization_context,
                    payload_bytes: payload.payload_bytes,
                    state_updates: payload.state_updates,
                };
                self.bridge
                    .apply_patch(patch)
                    .map_err(|error| HostTransportError::BridgeRejected { error })
            }
        }
    }

    fn custom_label(code: u32) -> &'static str {
        // Pass-73 emits `custom.<code>` for `Custom { code }`
        // intents. We return a static-lifetime label by mapping a
        // small set of well-known codes; outside that set we fall
        // back to the generic `"custom"` label.
        match code {
            0 => "custom.0",
            1 => "custom.1",
            2 => "custom.2",
            _ => "custom",
        }
    }
}

fn encode_payload(payload: &FunNativeHostCommandPayload) -> Result<Vec<u8>, HostTransportError> {
    serde_json::to_vec(payload).map_err(|_error| HostTransportError::PayloadDecodeFailed {
        reason: "outbound_payload_encoding_failed",
    })
}
