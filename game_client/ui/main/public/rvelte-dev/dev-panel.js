import { createDiagnosticsLogPanelFacade } from "./generated/facade.js";

export const RVELTE_BROWSER_DEV_PANEL_FACADE_ABI = 1;
export const RVELTE_BROWSER_DEV_PANEL_MANIFEST_ID =
  "rvelte.manifest.diagnostics_log_panel_component.v1";
export const RVELTE_BROWSER_DEV_PANEL_COMPONENT_ID = "diagnostics_log_panel";
export const RVELTE_BROWSER_DEV_PANEL_EVENT_SCHEMA = "rvelte.browser_dev_panel.event.v1";

const SNAPSHOT_SCHEMA = "rvelte.browser_dev_panel.snapshot.v1";
const BRIDGE_SCHEMA = "rvelte.browser_dev_panel.bridge.v1";
const MAX_ROWS = 8;
const MAX_LABEL = 96;
const MAX_MESSAGE = 160;
const decoder = new TextDecoder();
const encoder = new TextEncoder();

function clampInteger(value, min, max, label) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed)) {
    throw new TypeError(`invalid ${label}`);
  }
  return Math.max(min, Math.min(max, parsed));
}

function sanitizeText(value, maxBytes = MAX_LABEL) {
  const input = typeof value === "string" ? value : "";
  const compact = input.replace(/[|\r\n\t]+/g, " ").replace(/\s{2,}/g, " ").trim();
  const lower = compact.toLowerCase();
  if (
    lower.includes("password") ||
    lower.includes("token") ||
    lower.includes("ticket") ||
    lower.includes("session_id") ||
    lower.includes("cookie")
  ) {
    return "[redacted]";
  }
  return compact.slice(0, maxBytes);
}

function normalizeLevel(value) {
  return value === "error" || value === "warning" || value === "info" ? value : "info";
}

function normalizeRow(value, index) {
  const row = isRecord(value) ? value : {};
  const id = sanitizeText(row.id ?? `row-${index}`, 48) || `row-${index}`;
  const level = normalizeLevel(row.level);
  return Object.freeze({
    id,
    level,
    group: sanitizeText(row.group ?? "diagnostic", 36) || "diagnostic",
    source: sanitizeText(row.source ?? "unknown", 64) || "unknown",
    message: sanitizeText(row.message ?? "No message.", MAX_MESSAGE) || "No message.",
    detail: sanitizeText(row.detail ?? "editor", MAX_LABEL) || "editor"
  });
}

export function normalizeDevPanelSnapshot(value = {}) {
  const snapshot = isRecord(value) ? value : {};
  const rows = Array.isArray(snapshot.rows)
    ? snapshot.rows.slice(0, MAX_ROWS).map((row, index) => normalizeRow(row, index))
    : [];
  const errors = clampInteger(snapshot.errors ?? rows.filter((row) => row.level === "error").length, 0, 999, "errors");
  const warnings = clampInteger(
    snapshot.warnings ?? rows.filter((row) => row.level === "warning").length,
    0,
    999,
    "warnings"
  );
  const totalRows = clampInteger(snapshot.totalRows ?? rows.length, 0, 999, "totalRows");
  return Object.freeze({
    revision: clampInteger(snapshot.revision ?? 1, 0, 999999, "revision"),
    errors,
    warnings,
    totalRows,
    runtimeLabel: sanitizeText(snapshot.runtimeLabel ?? "runtime unknown", 64) || "runtime unknown",
    frameLabel: sanitizeText(snapshot.frameLabel ?? "frame n/a", 64) || "frame n/a",
    rows: Object.freeze(rows)
  });
}

function serializeSnapshot(snapshot) {
  const lines = [
    `schema|${SNAPSHOT_SCHEMA}`,
    `revision|${snapshot.revision}`,
    `errors|${snapshot.errors}`,
    `warnings|${snapshot.warnings}`,
    `total_rows|${snapshot.totalRows}`,
    `runtime|${snapshot.runtimeLabel}`,
    `frame|${snapshot.frameLabel}`
  ];
  for (const row of snapshot.rows) {
    lines.push(
      [
        "row",
        row.id,
        row.level,
        row.group,
        row.source,
        row.message,
        row.detail
      ].join("|")
    );
  }
  return `${lines.join("\n")}\n`;
}

