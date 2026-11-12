// Courier Settings page. Talks to Courier through exactly two commands,
// get_settings and save_settings (src-tauri/src/settings.rs). Every change is
// saved (and applied to the Messenger window) right away; Courier validates
// all values again before using them.
"use strict";

(function () {
  var invoke = window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke;
  var state = null; // { config, autostart, modes, colors, sizes, version }
  var $ = function (sel) { return document.querySelector(sel); };

  // ----- saving --------------------------------------------------------------
  var saveTimer = null, statusTimer = null;
  function status(text, isError) {
    var s = $("#status");
    s.textContent = text;
    s.className = isError ? "error" : "";
    clearTimeout(statusTimer);
    if (!isError && text) statusTimer = setTimeout(function () { s.textContent = ""; }, 1500);
  }
  function save(delay) {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(function () {
      invoke("save_settings", { settings: { config: state.config, autostart: state.autostart } })
        .then(function (v) {
          // Keep what Courier actually stored (it may have corrected values),
          // but don't overwrite the field being typed in.
          var typing = document.activeElement;
          state = v;
          render(typing);
          status("Saved");
        })
        .catch(function (e) { status("Couldn't save: " + e, true); });
    }, delay === undefined ? 150 : delay);
  }

  // ----- tabs -----------------------------------------------------------------
  function showTab(name) {
    document.querySelectorAll(".tabs button").forEach(function (b) {
      b.setAttribute("aria-selected", String(b.dataset.tab === name));
    });
    document.querySelectorAll("section[role=tabpanel]").forEach(function (s) {
      s.hidden = s.id !== "tab-" + name;
    });
  }

  // ----- building controls ------------------------------------------------------
  function el(tag, attrs, children) {
    var e = document.createElement(tag);
    Object.keys(attrs || {}).forEach(function (k) {
      if (k === "text") e.textContent = attrs[k];
      else if (k === "class") e.className = attrs[k];
      else e.setAttribute(k, attrs[k]);
    });
    (children || []).forEach(function (c) { e.appendChild(c); });
    return e;
  }

  function build() {
    // Mode
    var mode = $("#mode");
    mode.textContent = "";
    state.modes.forEach(function (m) {
      var b = el("button", { type: "button", "data-mode": m, text: m.charAt(0).toUpperCase() + m.slice(1) });
      b.addEventListener("click", function () {
        state.config.theme.mode = m;
        render();
        save();
      });
      mode.appendChild(b);
    });

    // Colors
    var colors = $("#colors");
    colors.textContent = "";
    state.colors.forEach(function (t) {
      var picker = el("input", { type: "color", "aria-label": t.label });
      var value = el("span", { class: "value small" });
      var reset = el("button", { type: "button", class: "link", text: "Reset" });
      picker.addEventListener("input", function () {
        state.config.theme.colors[t.key] = picker.value;
        render(picker);
        save(250);
      });
      reset.addEventListener("click", function () {
        delete state.config.theme.colors[t.key];
        render();
        save();
      });
      colors.appendChild(el("div", { class: "token", "data-color": t.key }, [
        el("span", { class: "name" }, [el("span", { text: t.label }), value]),
        el("span", { class: "controls" }, [reset, picker]),
      ]));
    });

    // Sizes
    var sizes = $("#sizes");
    sizes.textContent = "";
    state.sizes.forEach(function (t) {
      var range = el("input", { type: "range", min: t.min, max: t.max, step: 1, "aria-label": t.label });
      var out = el("output");
      var reset = el("button", { type: "button", class: "link", text: "Reset" });
      range.addEventListener("input", function () {
        state.config.theme.sizes[t.key] = Number(range.value);
        render(range);
        save(200);
      });
      reset.addEventListener("click", function () {
        delete state.config.theme.sizes[t.key];
        render();
        save();
      });
      sizes.appendChild(el("div", { class: "row token", "data-size": t.key }, [
        el("span", { class: "name" }, [el("b", { text: t.label }), el("small", { class: "value" })]),
        el("span", { class: "controls" }, [reset, range, out]),
      ]));
    });
  }

  // ----- reflecting state -------------------------------------------------------
  function render(skip) {
    var c = state.config;
    document.querySelectorAll("[data-key]").forEach(function (input) {
      if (input === skip) return;
      var v = c[input.dataset.key];
      if (input.type === "checkbox") input.checked = !!v;
      else input.value = v;
    });
    $("#autostart").checked = state.autostart;
    $("#drawer").checked = c.sidebar !== "pinned";
    if ($("#zoom") !== skip) $("#zoom").value = Math.round(c.zoom * 100);
    $("#zoom-out").textContent = Math.round(c.zoom * 100) + "%";

    document.querySelectorAll("#mode button").forEach(function (b) {
      b.setAttribute("aria-pressed", String(b.dataset.mode === c.theme.mode));
    });

    state.colors.forEach(function (t) {
      var row = document.querySelector('[data-color="' + t.key + '"]');
      var v = c.theme.colors[t.key];
      row.classList.toggle("unset", !v);
      row.querySelector(".value").textContent = v ? v : "Facebook default";
      row.querySelector("button.link").disabled = !v;
      var picker = row.querySelector("input");
      if (picker !== skip) picker.value = v ? v.slice(0, 7) : "#808080";
    });

    state.sizes.forEach(function (t) {
      var row = document.querySelector('[data-size="' + t.key + '"]');
      var v = c.theme.sizes[t.key];
      var shown = v === undefined ? t.default : v;
      row.classList.toggle("unset", v === undefined);
      row.querySelector(".value").textContent = v === undefined ? "Default (" + t.default + "px)" : v + "px";
      row.querySelector("output").textContent = shown + "px";
      row.querySelector("button.link").disabled = v === undefined;
      var range = row.querySelector("input");
      if (range !== skip) range.value = shown;
    });

    var css = $("#custom-css");
    if (css !== skip) css.value = c.theme.custom_css;
    $("#css-count").textContent = css.value.length + " / 20000 characters";
    $("#about").textContent = "Courier " + state.version +
      " · settings are stored in %APPDATA%\\dev.personal.courier\\config.json";
  }

  // ----- wiring -------------------------------------------------------------------
  function wire() {
    document.querySelectorAll(".tabs button").forEach(function (b) {
      b.addEventListener("click", function () { showTab(b.dataset.tab); });
    });
    document.querySelectorAll("[data-key]").forEach(function (input) {
      var key = input.dataset.key;
      if (input.type === "checkbox") {
        input.addEventListener("change", function () { state.config[key] = input.checked; save(); });
      } else {
        // Text: save when you leave the field or press Enter.
        input.addEventListener("change", function () { state.config[key] = input.value.trim(); save(); });
      }
    });
    $("#autostart").addEventListener("change", function (e) { state.autostart = e.target.checked; save(); });
    $("#drawer").addEventListener("change", function (e) {
      state.config.sidebar = e.target.checked ? "drawer" : "pinned";
      save();
    });
    $("#zoom").addEventListener("input", function (e) {
      state.config.zoom = Number(e.target.value) / 100;
      render(e.target);
      save(200);
    });
    $("#reset-colors").addEventListener("click", function () { state.config.theme.colors = {}; render(); save(); });
    $("#reset-sizes").addEventListener("click", function () { state.config.theme.sizes = {}; render(); save(); });
    $("#custom-css").addEventListener("input", function (e) {
      state.config.theme.custom_css = e.target.value;
      $("#css-count").textContent = e.target.value.length + " / 20000 characters";
      save(600);
    });
    // Tab inserts two spaces in the CSS editor instead of leaving the field.
    $("#custom-css").addEventListener("keydown", function (e) {
      if (e.key !== "Tab" || e.shiftKey) return;
      e.preventDefault();
      var t = e.target, s = t.selectionStart;
      t.setRangeText("  ", s, t.selectionEnd, "end");
      t.dispatchEvent(new Event("input"));
    });
  }

  function start() {
    if (!invoke) {
      status("Settings must be opened from Courier.", true);
      return;
    }
    invoke("get_settings")
      .then(function (v) {
        state = v;
        build();
        wire();
        render();
      })
      .catch(function (e) { status("Couldn't load settings: " + e, true); });
  }

  start();
})();
