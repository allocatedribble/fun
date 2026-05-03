(function () {
  "use strict";

  var PROTOCOL_VERSION = 1;
  var SCHEMA_REVISION = 1;
  var sequence = 0;
  var requestId = 0;
  var pending = new Map();
  var subscribers = new Map();
  var hostOutbox = [];

  function nextSequence() {
    sequence += 1;
    return sequence;
  }

  function nextRequestId() {
    requestId += 1;
    return requestId;
  }

  function makeEnvelope(channel, kind, payload, id) {
    var envelope = {
      protocol_version: PROTOCOL_VERSION,
      schema_revision: SCHEMA_REVISION,
      channel: channel,
      kind: kind,
      sequence: nextSequence(),
      payload: payload === undefined ? null : payload,
    };
    if (id !== undefined && id !== null) {
      envelope.request_id = id;
    }
    return envelope;
  }

  function sendEnvelope(envelope) {
    if (window.funHost && typeof window.funHost.postMessage === "function") {
      window.funHost.postMessage(envelope);
      return;
    }
    if (typeof window.cefQuery === "function") {
      window.cefQuery({
        request: JSON.stringify(envelope),
        persistent: false,
      });
      return;
    }
    hostOutbox.push(envelope);
  }

  function dispatch(channel, envelope) {
    var handlers = subscribers.get(channel);
    if (!handlers) {
      return;
    }
    handlers.slice().forEach(function (handler) {
      handler(envelope);
    });
  }

  function setText(field, value) {
    var node = document.querySelector('[data-field="' + field + '"]');
    if (node) {
      node.textContent = value == null ? "" : String(value);
    }
  }

  function reportHitRegions() {
    var regions = Array.prototype.slice
      .call(document.querySelectorAll("[data-hit-region]"))
      .map(function (node) {
        var rect = node.getBoundingClientRect();
        return {
          id: node.dataset.hitRegion,
          x: Math.max(0, Math.round(rect.left)),
          y: Math.max(0, Math.round(rect.top)),
          w: Math.max(1, Math.round(rect.width)),
          h: Math.max(1, Math.round(rect.height)),
        };
      });
    window.fun.emit("ui.hit_regions.changed", {
      mode: "hud_passive",
      regions: regions,
    });
  }

  window.fun = {
    version: PROTOCOL_VERSION,
    capabilities: [],
    request: function request(method, payload, options) {
      var id = nextRequestId();
      var timeoutMs = options && options.timeoutMs ? options.timeoutMs : 8000;
      sendEnvelope(
        makeEnvelope(
          "control",
          "request",
          { method: method, payload: payload === undefined ? null : payload },
          id,
        ),
      );
      return new Promise(function (resolve, reject) {
        var timeout = window.setTimeout(function () {
          pending.delete(id);
          reject(new Error("fun UI request timed out"));
        }, timeoutMs);
        pending.set(id, {
          resolve: resolve,
          reject: reject,
          timeout: timeout,
        });
      });
    },
    emit: function emit(eventType, payload) {
      sendEnvelope(makeEnvelope("control", "event", { eventType: eventType, payload: payload }));
    },
    subscribe: function subscribe(channel, handler) {
      var handlers = subscribers.get(channel) || [];
      handlers.push(handler);
      subscribers.set(channel, handlers);
      return function unsubscribe() {
        var current = subscribers.get(channel) || [];
        subscribers.set(
          channel,
          current.filter(function (candidate) {
            return candidate !== handler;
          }),
        );
      };
    },
    receiveFromHost: function receiveFromHost(envelope) {
      if (!envelope || envelope.protocol_version !== PROTOCOL_VERSION) {
        return;
      }
      if (envelope.kind === "response" || envelope.kind === "error") {
        var pendingRequest = pending.get(envelope.request_id);
        if (pendingRequest) {
          window.clearTimeout(pendingRequest.timeout);
          pending.delete(envelope.request_id);
          if (envelope.kind === "error") {
            pendingRequest.reject(envelope.payload);
          } else {
            pendingRequest.resolve(envelope.payload);
          }
        }
      }
      if (envelope.kind === "patch" && envelope.payload) {
        setText("bridge", "patch " + envelope.sequence);
      }
      dispatch(envelope.channel, envelope);
    },
  };

  window.__funHostOutbox = hostOutbox;

  document.addEventListener("click", function (event) {
    var command = event.target && event.target.dataset ? event.target.dataset.command : null;
    if (!command) {
      return;
    }
    setText("bridge", "sent " + command);
    window.fun.request("diagnostics.overlay.set", { enabled: true }).catch(function () {
      setText("bridge", "queued " + command);
    });
  });

  window.addEventListener("resize", reportHitRegions);

  setText("route", document.getElementById("app").dataset.route);
  reportHitRegions();
  window.fun.emit("lifecycle.ready", { route: "hud", page: "hello_world" });
})();