function writeSnapshotInput(wasm, snapshot) {
  const bytes = encoder.encode(serializeSnapshot(snapshot));
  const ptr = Number(wasm.rvelte_dev_panel_prepare_input(bytes.length));
  if (!Number.isInteger(ptr) || ptr < 0) {
    throw new TypeError("invalid rvelte input pointer");
  }
  new Uint8Array(wasm.memory.buffer, ptr, bytes.length).set(bytes);
  return bytes.length;
}

function readBridge(exports) {
  const ptr = Number(exports.rvelte_dev_panel_last_ptr());
  const len = Number(exports.rvelte_dev_panel_last_len());
  if (!Number.isInteger(ptr) || !Number.isInteger(len) || ptr < 0 || len < 0) {
    throw new TypeError("invalid rvelte bridge pointer");
  }
  return decoder.decode(new Uint8Array(exports.memory.buffer, ptr, len));
}

function parseInteger(value, label) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new TypeError(`invalid ${label}`);
  }
  return parsed;
}

function parseBridge(text) {
  const lines = text.split("\n").filter((line) => line.length > 0);
  if (lines.shift() !== `schema|${BRIDGE_SCHEMA}`) {
    throw new TypeError("unsupported rvelte dev panel bridge schema");
  }
  const strings = [];
  const patches = [];
  const events = [];
  let revision = 0;
  let rowCount = 0;
  let interactionCount = 0;
  for (const line of lines) {
    const parts = line.split("|");
    switch (parts[0]) {
      case "revision":
        revision = parseInteger(parts[1], "revision");
        break;
      case "row_count":
        rowCount = parseInteger(parts[1], "row count");
        break;
      case "interaction_count":
        interactionCount = parseInteger(parts[1], "interaction count");
        break;
      case "string":
        strings.push(Object.freeze({
          id: parseInteger(parts[1], "string id"),
          value: parts.slice(2).join("|")
        }));
        break;
      case "patch":
        patches.push(Object.freeze(parts.slice(1).map((part) => parseInteger(part, "patch field"))));
        break;
      case "event":
        events.push(Object.freeze({
          name: sanitizeText(parts[1], 48),
          revision: parseInteger(parts[2], "event revision")
        }));
        break;
      case "error":
        throw new Error(`rvelte wasm error: ${parts[1] ?? "unknown"}`);
      default:
        throw new TypeError("unsupported rvelte dev panel bridge record");
    }
  }
  return Object.freeze({
    revision,
    rowCount,
    interactionCount,
    strings: Object.freeze(strings),
    patches: Object.freeze(patches),
    events: Object.freeze(events)
  });
}

function commitBridge(facade, bridge) {
  for (const entry of bridge.strings) {
    facade.setStringValue(entry.id, entry.value);
  }
  facade.applyPatches(bridge.patches);
}

async function instantiateDevPanelWasm(wasmUrl, wasmBytes) {
  if (wasmBytes !== undefined) {
    const module = await WebAssembly.instantiate(wasmBytes, {});
    return module.instance.exports;
  }
  const response = await fetch(wasmUrl);
  if (!response.ok) {
    throw new Error(`dev panel wasm fetch failed: ${response.status}`);
  }
  const bytes = await response.arrayBuffer();
  const module = await WebAssembly.instantiate(bytes, {});
  return module.instance.exports;
}

function resolveElement(value, label, documentRef) {
  if (isElementLike(value)) {
    return value;
  }
  if (typeof value === "string") {
    const element = documentRef.getElementById(value);
    if (element !== null) {
      return element;
    }
  }
  throw new Error(`rvelte dev panel ${label} element is missing`);
}

function resolveOptionalElement(value, documentRef) {
  if (value === undefined || value === null) {
    return null;
  }
  return resolveElement(value, "status", documentRef);
}

function writeStatus(status, text) {
  if (status !== null) {
    status.textContent = text;
  }
}

function refreshEvent(mountSnapshot, bridge) {
  return Object.freeze({
    schema_version: RVELTE_BROWSER_DEV_PANEL_EVENT_SCHEMA,
    component_id: RVELTE_BROWSER_DEV_PANEL_COMPONENT_ID,
    manifest_id: RVELTE_BROWSER_DEV_PANEL_MANIFEST_ID,
    event: "refresh_requested",
    revision: bridge.revision,
    initial_revision: mountSnapshot.revision,
    interaction_count: bridge.interactionCount
  });
}

