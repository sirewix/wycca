"use strict";

// The page. No bundler and no module script, because a file:// page can load
// neither: plain <script> tags, and a wasm module that is fetched when the page is
// served and base64-inlined when it is not.
(async function () {
  const CONFIG = "wycca.config";
  const ZONE = "wycca.zone";
  const SCRATCH = 1 << 20; // one page-long buffer for the JSON and .ics we hand in

  const enc = new TextEncoder();
  const dec = new TextDecoder();
  const stage = document.getElementById("stage");
  const panel = document.getElementById("panel");
  const boot = document.getElementById("boot");
  const el = (tag, cls) => Object.assign(document.createElement(tag), cls ? { className: cls } : {});

  /** Served: fetch the module like any other asset. file://: fall back to the
   *  base64 build.sh also emits, because fetch of a relative path is blocked. */
  async function instantiate() {
    try {
      const res = await fetch("wycca.wasm");
      if (!res.ok) throw new Error(res.status);
      return (await WebAssembly.instantiate(await res.arrayBuffer(), {})).instance;
    } catch {
      if (!globalThis.wyccaWasm) throw new Error("wycca.wasm.js is missing");
      const b64 = Uint8Array.from(atob(wyccaWasm), (c) => c.charCodeAt(0));
      return (await WebAssembly.instantiate(b64, {})).instance;
    }
  }

  const w = (await instantiate()).exports;
  const scratch = w.wycca_scratch(SCRATCH);

  /** Whatever the last call left behind: the C ABI hands back a pointer and a length. */
  function take(ptr) {
    const s = dec.decode(new Uint8Array(w.memory.buffer, ptr, w.wycca_len()));
    w.wycca_free(ptr);
    return s;
  }

  /** Pass a JSON string in, read a JSON string out. */
  function json(fn, str) {
    const b = enc.encode(str);
    new Uint8Array(w.memory.buffer, scratch, SCRATCH).set(b);
    return take(fn(scratch, b.length));
  }

  const form = JSON.parse(take(w.wycca_form()));
  let zone = localStorage.getItem(ZONE) || null;
  let cfg = load();

  function load() {
    try {
      const stored = JSON.parse(localStorage.getItem(CONFIG));
      // A config written before layers were a list parses into a ring that draws
      // nothing, so ask the core rather than trust the shape: its render is the
      // only parser there is, and it says so in the SVG.
      if (stored && Array.isArray(stored.rings) && !broken(stored)) return stored;
    } catch {}
    const fresh = JSON.parse(take(w.wycca_default()));
    // Nothing on any platform can discover where you are, so a first run starts
    // somewhere and the panel has a slider for it.
    fresh.location = { lat: 51, lon: 0 };
    return fresh;
  }

  const broken = (config) =>
    json(w.wycca_render, JSON.stringify(config), 0).includes("<desc>");
  const commit = () => {
    localStorage.setItem(CONFIG, JSON.stringify(cfg));
    draw();
  };

  // --- dotted paths ------------------------------------------------------
  const get = (o, path) => {
    let v = o;
    for (const k of path.split(".")) {
      if (v == null) return undefined;
      v = v[/^\d+$/.test(k) ? +k : k];
    }
    return v;
  };

  function put(obj, path, value) {
    const keys = path.split(".");
    const last = keys.pop();
    let o = obj;
    for (const k of keys) {
      if (o[k] == null) o[k] = {};
      o = o[k];
    }
    o[last] = value;
  }

  const palette = () => form.palettes[cfg.theme] || form.palettes.dark;
  const feeds = () => Object.keys(cfg.feeds || {});

  // --- controls ----------------------------------------------------------
  function row(label, ...nodes) {
    const r = el("div", "row");
    const l = el("label");
    l.textContent = label;
    const box = el("div", "ctl");
    nodes.forEach((n) => box.append(n));
    r.append(l, box);
    return r;
  }

  const button = (text, on) => {
    const b = el("button");
    b.textContent = text;
    b.onclick = on;
    return b;
  };

  const select = (options, here, on) => {
    const s = el("select");
    options.forEach((o) => s.append(new Option(o, o)));
    s.value = here ?? options[0] ?? "";
    s.oninput = () => on(s.value);
    return s;
  };

  /** One form field, bound to the object it configures. */
  function field(f, obj) {
    const here = get(obj, f.key);
    const out = [];
    const set = (v) => {
      put(obj, f.key, v);
      commit();
    };

    switch (f.kind) {
      case "bool": {
        const c = el("input");
        c.type = "checkbox";
        // A flag that can be null is on when it is there at all, and unchecking
        // takes the whole thing away rather than writing false.
        c.checked = f.optional ? here != null : !!here;
        c.oninput = () => set(f.optional ? (c.checked ? {} : null) : c.checked);
        out.push(c);
        break;
      }
      case "num": {
        const n = el("input");
        n.type = "number";
        Object.assign(n, { step: f.step, min: f.min, max: f.max });
        n.value = here ?? (f.min + f.max) / 2;
        const s = el("input");
        // Spanning a year in milliseconds is too many steps to slide through, so
        // wide ranges get the number box and its steppers only.
        if ((f.max - f.min) / f.step <= 2000) {
          s.type = "range";
          Object.assign(s, { min: f.min, max: f.max, step: f.step, value: n.value });
          s.oninput = () => {
            n.value = s.value;
            set(+s.value);
          };
          out.push(s);
        }
        n.oninput = () => {
          s.value = n.value;
          set(+n.value);
        };
        out.push(n);
        break;
      }
      case "color": {
        const c = el("input");
        c.type = "color";
        c.value = resolved(here) || defaultFor(f);
        c.oninput = () => set(c.value);
        out.push(c);
        break;
      }
      case "choice":
      case "feed": {
        const options = f.kind === "feed" ? ["", ...feeds()] : f.options;
        const s = select(options, here, (v) => {
          set(v || null);
          // Every unset colour shows the theme's own, so a theme change has to
          // redraw the panel rather than just the wall. A new scale or layer kind
          // changes the card below it, so it rebuilds for the same reason.
          if (f.key === "theme" || f.key === "scale") build();
        });
        out.push(s);
        break;
      }
      default: {
        const t = el("input");
        t.type = "text";
        t.value = here ?? "";
        // A phase is a weekday or a month where the unit has names, and a number
        // where it does not, so a number typed stays a number.
        const typed = (v) => (/^\s*-?\d+\s*$/.test(v) ? +v : v);
        t.oninput = () => set(typed(t.value));
        out.push(t);
      }
    }

    if (!f.optional) return row(f.label, ...out);
    const auto = el("input");
    auto.type = "checkbox";
    auto.checked = here == null;
    auto.oninput = () => {
      out.forEach((n) => (n.disabled = auto.checked));
      set(auto.checked ? null : defaultFor(f));
    };
    out.forEach((n) => (n.disabled = auto.checked));
    const tag = el("span", "auto");
    const word = el("span");
    word.textContent = f.optional;
    tag.append(auto, word);
    return row(f.label, tag, ...out);
  }

  /** A layer's colour is a palette slot name or a hex, so the swatch shows the
   *  colour either resolves to, while the config keeps the name. */
  function resolved(value) {
    return /^#/.test(value) ? value : palette()[value];
  }

  /** What a control offers when "auto" comes off: the theme's own colour, or the middle. */
  function defaultFor(f) {
    if (f.kind === "color") {
      const p = palette();
      // A layer's own colour ends in the part (`background`, `color`), not in a
      // slot, so there is nothing to look up: muted, the one colour that shows on
      // every fill. A section colour ends in its slot.
      if (f.key.startsWith("disp.")) return p.muted;
      return p[f.key.split(".").pop()] ?? p.muted;
    }
    return (f.min + f.max) / 2;
  }

  // --- rings and layers ---------------------------------------------------
  const newRing = (scale) => JSON.parse(json(w.wycca_ring, scale));
  const newLayer = (kind) => JSON.parse(json(w.wycca_layer, kind));
  /** The core names its own layers, so a card's title and its fields cannot
   *  disagree about what it is. */
  const titleOf = (layer) => json(w.wycca_title, JSON.stringify(layer));

  function rings(f) {
    const box = el("div");
    const build = () => {
      box.textContent = "";
      cfg.rings.forEach((ring, i) => {
        // A ring is a handful of controls and a list of layers, so the card
        // folds: a row of rings is a screen, not a scroll.
        const card = el("details", "ring");
        const head = el("summary", "ringhead");
        const step = (d) => () => {
          [cfg.rings[i], cfg.rings[i + d]] = [cfg.rings[i + d], cfg.rings[i]];
          build();
          commit();
        };
        const title = el("span", "ringtitle");
        title.textContent = ring.scale;
        head.append(
          title,
          button("▲", step(-1)),
          button("▼", step(1)),
          button("✕", () => {
            cfg.rings.splice(i, 1);
            build();
            commit();
          })
        );
        card.append(head);
        form.ring_fields.forEach((rf) => card.append(field(rf, ring)));
        card.append(layers(ring, { kinds: f.kinds }));
        box.append(card);
      });
      const add = select(f.scales, "", (s) => {
        if (!s) return;
        cfg.rings.push(newRing(s));
        build();
        commit();
      });
      add.prepend(new Option("+ add ring", ""));
      box.append(add);
    };
    build();
    return box;
  }

  /** The layers on one ring, in paint order. Never folded: the layers are the
   *  ring, and a ring whose layers are hidden is a ring nobody can see. */
  function layers(ring, f) {
    const box = el("div", "layers");
    const build = () => {
      box.textContent = "";
      const step = (i, d) => () => {
        [ring.layers[i], ring.layers[i + d]] = [ring.layers[i + d], ring.layers[i]];
        build();
        commit();
      };
      ring.layers.forEach((layer, i) => {
        const card = el("div", "layer");
        const head = el("div", "layerhead");
        const title = el("span", "layertitle");
        title.textContent = titleOf(layer);
        head.append(
          title,
          select(f.kinds, layer.kind, (kind) => {
            // The drawing is the person's; only the source comes from the core.
            const fresh = newLayer(kind);
            fresh.disp = layer.disp || {};
            ring.layers[i] = fresh;
            build();
            commit();
          }),
          button("▲", step(i, -1)),
          button("▼", step(i, 1)),
          button("✕", () => {
            ring.layers.splice(i, 1);
            build();
            commit();
          })
        );
        card.append(head);
        (form.layer_fields[layer.kind] || []).forEach((lf) => card.append(field(lf, layer)));
        box.append(card);
      });
      const add = select(f.kinds, "", (kind) => {
        if (!kind) return;
        ring.layers.push(newLayer(kind));
        build();
        commit();
      });
      add.prepend(new Option("+ add layer", ""));
      box.append(add);
    };
    build();
    return box;
  }

  // --- feeds ---------------------------------------------------------------
  /** Whole days from an .ics feed, by the name layers refer to it by. The parser
   *  is in the core already, so this is what `--events` does, minus the file
   *  being on disk. */
  function feedPanel() {
    const box = el("div");
    const load = async (file, name) => {
      if (!file) return;
      if (file.size > SCRATCH) {
        boot.textContent = `${file.name} is ${file.size} bytes; up to ${SCRATCH} fits`;
        return;
      }
      const parsed = JSON.parse(json(w.wycca_events, await file.text()));
      // Already-present days are skipped so re-importing a feed changes nothing.
      cfg.feeds = cfg.feeds || {};
      const days = (cfg.feeds[name] = cfg.feeds[name] || []);
      for (const e of parsed) {
        if (!days.some((x) => x.date === e.date && x.label === e.label)) days.push(e);
      }
      days.sort((a, b) => a.date.localeCompare(b.date));
      list();
      commit();
    };
    const list = () => {
      box.textContent = "";
      cfg.feeds = cfg.feeds || {};
      for (const name of feeds()) {
        const days = cfg.feeds[name];
        const rename = el("input");
        rename.type = "text";
        rename.value = name;
        // One keystroke at a time, so the days are held here rather than looked
        // up by the old name, which is gone after the first one.
        rename.oninput = () => {
          delete cfg.feeds[name];
          cfg.feeds[rename.value] = days;
          // Every layer that reads this feed has to read the new name.
          cfg.rings.forEach((ring) =>
            ring.layers.forEach((l) => {
              if (l.source === name) l.source = rename.value;
            })
          );
          commit();
        };
        const file = el("input");
        file.type = "file";
        file.accept = ".ics,text/calendar";
        file.onchange = () => load(file.files[0], rename.value || name);
        box.append(
          row(
            "feed",
            rename,
            file,
            String(days.length),
            button("✕", () => {
              delete cfg.feeds[name];
              list();
              commit();
            })
          )
        );
      }
      const add = el("input");
      add.type = "file";
      add.accept = ".ics,text/calendar";
      // A new feed is named after the file, which is the name `--events` and the
      // year ring's holidays layer already agree on.
      add.onchange = () => load(add.files[0], add.files[0].name.replace(/\.ics$/i, ""));
      box.append(row("import .ics", add));
      box.append(
        row("days in all", String(feeds().reduce((n, f) => n + cfg.feeds[f].length, 0)))
      );
    };
    list();
    return box;
  }

  function platform() {
    const frag = document.createDocumentFragment();
    const tz = el("select");
    tz.append(new Option(`auto (${Intl.DateTimeFormat().resolvedOptions().timeZone})`, ""));
    (Intl.supportedValuesOf?.("timeZone") || []).forEach((z) => tz.append(new Option(z, z)));
    tz.value = zone || "";
    tz.oninput = () => {
      zone = tz.value || null;
      localStorage.setItem(ZONE, zone || "");
      commit();
    };
    frag.append(row("timezone", tz));

    const off = el("input");
    off.type = "number";
    off.step = 60000;
    off.placeholder = "auto";
    off.value = cfg.utc_offset_ms ?? "";
    off.oninput = () => set(cfg, "utc_offset_ms", off.value === "" ? null : +off.value);
    frag.append(row("offset ms", off));
    frag.append(
      row(
        "config",
        button("reset", () => {
          localStorage.removeItem(CONFIG);
          cfg = load();
          build();
          commit();
        })
      )
    );
    return frag;
  }

  /** Milliseconds to add to the instant to get the wall clock, which is the
   *  whole timezone seam: an offset, recomputed so daylight saving follows. */
  function offsetMs(now) {
    if (cfg.utc_offset_ms != null) return cfg.utc_offset_ms;
    if (!zone) return -new Date().getTimezoneOffset() * 60000;
    const p = {};
    new Intl.DateTimeFormat("en-US", {
      timeZone: zone,
      hour12: false,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    })
      .formatToParts(now)
      .forEach((x) => (p[x.type] = +x.value));
    return Date.UTC(p.year, p.month - 1, p.day, p.hour % 24, p.minute, p.second) - Math.floor(now / 1000) * 1000;
  }

  // --- the wall ----------------------------------------------------------
  function draw() {
    const now = Date.now();
    // Rings read civil time, so they get the wall clock relabelled as UTC, and
    // the solar marks get the offset. Exactly what the CLI does.
    const offset = offsetMs(now);
    const b = enc.encode(JSON.stringify(Object.assign({}, cfg, { utc_offset_ms: offset })));
    new Uint8Array(w.memory.buffer, scratch, SCRATCH).set(b);
    stage.innerHTML = take(w.wycca_render(scratch, b.length, now + offset));
  }

  function build() {
    panel.textContent = "";
    panel.append(platform());
    for (const s of form.sections) {
      const d = el("details");
      const sum = el("summary");
      sum.textContent = s.label;
      d.append(sum);
      s.fields.forEach((f) => d.append(f.kind === "rings" ? rings(f) : field(f, cfg)));
      d.open = s.id === "layout";
      panel.append(d);
    }
    // List editing is chrome, not a field, so it sits outside the core's sections.
    const d = el("details");
    const sum = el("summary");
    sum.textContent = "Calendars";
    d.append(sum, feedPanel());
    panel.append(d);
  }

  build();
  draw();
  setInterval(draw, 1000);
})().catch((e) => {
  document.getElementById("boot").textContent = `${e && e.message ? e.message : e} — try: nix develop -c web/build.sh`;
});