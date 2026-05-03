(function () {
  "use strict";

  var PROTOCOL_VERSION = 1;
  var SCHEMA_REVISION = 1;
  var sequence = 0;
  var requestId = 0;
  var pending = new Map();
  var subscribers = new Map();
  var hostOutbox = [];
  var allowedRoutes = new Set([
    "hud",
    "pause_menu",
    "loadout",
    "scoreboard",
    "chat",
    "loading",
    "diagnostics",
    "devtools_overlay",
  ]);

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

  function setRoute(route) {
    if (!allowedRoutes.has(route)) {
      return;
    }
    document.getElementById("app").dataset.route = route;
    allowedRoutes.forEach(function (candidate) {
      var node = document.querySelector(".route-" + candidate);
      if (node) {
        node.hidden = candidate !== route;
      }
    });
    reportHitRegions();
  }

  function currentHitMode() {
    var route = document.getElementById("app").dataset.route;
    if (route === "hud") {
      return "hud_passive";
    }
    if (route === "chat") {
      return "text_entry";
    }
    return "ui_modal";
  }

  function reportHitRegions() {
    var regions = Array.prototype.slice
      .call(document.querySelectorAll("[data-hit-region]"))
      .filter(function (node) {
        return !node.closest("[hidden]");
      })
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
      mode: currentHitMode(),
      regions: regions,
    });
  }

  function applyPatch(patch) {
    if (!patch || !Array.isArray(patch.patches)) {
      return;
    }
    patch.patches.forEach(function (item) {
      var value = item.value;
      switch (item.path) {
        case "HudHealth":
          setText("health", value);
          break;
        case "HudArmor":
          setText("armor", value);
          break;
        case "HudAmmo":
          setText("ammo", value);
          break;
        case "ObjectiveLabel":
          setText("objective", value);
          break;
        case "Loading":
          setText("loading", value);
          break;
        default:
          setJson(item.path, value);
          break;
      }
    });
  }

  function unwrapValue(value) {
    if (value && typeof value === "object") {
      if ("Text" in value) {
        return value.Text.value;
      }
      if ("U16" in value) {
        return String(value.U16.value);
      }
      if ("U32" in value) {
        return String(value.U32.value);
      }
      if ("Bool" in value) {
        return value.Bool.value ? "On" : "Off";
      }
      if ("JsonBytes" in value) {
        return JSON.stringify(value.JsonBytes.bytes);
      }
    }
    return value == null ? "" : String(value);
  }

  function setText(field, value) {
    var node = document.querySelector('[data-field="' + field + '"]');
    if (node) {
      node.textContent = unwrapValue(value);
    }
  }

  function setJson(field, value) {
    var node = document.querySelector('[data-field="' + String(field).toLowerCase() + '"]');
    if (node) {
      node.textContent = JSON.stringify(value);
    }
  }

  window.fun = {
    version: PROTOCOL_VERSION,
    capabilities: [],
    request: function request(method, payload, options) {
      var id = nextRequestId();
      var timeoutMs = options && options.timeoutMs ? options.timeoutMs : 8000;
      var envelope = makeEnvelope(
        "control",
        "request",
        { method: method, payload: payload === undefined ? null : payload },
        id,
      );
      sendEnvelope(envelope);
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
      if (envelope.kind === "patch") {
        applyPatch(envelope.payload);
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
    window.fun.request("menu.command", { command: command }).catch(function () {});
  });

  document.addEventListener("submit", function (event) {
    if (!event.target || event.target.dataset.form !== "chat") {
      return;
    }
    event.preventDefault();
    var input = event.target.elements.message;
    var message = input.value.trim();
    if (message) {
      window.fun.request("chat.submit", { message: message }).catch(function () {});
      input.value = "";
    }
  });

  document.addEventListener("focusin", function (event) {
    if (event.target && event.target.matches("input, textarea")) {
      window.fun.emit("ui.text_entry.changed", { active: true });
    }
  });

  document.addEventListener("focusout", function (event) {
    if (event.target && event.target.matches("input, textarea")) {
      window.fun.emit("ui.text_entry.changed", { active: false });
    }
  });

  window.addEventListener("resize", reportHitRegions);

  window.addEventListener("hashchange", function () {
    setRoute(window.location.hash.slice(1) || "hud");
  });

  setRoute(window.location.hash.slice(1) || "hud");
  window.fun.emit("lifecycle.ready", { route: document.getElementById("app").dataset.route });
})();