export async function mountBrowserDevPanelIsland(options = {}) {
  const documentRef = options.document ?? document;
  const root = resolveElement(options.root ?? "fixture-root", "root", documentRef);
  const status = resolveOptionalElement(options.status ?? "smoke-status", documentRef);
  const wasmUrl = options.wasmUrl ?? new URL("./dev_panel.wasm", import.meta.url).href;
  let snapshot = normalizeDevPanelSnapshot(options.snapshot);
  const onRefreshRequested =
    typeof options.onRefreshRequested === "function" ? options.onRefreshRequested : null;
  const updateDocumentTitle = options.updateDocumentTitle !== false;

  const wasm = await instantiateDevPanelWasm(wasmUrl, options.wasmBytes);
  let facade;
  const wasmFacade = Object.freeze({
    rvelte_handle_event(kind, targetNodeId, routeId, payloadSlot) {
      wasm.rvelte_handle_event(kind, targetNodeId, routeId, payloadSlot);
      const bridge = parseBridge(readBridge(wasm));
      commitBridge(facade, bridge);
      writeStatus(status, `event rev=${bridge.revision} rows=${bridge.rowCount}`);
      for (const event of bridge.events) {
        if (event.name === "refresh_requested" && onRefreshRequested !== null) {
          onRefreshRequested(refreshEvent(snapshot, bridge));
        }
      }
    }
  });

  facade = createDiagnosticsLogPanelFacade({
    document: documentRef,
    wasm: wasmFacade,
    detailedEvents: true
  });
  const mountLen = writeSnapshotInput(wasm, snapshot);
  if (wasm.rvelte_dev_panel_mount(mountLen) !== 1) {
    throw new Error("rvelte dev panel mount failed");
  }
  const initialBridge = parseBridge(readBridge(wasm));
  root.replaceChildren();
  commitBridge(facade, initialBridge);
  root.appendChild(facade.node(1));

  writeStatus(status, `ready rev=${initialBridge.revision} rows=${initialBridge.rowCount}`);
  if (updateDocumentTitle) {
    document.title = "rvelte Project-FUN dev panel: ready";
  }

  const mounted = Object.freeze({
    status,
    root,
    refreshButton: facade.node(8),
    snapshot() {
      return snapshot;
    },
    update(nextSnapshot) {
      snapshot = normalizeDevPanelSnapshot(nextSnapshot);
      const updateLen = writeSnapshotInput(wasm, snapshot);
      if (wasm.rvelte_dev_panel_update(updateLen) !== 1) {
        throw new Error("rvelte dev panel update failed");
      }
      const bridge = parseBridge(readBridge(wasm));
      commitBridge(facade, bridge);
      writeStatus(status, `updated rev=${bridge.revision} rows=${bridge.rowCount}`);
      return bridge;
    },
    destroy() {
      root.replaceChildren();
      writeStatus(status, "unmounted");
    }
  });
  if (options.exposeGlobal === true) {
    globalThis.rvelteDevPanelSmoke = mounted;
  }
  return mounted;
}

export function createSampleDevPanelSnapshot(revision = 1) {
  return normalizeDevPanelSnapshot({
    revision,
    errors: 1,
    warnings: 2,
    totalRows: 5,
    runtimeLabel: "client running / server running",
    frameLabel: "frame 42 / gpu 2600 us",
    rows: [
      {
        id: `runtime:${revision}:41`,
        level: "error",
        group: "runtime",
        source: "client.frame_budget",
        message: "Frame budget exceeded in diagnostics sample.",
        detail: "frame 42"
      },
      {
        id: `diagnostic:${revision}:preview`,
        level: "warning",
        group: "diagnostic",
        source: "preview.renderer",
        message: "Preview renderer is using fallback diagnostics data.",
        detail: "preview"
      },
      {
        id: `event:${revision}:refresh`,
        level: "info",
        group: "event",
        source: "editor.diagnostics",
        message: "Diagnostics route snapshot accepted.",
        detail: "dev panel"
      }
    ]
  });
}

function isElementLike(value) {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof value.appendChild === "function" &&
    typeof value.querySelectorAll === "function"
  );
}

function isRecord(value) {
  return typeof value === "object" && value !== null;
}

if (typeof document !== "undefined" && document.getElementById("fixture-root") !== null) {
  mountBrowserDevPanelIsland({
    exposeGlobal: true,
    snapshot: createSampleDevPanelSnapshot()
  }).catch((error) => {
    const status = document.getElementById("smoke-status");
    if (status !== null) {
      status.textContent = "error";
    }
    document.title = "rvelte Project-FUN dev panel: error";
    throw error;
  });
}
