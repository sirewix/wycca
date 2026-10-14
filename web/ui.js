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
      if (stored && Array.isArray(stored.rings)) return stored;
    } catch {}
    const fresh = JSON.parse(take(w.wycca_default()));
    // Nothing on any platform can discover where you are, so a first run starts
    // somewhere and the panel has a slider for it.
    fresh.location = { lat: 51, lon: 0 };
    return fresh;
  }

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
      if (o[k] == null) o[k] = /^\d+$/.test(k) ? [] : {};
      o = o[k];
    }
    if (Array.isArray(o) && /^\d+$/.test(last)) {
      // The four seasons are one decision, not four: a hole in the array is not
      // a colour serde will take, so an unset sibling falls back to the palette.
      o.length = 4;
      o[last] = value;
      o.forEach((c, i) => {
        if (c == null) o[i] = palette().seasons[i];
      });
    } else {
      o[last] = value;
    }
  }

  const palette = () => form.palettes[cfg.theme] || form.palettes.dark;

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
        c.checked = !!here;
        c.oninput = () => set(c.checked);
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
        c.value = here || defaultFor(f);
        c.oninput = () => set(c.value);
        out.push(c);
        break;
      }
      case "choice": {
        const s = el("select");
        f.options.forEach((o) => s.append(new Option(o, o)));
        s.value = here ?? f.options[0];
        s.oninput = () => {
          set(s.value);
          // Every unset colour shows the theme's own, so a theme change has to
          // redraw the panel rather than just the wall. A new scale changes the
          // ring's title too, so it rebuilds for the same reason.
          if (f.key === "theme" || f.key === "scale") build();
        };
        out.push(s);
        break;
      }
      default: {
        const t = el("input");
        t.type = "text";
        t.placeholder = f.kind === "span" ? "2w" : "";
        t.value = here ?? "";
        t.oninput = () => set(t.value);
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
    word.textContent = "auto";
    tag.append(auto, word);
    return row(f.label, tag, ...out);
  }

  /** What a control offers when "auto" comes off: the theme's own colour, or the middle. */
  function defaultFor(f) {
    if (f.kind === "color") {
      // The last path segment is the palette slot; the seasons are indexed.
      const slot = f.key.split(".").pop();
      const p = palette();
      return /^\d+$/.test(slot) ? p.seasons[+slot] : p[slot];
    }
    return (f.min + f.max) / 2;
  }

  // --- rings -------------------------------------------------------------
  const newRing = (scale) => JSON.parse(json(w.wycca_ring, scale));

  function rings(f) {
    const box = el("div");
    const build = () => {
      box.textContent = "";
      cfg.rings.forEach((ring, i) => {
        // A ring is a dozen controls, so it folds: a row of rings is a screen,
        // not a scroll.
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
        box.append(card);
      });
      const add = el("select");
      add.append(new Option("+ add ring", ""));
      f.scales.forEach((s) => add.append(new Option(s, s)));
      add.oninput = () => {
        cfg.rings.push(newRing(add.value));
        build();
        commit();
      };
      box.append(add);
    };
    build();
    return box;
  }

  // --- events -------------------------------------------------------------
/** Whole days from an .ics feed. The parser is already in the core, so this is the
   same thing `--events` does, minus the file being on disk. */
  function events() {
    const box = el("div");
    const list = () => {
      box.textContent = "";
      const file = el("input");
      file.type = "file";
      file.accept = ".ics,text/calendar";
      file.onchange = async () => {
        const f = file.files[0];
        if (!f) return;
        if (f.size > SCRATCH) {
          boot.textContent = `${f.name} is ${f.size} bytes; up to ${SCRATCH} fits`;
          return;
        }
        const parsed = JSON.parse(json(w.wycca_events, await f.text()));
        // Already-present days are skipped so re-importing a feed changes nothing.
        for (const e of parsed) {
          if (!cfg.events.some((x) => x.date === e.date && x.label === e.label)) cfg.events.push(e);
        }
        cfg.events.sort((a, b) => a.date.localeCompare(b.date));
        list();
        commit();
      };
      box.append(
        row("import .ics", file),
        row(
          "count",
          String(cfg.events.length),
          button("clear", () => {
            cfg.events = [];
            list();
            commit();
          })
        )
      );
      const scroll = el("div", "scroll");
      cfg.events.forEach((e, i) => {
        scroll.append(
          row(
            `${e.date} ${e.label}`,
            button("✕", () => {
              cfg.events.splice(i, 1);
              list();
              commit();
            })
          )
        );
      });
      box.append(scroll);
    };
    list();
    return box;
  }

  // --- what the platform knows -------------------------------------------
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
    sum.textContent = "Events";
    d.append(sum, events());
    panel.append(d);
  }

  build();
  draw();
  setInterval(draw, 1000);
})().catch((e) => {
  document.getElementById("boot").textContent = `${e && e.message ? e.message : e} — try: nix develop -c web/build.sh`;
});