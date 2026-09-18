/*
 * ProfileIT editor.
 *
 * Holds the whole config as JSON and sends it in one piece on save. The
 * server merges it into the existing file with toml_edit, so comments in
 * the file are preserved.
 *
 * A translatable value (Text) is either a plain string or a
 * { ko: "...", en: "..." } map. Editing the default language keeps it as a
 * plain string; typing into another language for the first time is what
 * turns it into a map — this keeps the TOML clean for entries nobody has
 * translated yet.
 */
(function () {
  "use strict";

  var UI_LANG_KEY = "profileit.uiLang";

  var state = {
    config: null,
    meta: null,
    lang: null, // Language currently being edited (card content)
    defaultLang: null,
    uiLang: null, // Editor's own UI language. Independent of the card's language
    diagnostics: null, // Last validation results. Redrawn when the UI language changes
    dirty: false,
  };

  var panel = document.getElementById("panel");

  // ─── UI strings ───────────────────────────────────────────────────────────

  // Assembles a message from the server into the current UI language.
  //
  // The server sends `{key, args}` rather than a finished sentence, since
  // it's the UI's job to decide which language to render it in. The old
  // shape (a plain string) is passed through as-is too.
  function msg(value) {
    if (!value) return "";
    if (typeof value === "string") return value;
    if (value.key) return t(value.key, value.args);
    return String(value);
  }

  // Font list. The server sends keys instead of names, so we resolve them to the UI language here.
  function fontOptions() {
    return (state.meta.fonts || []).map(function (f) {
      return { value: f.value, label: t(f.labelKey), weights: f.weights };
    });
  }

  /**
   * Editor UI strings.
   *
   * **Independent** of the card content's language — it's common to edit an
   * English card from a Korean UI. The server sends all three languages at
   * once, so switching between them needs no round trip.
   */
  function t(key, args) {
    var table = (state.meta && state.meta.uiStrings && state.meta.uiStrings[state.uiLang]) || {};
    // A missing key falls back to showing the key itself, so it's obvious right away what's missing.
    var text = table[key] || key;

    Object.keys(args || {}).forEach(function (name) {
      var value = args[name];
      // An "@key" argument marks the value itself as translatable (e.g. a "push" failure).
      if (typeof value === "string" && value.charAt(0) === "@") {
        value = table[value.slice(1)] || value.slice(1);
      }
      text = text.split("{" + name + "}").join(value);
    });
    return text;
  }

  /** UI language remembered by this machine. Falls back to the browser's setting if none. */
  function initialUiLang(available) {
    var saved = null;
    try {
      saved = localStorage.getItem(UI_LANG_KEY);
    } catch (err) {
      // If we can't read storage, we just fall back to the default.
    }
    if (saved && available.indexOf(saved) >= 0) return saved;

    var preferred = (navigator.language || "").slice(0, 2);
    if (available.indexOf(preferred) >= 0) return preferred;

    return available[0];
  }

  function setUiLang(code) {
    state.uiLang = code;
    try {
      localStorage.setItem(UI_LANG_KEY, code);
    } catch (err) {
      // If we can't remember it, this session still works fine.
    }
    document.documentElement.lang = code;
    applyStaticText();
    renderUiLangPicker();
    renderLangTabs();
    render();
    // The diagnostics lines are server strings too, so re-render them along with everything else.
    if (state.diagnostics) showDiagnostics(state.diagnostics);
  }

  /** Fills in the text baked into the HTML using the current UI language. */
  function applyStaticText() {
    document.title = t("editor.title");
    setText(".ed-bar__title", t("editor.title"));
    setText("#build", t("editor.build"));
    setText("#github", t("editor.github"));
    setText("#deploy", t("editor.deploy"));
    setText("#save", t("editor.save"));
    setText("#reload", t("editor.reload"));
    setText(".ed-preview__bar > span", t("editor.preview"));

    var tabs = document.getElementById("lang-tabs");
    if (tabs) tabs.setAttribute("aria-label", t("editor.contentLang"));
  }

  function setText(selector, text) {
    var node = document.querySelector(selector);
    if (node) node.textContent = text;
  }

  /**
   * UI language picker.
   *
   * Placed small in the top-right corner rather than next to the card
   * language tabs, since sitting them side by side would be confusing —
   * this is a setting people rarely change once they've picked it.
   */
  function renderUiLangPicker() {
    var host = document.getElementById("ui-lang");
    if (!host || !state.meta) return;

    host.textContent = "";
    host.setAttribute("aria-label", t("editor.uiLang"));
    host.title = t("editor.uiLang");

    (state.meta.languages || []).forEach(function (lang) {
      var button = el("button", {
        class: "ed-uilang",
        type: "button",
        text: lang.value.toUpperCase(),
        "aria-pressed": String(lang.value === state.uiLang),
        title: lang.label,
        onclick: function () {
          if (lang.value !== state.uiLang) setUiLang(lang.value);
        },
      });
      host.appendChild(button);
    });
  }

  var statusEl = document.getElementById("status");
  var diagnosticsEl = document.getElementById("diagnostics");
  var previewEl = document.getElementById("preview");
  var langTabs = document.getElementById("lang-tabs");

  // ─── Translatable values ────────────────────────────────────────────────

  function readText(value) {
    if (value == null) return "";
    if (typeof value === "string") {
      // A plain string is shared by every language. Leave it blank on other
      // language tabs, showing the original as a placeholder so it's clear
      // what needs translating.
      return state.lang === state.defaultLang ? value : "";
    }
    return value[state.lang] || "";
  }

  /** Default-language original text, used as the placeholder on translation tabs. */
  function baseText(value) {
    if (value == null) return "";
    if (typeof value === "string") return value;
    return value[state.defaultLang] || "";
  }

  function writeText(value, next) {
    var isDefault = state.lang === state.defaultLang;

    if (value == null || typeof value === "string") {
      if (isDefault) return next;
      if (!next) return value; // Clearing the translation leaves the plain string as-is
      var created = {};
      if (value) created[state.defaultLang] = value;
      created[state.lang] = next;
      return created;
    }

    var map = Object.assign({}, value);
    if (next) map[state.lang] = next;
    else delete map[state.lang];

    // If only the default language is left, revert back to a plain string.
    var keys = Object.keys(map);
    if (keys.length === 0) return "";
    if (keys.length === 1 && keys[0] === state.defaultLang) return map[state.defaultLang];
    return map;
  }

  // ─── Small DOM helpers ────────────────────────────────────────────────────

  function el(tag, attrs, children) {
    var node = document.createElement(tag);
    Object.keys(attrs || {}).forEach(function (key) {
      if (key === "class") node.className = attrs[key];
      else if (key === "text") node.textContent = attrs[key];
      else if (key.slice(0, 2) === "on") node.addEventListener(key.slice(2), attrs[key]);
      else if (attrs[key] !== null && attrs[key] !== undefined) node.setAttribute(key, attrs[key]);
    });
    (children || []).forEach(function (child) {
      if (child) node.appendChild(child);
    });
    return node;
  }

  /**
   * Opens an outside address in the default browser.
   *
   * The desktop app's webview blocks `window.open`. And even where it
   * doesn't, if the editor window navigated to an external site there'd be
   * no good way back, so we ask the server to open it in the outside
   * browser instead.
   */
  function openExternal(url) {
    if (!url) return;
    fetch("/api/open", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ url: url }),
    }).catch(function () {
      // Fallback for when the editor is open in a regular browser.
      window.open(url, "_blank");
    });
  }

  // Intercept every target="_blank" link in one place. Catching them all
  // here leaves no room for a newly-created link to slip through unhandled.
  document.addEventListener("click", function (event) {
    var link = event.target.closest('a[target="_blank"]');
    if (!link) return;
    event.preventDefault();
    openExternal(link.href);
  });

  function changed() {
    state.dirty = true;
    setStatus(t("editor.status.unsaved"));
  }

  function setStatus(message, isError) {
    statusEl.textContent = message || "";
    statusEl.className = "ed-status" + (isError ? " ed-status--error" : "");
  }

  /** Mutates the value and re-renders. Used for edits that change structure. */
  function update(fn) {
    fn();
    changed();
    render();
  }

  // ─── Input widgets ──────────────────────────────────────────────────────

  /** A translatable single-line input. */
  function textField(label, owner, key, options) {
    options = options || {};
    var translating = state.lang !== state.defaultLang;
    var input = el(options.multiline ? "textarea" : "input", {
      type: "text",
      value: readText(owner[key]),
      placeholder: translating ? baseText(owner[key]) : options.placeholder || "",
      oninput: function () {
        owner[key] = writeText(owner[key], this.value);
        changed();
      },
    });
    if (options.multiline) input.value = readText(owner[key]);

    return el(
      "div",
      { class: "ed-field" + (translating ? " ed-field--translating" : "") },
      [el("label", { class: "ed-field__label", text: label }), input]
    );
  }

  /** A non-translatable single-line input (URL, path, etc). */
  function plainField(label, owner, key, options) {
    options = options || {};
    var input = el("input", {
      type: options.type || "text",
      value: owner[key] == null ? "" : owner[key],
      placeholder: options.placeholder || "",
      oninput: function () {
        var value = this.value;
        if (options.type === "number") {
          owner[key] = value === "" ? null : Number(value);
        } else {
          owner[key] = value === "" && options.nullable ? null : value;
        }
        changed();
      },
    });
    if (options.step) input.setAttribute("step", options.step);
    return el("div", { class: "ed-field" }, [
      el("label", { class: "ed-field__label", text: label }),
      input,
    ]);
  }

  function colorField(label, owner, key) {
    var text = el("input", {
      type: "text",
      value: owner[key] || "",
      oninput: function () {
        owner[key] = this.value;
        if (/^#[0-9a-fA-F]{6}$/.test(this.value)) picker.value = this.value;
        changed();
      },
    });
    var picker = el("input", {
      type: "color",
      value: /^#[0-9a-fA-F]{6}$/.test(owner[key] || "") ? owner[key] : "#ffffff",
      oninput: function () {
        owner[key] = this.value;
        text.value = this.value;
        changed();
      },
    });
    return el("div", { class: "ed-field" }, [
      el("label", { class: "ed-field__label", text: label }),
      el("div", { class: "ed-color" }, [picker, text]),
    ]);
  }

  function selectField(label, owner, key, options, onChange) {
    var select = el("select", {
      onchange: function () {
        owner[key] = this.value === "" ? null : this.value;
        if (onChange) onChange(this.value);
        else changed();
      },
    });
    options.forEach(function (option) {
      var value = typeof option === "string" ? option : option.value;
      var text = typeof option === "string" ? option : option.label;
      var node = el("option", { value: value, text: text });
      if (String(owner[key]) === String(value)) node.selected = true;
      select.appendChild(node);
    });
    return el("div", { class: "ed-field" }, [
      el("label", { class: "ed-field__label", text: label }),
      select,
    ]);
  }

  /**
   * Image field. You can type a path directly or choose a file to upload.
   * The server saves uploaded files under assets/ and returns the relative path.
   */
  function imageField(label, owner, key, kind) {
    var path = el("input", {
      type: "text",
      value: owner[key] == null ? "" : owner[key],
      placeholder: "assets/avatar.jpg",
      oninput: function () {
        owner[key] = this.value === "" ? null : this.value;
        refreshThumb();
        changed();
      },
    });

    var thumb = el("img", { class: "ed-thumb", alt: "" });
    function refreshThumb() {
      if (owner[key]) {
        // Append a timestamp so the cache doesn't keep serving the old photo.
        thumb.src = "/preview/" + owner[key] + "?t=" + Date.now();
        thumb.hidden = false;
      } else {
        thumb.removeAttribute("src");
        thumb.hidden = true;
      }
    }
    refreshThumb();

    var file = el("input", {
      type: "file",
      accept: "image/png,image/jpeg,image/gif,image/webp,image/svg+xml",
      onchange: function () {
        var chosen = this.files && this.files[0];
        if (!chosen) return;
        var input = this;

        setStatus(t("editor.status.uploading"));
        fetch(
          "/api/upload?kind=" + encodeURIComponent(kind || "image") +
            "&name=" + encodeURIComponent(chosen.name),
          { method: "POST", body: chosen }
        )
          .then(function (r) {
            return r.json();
          })
          .then(function (result) {
            if (result.error) {
              setStatus(msg(result.error), true);
              return;
            }
            owner[key] = result.path;
            path.value = result.path;
            refreshThumb();
            changed();
            setStatus(t("editor.status.uploaded"));
          })
          .catch(function (err) {
            setStatus(t("editor.status.uploadFailed", { message: err.message }), true);
          })
          .finally(function () {
            input.value = "";
          });
      },
    });

    var clear = el("button", {
      class: "ed-btn ed-btn--sm ed-btn--danger",
      type: "button",
      text: t("editor.clear"),
      onclick: function () {
        owner[key] = null;
        path.value = "";
        refreshThumb();
        changed();
      },
    });

    return el("div", { class: "ed-field" }, [
      el("label", { class: "ed-field__label", text: label }),
      el("div", { class: "ed-image" }, [
        thumb,
        el("div", { class: "ed-image__controls" }, [
          path,
          el("div", { class: "ed-image__buttons" }, [file, clear]),
        ]),
      ]),
    ]);
  }

  function checkField(label, owner, key) {
    var input = el("input", {
      type: "checkbox",
      onchange: function () {
        owner[key] = this.checked;
        changed();
      },
    });
    input.checked = !!owner[key];
    return el("label", { class: "ed-check" }, [input, document.createTextNode(label)]);
  }

  function group(title, open, children) {
    var details = el("details", { class: "ed-group" }, [
      el("summary", { text: title }),
      el("div", { class: "ed-group__body" }, children),
    ]);
    if (open) details.open = true;
    return details;
  }

  function row(children) {
    return el("div", { class: "ed-row" }, children);
  }

  /** Box wrapping a single item. Comes with move up/down and delete. */
  function itemBox(title, list, index, children, extraButtons) {
    var head = el("div", { class: "ed-item__head" }, [
      el("span", { class: "ed-item__title", text: title }),
    ]);
    (extraButtons || []).forEach(function (button) {
      head.appendChild(button);
    });
    head.appendChild(
      el("button", {
        class: "ed-btn ed-btn--sm",
        type: "button",
        text: "↑",
        title: t("editor.moveUp"),
        onclick: function () {
          if (index === 0) return;
          update(function () {
            var moved = list.splice(index, 1)[0];
            list.splice(index - 1, 0, moved);
          });
        },
      })
    );
    head.appendChild(
      el("button", {
        class: "ed-btn ed-btn--sm",
        type: "button",
        text: "↓",
        title: t("editor.moveDown"),
        onclick: function () {
          if (index >= list.length - 1) return;
          update(function () {
            var moved = list.splice(index, 1)[0];
            list.splice(index + 1, 0, moved);
          });
        },
      })
    );
    head.appendChild(
      el("button", {
        class: "ed-btn ed-btn--sm ed-btn--danger",
        type: "button",
        text: t("editor.delete"),
        onclick: function () {
          if (!confirm(t("editor.confirmDelete", { title: title }))) return;
          update(function () {
            list.splice(index, 1);
          });
        },
      })
    );

    var box = el("div", { class: "ed-item" }, [head].concat(children));
    if (list[index] && list[index].enabled === false) box.className += " ed-item--off";
    return box;
  }

  // ─── Layout ───────────────────────────────────────────────────────────────

  function render() {
    var config = state.config;
    panel.textContent = "";

    panel.appendChild(renderProfile(config));
    panel.appendChild(renderSocials(config));
    panel.appendChild(renderSections(config));
    panel.appendChild(renderTheme(config));
    panel.appendChild(renderSite(config));
  }

  function renderProfile(config) {
    var p = config.profile;
    return group(t("editor.group.profile"), true, [
      textField(t("editor.profile.name"), p, "name"),
      textField(t("editor.profile.tagline"), p, "tagline"),
      textField(t("editor.profile.location"), p, "location"),
      textField(t("editor.profile.bio"), p, "bio", { multiline: true }),
      imageField(t("editor.profile.avatar"), p, "avatar", "avatar"),
    ]);
  }

  function renderSocials(config) {
    var list = config.socials || (config.socials = []);
    var children = list.map(function (social, i) {
      return itemBox(social.platform, list, i, [
        row([
          selectField(t("editor.social.platform"), social, "platform", state.meta.platforms),
          plainField(t("editor.social.url"), social, "url"),
        ]),
        // Always shown regardless of platform. Hiding it unless custom is
        // selected would leave anyone who wants a real logo instead of the
        // built-in glyph unaware the field even exists.
        plainField(t("editor.social.icon"), social, "icon", { nullable: true }),
      ]);
    });

    children.push(
      el("div", { class: "ed-add" }, [
        el("button", {
          class: "ed-btn",
          type: "button",
          text: t("editor.social.add"),
          onclick: function () {
            update(function () {
              list.push({ platform: "instagram", url: "https://" });
            });
          },
        }),
      ])
    );

    return group(t("editor.group.socials", { count: list.length }), false, children);
  }

  function renderSections(config) {
    var list = config.sections || (config.sections = []);
    var children = list.map(function (section, i) {
      var title = readText(section.title) || baseText(section.title) || section.type;

      var toggle = el("button", {
        class: "ed-btn ed-btn--sm",
        type: "button",
        text: section.enabled === false ? t("editor.hide") : t("editor.show"),
        title: t("editor.showTitle"),
        onclick: function () {
          update(function () {
            section.enabled = section.enabled === false;
          });
        },
      });

      return itemBox(
        section.type + " · " + title,
        list,
        i,
        [
          row([
            textField(t("editor.section.title"), section, "title"),
            plainField(t("editor.section.icon"), section, "icon", { nullable: true }),
          ]),
        ].concat(renderSectionBody(section)),
        [toggle]
      );
    });

    var picker = el("select", {}, []);
    state.meta.sectionTypes.forEach(function (type) {
      picker.appendChild(el("option", { value: type, text: type }));
    });

    children.push(
      el("div", { class: "ed-add" }, [
        picker,
        el("button", {
          class: "ed-btn",
          type: "button",
          text: t("editor.section.add"),
          onclick: function () {
            update(function () {
              list.push(newSection(picker.value));
            });
          },
        }),
      ])
    );

    return group(t("editor.group.sections", { count: list.length }), true, children);
  }

  function newSection(type) {
    var section = { type: type, title: "", enabled: true };
    if (type === "about") section.body = "";
    else if (type === "tags") section.items = [];
    else if (type === "checklist") {
      section.items = [];
      section.show_progress = true;
    } else section.items = [];
    return section;
  }

  function renderSectionBody(section) {
    if (section.type === "about") {
      return [textField(t("editor.section.body"), section, "body", { multiline: true })];
    }

    var items = section.items || (section.items = []);

    if (section.type === "tags") {
      /*
       * Tags come in two shapes.
       *   "Rust" or { ko: "요리" }             — no emoji
       *   { icon: "🍳", text: ... }            — with emoji
       * The editor shows both as one unified field, and clearing the emoji
       * reverts back to the plain shape. This keeps the TOML clean for tags
       * that don't use an emoji.
       */
      var nodes = items.map(function (_, i) {
        function isWithIcon(v) {
          return v && typeof v === "object" && typeof v.icon === "string";
        }

        var iconHolder = {
          get icon() {
            return isWithIcon(items[i]) ? items[i].icon : "";
          },
          set icon(next) {
            if (next) {
              items[i] = isWithIcon(items[i])
                ? Object.assign({}, items[i], { icon: next })
                : { icon: next, text: items[i] };
            } else if (isWithIcon(items[i])) {
              items[i] = items[i].text;
            }
          },
        };

        var textHolder = {
          get value() {
            return isWithIcon(items[i]) ? items[i].text : items[i];
          },
          set value(next) {
            if (isWithIcon(items[i])) items[i] = Object.assign({}, items[i], { text: next });
            else items[i] = next;
          },
        };

        var icon = el("input", {
          type: "text",
          value: iconHolder.icon,
          placeholder: "🍳",
          maxlength: "2",
          oninput: function () {
            iconHolder.icon = this.value;
            changed();
          },
        });

        return itemBox(t("editor.tag"), items, i, [
          row([
            el("div", { class: "ed-field" }, [
              el("label", { class: "ed-field__label", text: t("editor.tag.emoji") }),
              icon,
            ]),
            textField(t("editor.tag.text"), textHolder, "value"),
          ]),
        ]);
      });
      nodes.push(addButton(t("editor.tag.add"), items, ""));
      return nodes;
    }

    var nodes = items.map(function (item, i) {
      return itemBox(itemLabel(section.type, item), items, i, itemFields(section.type, item));
    });
    nodes.push(addButton(t("editor.addItem"), items, newItem(section.type)));
    return nodes;
  }

  function itemLabel(type, item) {
    if (type === "contact") return item.kind;
    if (type === "gallery") return readText(item.caption) || baseText(item.caption) || t("editor.item");
    return readText(item.title || item.text) || baseText(item.title || item.text) || t("editor.item");
  }

  function itemFields(type, item) {
    if (type === "timeline") {
      return [
        row([textField(t("editor.timeline.period"), item, "period"), textField(t("editor.timeline.subtitle"), item, "subtitle")]),
        textField(t("editor.timeline.title"), item, "title"),
        textField(t("editor.timeline.description"), item, "description", { multiline: true }),
        plainField(t("editor.timeline.url"), item, "url", { nullable: true }),
      ];
    }
    if (type === "checklist") {
      return [
        textField(t("editor.checklist.text"), item, "text"),
        row([textField(t("editor.checklist.date"), item, "date"), textField(t("editor.checklist.note"), item, "note")]),
        checkField(t("editor.checklist.done"), item, "done"),
      ];
    }
    if (type === "links") {
      return [
        textField(t("editor.link.title"), item, "title"),
        plainField(t("editor.link.url"), item, "url"),
        row([textField(t("editor.timeline.subtitle"), item, "subtitle"), textField(t("editor.link.badge"), item, "badge")]),
        imageField(t("editor.link.thumbnail"), item, "thumbnail", "thumb"),
        row([checkField(t("editor.show"), item, "enabled"), checkField(t("editor.link.highlight"), item, "highlight")]),
      ];
    }
    if (type === "gallery") {
      return [
        imageField(t("editor.gallery.image"), item, "src", "gallery"),
        textField(t("editor.gallery.caption"), item, "caption"),
        checkField(t("editor.show"), item, "enabled"),
      ];
    }
    // contact
    return [
      row([
        selectField(t("editor.contact.kind"), item, "kind", state.meta.contactKinds),
        textField(t("editor.contact.label"), item, "label"),
      ]),
      textField(t("editor.contact.value"), item, "value"),
    ];
  }

  function newItem(type) {
    if (type === "timeline") return { title: "", enabled: true };
    if (type === "checklist") return { text: "", done: false };
    if (type === "links") return { title: "", url: "https://", enabled: true };
    if (type === "gallery") return { src: "", enabled: true };
    return { kind: "email", value: "" };
  }

  function addButton(label, list, template) {
    return el("div", { class: "ed-add" }, [
      el("button", {
        class: "ed-btn",
        type: "button",
        text: label,
        onclick: function () {
          update(function () {
            list.push(typeof template === "object" ? JSON.parse(JSON.stringify(template)) : template);
          });
        },
      }),
    ]);
  }

  function renderTheme(config) {
    var theme = config.theme || (config.theme = {});
    var bg = theme.background || (theme.background = { type: "solid", color: "#ffffff" });
    var font = theme.font || (theme.font = {});
    var text = theme.text || (theme.text = {});

    var bgFields = [
      selectField(t("editor.theme.backgroundType"), bg, "type", state.meta.backgroundTypes, function (type) {
        update(function () {
          theme.background = defaultBackground(type);
        });
      }),
    ];
    if (bg.type === "solid") bgFields.push(colorField(t("editor.theme.color"), bg, "color"));
    if (bg.type === "gradient") {
      bgFields.push(row([colorField(t("editor.theme.from"), bg, "from"), colorField(t("editor.theme.to"), bg, "to")]));
      bgFields.push(plainField(t("editor.theme.angle"), bg, "angle", { type: "number" }));
    }
    if (bg.type === "pattern") {
      bgFields.push(selectField(t("editor.theme.pattern"), bg, "name", state.meta.patternNames));
      bgFields.push(row([colorField(t("editor.theme.patternBase"), bg, "color"), colorField(t("editor.theme.patternInk"), bg, "pattern_color")]));
      bgFields.push(plainField(t("editor.theme.patternSize"), bg, "size", { placeholder: "22px" }));
    }
    if (bg.type === "image") {
      bgFields.push(plainField(t("editor.theme.imagePath"), bg, "src"));
      bgFields.push(row([selectField(t("editor.theme.fit"), bg, "fit", state.meta.imageFits), plainField(t("editor.theme.position"), bg, "position")]));
      bgFields.push(row([plainField(t("editor.theme.blur"), bg, "blur", { placeholder: "0px" }), colorField(t("editor.theme.overlay"), bg, "overlay")]));
    }

    var decoration = theme.decoration;
    var decorSelect = selectField(
      t("editor.theme.decoration"),
      { current: decoration ? decoration.name || "custom" : "" },
      "current",
      [{ value: "", label: t("editor.none") }].concat(state.meta.decorationPresets),
      function (value) {
        update(function () {
          theme.decoration = value ? { type: "preset", name: value } : null;
        });
      }
    );

    var fontPreset = state.meta.fonts.filter(function (f) {
      return f.value === (font.preset || "system");
    })[0];
    var weightLabel = fontPreset && fontPreset.weights.length
      ? t("editor.theme.fontWeightHint", { weights: fontPreset.weights.join(", ") })
      : t("editor.theme.fontWeight");

    return group(t("editor.group.theme"), false, [
      colorField(t("editor.theme.accent"), theme, "accent"),
      el("div", { class: "ed-field" }, [
        el("label", { class: "ed-field__label", text: t("editor.theme.background") }),
      ]),
      row([]),
    ].concat(bgFields, [
      decorSelect,
      colorField(t("editor.theme.sheet"), theme.sheet || (theme.sheet = {}), "background"),
      colorField(t("editor.theme.card"), theme.card || (theme.card = {}), "background"),
      selectField(t("editor.theme.cardStyle"), theme.card, "style", state.meta.cardStyles),
      row([colorField(t("editor.theme.textHeading"), text, "heading"), colorField(t("editor.theme.textBody"), text, "body")]),
      row([colorField(t("editor.theme.textCardTitle"), text, "card_title"), colorField(t("editor.theme.textMuted"), text, "muted")]),
      selectField(t("editor.theme.fontBody"), font, "preset", fontOptions()),
      selectField(
        t("editor.theme.fontHeading"),
        font,
        "heading_preset",
        [{ value: "", label: t("editor.theme.fontSameAsBody") }].concat(fontOptions())
      ),
      plainField(weightLabel, font, "heading_weight", {
        type: "number",
      }),
      row([
        plainField(t("editor.theme.fontSize"), font, "base_size", { placeholder: "15px" }),
        plainField(t("editor.theme.lineHeight"), font, "line_height", { type: "number", step: "0.05" }),
      ]),
      plainField(t("editor.theme.letterSpacing"), font, "letter_spacing", { placeholder: "normal" }),
    ]));
  }

  function defaultBackground(type) {
    if (type === "gradient") return { type: "gradient", from: "#dff2fb", to: "#bfe6f7", angle: 165 };
    if (type === "pattern")
      return { type: "pattern", name: "dots", color: "#ffffff", pattern_color: "#e6f4fb", size: "22px" };
    if (type === "image")
      return { type: "image", src: "assets/bg.jpg", fit: "cover", position: "center", blur: "0px" };
    return { type: "solid", color: "#ffffff" };
  }

  function renderSite(config) {
    var site = config.site;
    var features = config.features || (config.features = {});
    var footer = config.footer || (config.footer = {});

    var langBoxes = state.meta.languages.map(function (lang) {
      var current = site.languages || [];
      var input = el("input", {
        type: "checkbox",
        onchange: function () {
          var checked = this.checked;
          update(function () {
            var list = site.languages || (site.languages = []);
            var at = list.indexOf(lang.value);
            if (checked && at < 0) list.push(lang.value);
            if (!checked && at >= 0) list.splice(at, 1);
            // The default language must always be included.
            if (list.indexOf(site.lang) < 0) list.unshift(site.lang);
          });
        },
      });
      input.checked = (site.languages || []).indexOf(lang.value) >= 0 || lang.value === site.lang;
      input.disabled = lang.value === site.lang;
      return el("label", { class: "ed-check" }, [input, document.createTextNode(lang.label)]);
    });

    return group(t("editor.group.site"), false, [
      textField(t("editor.site.title"), site, "title"),
      textField(t("editor.site.description"), site, "description", { multiline: true }),
      plainField(t("editor.site.baseUrl"), site, "base_url", { nullable: true }),
      selectField(t("editor.site.lang"), site, "lang", state.meta.languages, function () {
        update(function () {
          var list = site.languages || (site.languages = []);
          if (list.indexOf(site.lang) < 0) list.unshift(site.lang);
          state.defaultLang = site.lang;
          state.lang = site.lang;
        });
        renderLangTabs();
      }),
      el("div", { class: "ed-field" }, [
        el("label", { class: "ed-field__label", text: t("editor.site.languages") }),
        el("div", {}, langBoxes),
      ]),
      checkField(t("editor.site.shareMenu"), features, "share_menu"),
      checkField(t("editor.site.vcard"), features, "vcard_download"),
      textField(t("editor.site.footerText"), footer, "text"),
      checkField(t("editor.site.poweredBy"), footer, "show_powered_by"),
    ]);
  }

  function renderLangTabs() {
    langTabs.textContent = "";
    var languages = state.config.site.languages && state.config.site.languages.length
      ? state.config.site.languages
      : [state.defaultLang];

    languages.forEach(function (code) {
      var meta = state.meta.languages.filter(function (l) {
        return l.value === code;
      })[0];
      var button = el("button", {
        class: "ed-lang",
        type: "button",
        text:
          code === state.defaultLang
            ? t("editor.langDefault", { name: meta ? meta.label : code })
            : meta
              ? meta.label
              : code,
        "aria-pressed": String(code === state.lang),
        onclick: function () {
          state.lang = code;
          renderLangTabs();
          render();
          reloadPreview();
        },
      });
      langTabs.appendChild(button);
    });
  }

  // ─── Diagnostics · Save ─────────────────────────────────────────────────

  function showDiagnostics(list) {
    // If the UI language changes these lines need to be redrawn too, so we
    // keep the last list around instead of asking the server again.
    state.diagnostics = list;
    diagnosticsEl.textContent = "";
    if (!list || !list.length) {
      diagnosticsEl.hidden = true;
      return;
    }
    diagnosticsEl.hidden = false;
    list.forEach(function (d) {
      diagnosticsEl.appendChild(
        el("div", { class: "ed-diag ed-diag--" + d.severity }, [
          el("span", { class: "ed-diag__tag", text: d.severity === "error" ? t("editor.diag.error") : t("editor.diag.warning") }),
          el("span", { class: "ed-diag__path", text: d.path }),
          el("span", { text: t(d.key, d.args) }),
        ])
      );
    });
  }

  function reloadPreview() {
    previewEl.src = "/preview/?lang=" + encodeURIComponent(state.lang) + "&t=" + Date.now();
  }

  function save() {
    setStatus(t("editor.status.saving"));
    fetch("/api/config", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(state.config),
    })
      .then(function (response) {
        return response.json();
      })
      .then(function (result) {
        if (result.error) {
          setStatus(msg(result.error), true);
          return;
        }
        showDiagnostics(result.diagnostics);
        if (result.saved) {
          state.dirty = false;
          setStatus(t("editor.status.saved"));
          reloadPreview();
        } else {
          setStatus(t("editor.status.notSaved"), true);
        }
      })
      .catch(function (err) {
        setStatus(t("editor.status.saveFailed", { message: err.message }), true);
      });
  }

  function build() {
    setStatus(t("editor.status.building"));
    fetch("/api/build", { method: "POST" })
      .then(function (r) {
        return r.json();
      })
      .then(function (result) {
        if (result.error) setStatus(msg(result.error), true);
        else
          setStatus(
            t("editor.status.built", {
              dist: result.dist,
              languages: result.languages.join(", "),
            })
          );
      })
      .catch(function (err) {
        setStatus(t("editor.status.buildFailed", { message: err.message }), true);
      });
  }

  /**
   * Deploys to GitHub Pages. The server rebuilds automatically as part of this.
   *
   * If there are unsaved changes, we warn first — noticing something is
   * missing only after deploying makes it a hassle to undo.
   */
  function deploy() {
    if (state.dirty && !confirm(t("editor.confirmDeploy"))) {
      return;
    }

    setStatus(t("editor.status.uploading"));
    fetch("/api/deploy", { method: "POST" })
      .then(function (r) {
        return r.json();
      })
      .then(function (result) {
        if (result.error) {
          setStatus(msg(result.error), true);
          showDeployHelp(result.error);
          return;
        }
        setStatus(
          t("editor.status.deployed", {
            branch: result.branch,
            files: result.files,
          })
        );
        showDeployResult(result);
      })
      .catch(function (err) {
        setStatus(t("editor.status.deployFailed", { message: err.message }), true);
      });
  }

  /** Leaves the resulting URL in the diagnostics area after deploying. */
  function showDeployResult(result) {
    diagnosticsEl.textContent = "";
    diagnosticsEl.hidden = false;

    if (result.pagesUrl) {
      diagnosticsEl.appendChild(
        el("div", { class: "ed-diag" }, [
          el("span", { class: "ed-diag__tag", text: t("editor.diag.url") }),
          el("a", { href: result.pagesUrl, target: "_blank", text: result.pagesUrl }),
        ])
      );
    }
    if (result.settingsUrl) {
      diagnosticsEl.appendChild(
        el("div", { class: "ed-diag" }, [
          el("span", { class: "ed-diag__tag", text: t("editor.diag.hint") }),
          el("span", {
            text: t("editor.diag.pagesOff", { branch: result.branch }) + " ",
          }),
          el("a", { href: result.settingsUrl, target: "_blank", text: result.settingsUrl }),
        ])
      );
    }
  }

  /**
   * Appends guidance on what to do after a failure.
   *
   * The error text the server sends is still hardcoded in Korean, so we
   * match against that for now. If server messages become multi-language,
   * this comparison will need to switch to matching on the key instead.
   */
  // A one-line hint appended after a failed deploy.
  //
  // This used to look for Korean words in the error sentence. Once there
  // were three UI languages, that approach could find nothing in English or
  // Japanese. Now it matches on the key instead.
  function showDeployHelp(error) {
    var key = error && error.key;
    var hint = null;
    if (key === "msg.deploy.notARepository") {
      hint = t("editor.hint.notRepo");
    } else if (key === "msg.deploy.noRemote") {
      hint = t("editor.hint.noRemote");
    } else if (
      key === "msg.deploy.gitFailed" &&
      error.args &&
      error.args.step === "@msg.deploy.step.push"
    ) {
      hint = t("editor.hint.pushFailed");
    }
    if (!hint) return;

    diagnosticsEl.textContent = "";
    diagnosticsEl.hidden = false;
    diagnosticsEl.appendChild(
      el("div", { class: "ed-diag ed-diag--error" }, [
        el("span", { class: "ed-diag__tag", text: t("editor.diag.error") }),
        el("span", { text: msg(error) }),
      ])
    );
    diagnosticsEl.appendChild(
      el("div", { class: "ed-diag" }, [
        el("span", { class: "ed-diag__tag", text: t("editor.diag.fix") }),
        el("span", { text: hint }),
      ])
    );
  }


  // ─── GitHub connection ─────────────────────────────────────────────────────

  var githubDialog = document.getElementById("github-dialog");
  var githubBody = document.getElementById("github-body");

  /** Fetches the connection state and opens the dialog. */
  function openGithub() {
    githubBody.textContent = "";
    githubBody.appendChild(el("p", { text: t("editor.gh.checking") }));
    githubDialog.showModal();

    fetch("/api/github")
      .then(function (r) {
        return r.json();
      })
      .then(renderGithub)
      .catch(function (err) {
        renderGithub({ connected: false, error: err.message });
      });
  }

  function renderGithub(info) {
    githubBody.textContent = "";
    if (info.connected) renderConnected(info);
    else renderConnect(info);
  }

  /** Not connected yet. Collects a token. */
  function renderConnect(info) {
    githubBody.appendChild(el("h2", { text: t("editor.gh.connectTitle") }));

    if (info.error) {
      githubBody.appendChild(
        el("div", { class: "ed-dialog__error", text: msg(info.error) })
      );
    }

    githubBody.appendChild(
      el("p", {
        text: t("editor.gh.connectHelp", { scopes: info.scopes || "repo" }),
      })
    );

    var open = el("button", {
      class: "ed-btn",
      type: "button",
      text: t("editor.gh.openTokenPage"),
      onclick: function () {
        openExternal(info.tokenPageUrl);
      },
    });

    var field = el("input", {
      type: "password",
      placeholder: t("editor.gh.tokenPlaceholder"),
      autocomplete: "off",
    });
    field.style.width = "100%";
    field.style.padding = "7px 9px";
    field.style.border = "1px solid var(--ed-line)";
    field.style.borderRadius = "7px";
    field.style.font = "inherit";
    field.style.marginTop = "8px";

    githubBody.appendChild(open);
    githubBody.appendChild(field);
    githubBody.appendChild(
      el("p", {
        text: t("editor.gh.tokenStorage"),
      })
    );

    githubBody.appendChild(
      el("div", { class: "ed-dialog__actions" }, [
        el("button", {
          class: "ed-btn",
          type: "button",
          text: t("editor.close"),
          onclick: function () {
            githubDialog.close();
          },
        }),
        el("button", {
          class: "ed-btn ed-btn--primary",
          type: "button",
          text: t("editor.gh.connect"),
          onclick: function () {
            connectGithub(field.value);
          },
        }),
      ])
    );

    field.focus();
  }

  function connectGithub(token) {
    if (!token.trim()) return;

    githubBody.textContent = "";
    githubBody.appendChild(el("p", { text: t("editor.gh.connecting") }));

    fetch("/api/github/connect", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ token: token }),
    })
      .then(function (r) {
        return r.json();
      })
      .then(function (result) {
        if (result.error) {
          renderGithub({ connected: false, error: result.error, scopes: "repo", tokenPageUrl: "https://github.com/settings/tokens/new?scopes=repo" });
          return;
        }
        openGithub(); // Now connected, so move on to the repo list
      })
      .catch(function (err) {
        renderGithub({ connected: false, error: err.message });
      });
  }

  /** Connected. Choose or create a repository. */
  function renderConnected(info) {
    var account = info.account || {};

    githubBody.appendChild(el("h2", { text: t("editor.gh.repoTitle") }));

    var badge = el("div", { class: "ed-dialog__account" }, []);
    if (account.avatar_url) {
      badge.appendChild(el("img", { src: account.avatar_url, alt: "" }));
    }
    badge.appendChild(el("span", { text: "@" + (account.login || "") }));
    badge.appendChild(
      el("button", {
        class: "ed-btn ed-btn--sm",
        type: "button",
        text: t("editor.gh.disconnect"),
        onclick: disconnectGithub,
      })
    );
    badge.lastChild.style.marginLeft = "auto";
    githubBody.appendChild(badge);

    if (info.remote) {
      githubBody.appendChild(
        el("p", { text: t("editor.gh.currentRepo", { url: info.remote }) })
      );
    }

    var list = el("div", { class: "ed-repos" }, []);
    (info.repos || []).forEach(function (repo) {
      var current = info.remote === repo.clone_url;
      var row = el("button", {
        class: "ed-repo" + (current ? " ed-repo--current" : ""),
        type: "button",
        onclick: function () {
          chooseRepo({ clone_url: repo.clone_url });
        },
      }, [
        el("span", { text: repo.full_name }),
        el("span", {
          class: "ed-repo__tag",
          text:
            (repo.private ? t("editor.gh.private") : t("editor.gh.public")) +
            (current ? " · " + t("editor.gh.current") : ""),
        }),
      ]);
      list.appendChild(row);
    });

    if (!(info.repos || []).length) {
      list.appendChild(el("div", { class: "ed-repo", text: t("editor.gh.noRepos") }));
    }
    githubBody.appendChild(list);

    var newName = el("input", {
      type: "text",
      placeholder: t("editor.gh.newRepoName"),
    });
    newName.style.flex = "1";
    newName.style.padding = "7px 9px";
    newName.style.border = "1px solid var(--ed-line)";
    newName.style.borderRadius = "7px";
    newName.style.font = "inherit";

    var privateBox = el("input", { type: "checkbox" });
    // The card needs to be public for GitHub Pages to work for free.
    privateBox.checked = false;

    githubBody.appendChild(
      el("div", { class: "ed-row" }, [
        newName,
        el("button", {
          class: "ed-btn",
          type: "button",
          text: t("editor.gh.create"),
          onclick: function () {
            if (!newName.value.trim()) return;
            chooseRepo({ create: newName.value.trim(), private: privateBox.checked });
          },
        }),
      ])
    );

    githubBody.appendChild(
      el("label", { class: "ed-check" }, [privateBox, document.createTextNode(t("editor.gh.makePrivate"))])
    );

    githubBody.appendChild(
      el("div", { class: "ed-dialog__actions" }, [
        el("button", {
          class: "ed-btn ed-btn--primary",
          type: "button",
          text: t("editor.close"),
          onclick: function () {
            githubDialog.close();
          },
        }),
      ])
    );
  }

  function chooseRepo(body) {
    githubBody.textContent = "";
    githubBody.appendChild(el("p", { text: t("editor.gh.linking") }));

    fetch("/api/github/repo", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    })
      .then(function (r) {
        return r.json();
      })
      .then(function (result) {
        if (result.error) {
          openGithub();
          setStatus(result.error, true);
          return;
        }
        githubDialog.close();
        setStatus(t("editor.status.repoLinked"));
      })
      .catch(function (err) {
        setStatus(t("editor.status.connectFailed", { message: err.message }), true);
        githubDialog.close();
      });
  }

  function disconnectGithub() {
    fetch("/api/github/disconnect", { method: "POST" })
      .then(function () {
        openGithub();
      })
      .catch(function (err) {
        setStatus(t("editor.status.disconnectFailed", { message: err.message }), true);
      });
  }

  // ─── Startup ────────────────────────────────────────────────────────────────

  document.getElementById("save").addEventListener("click", save);
  document.getElementById("build").addEventListener("click", build);
  document.getElementById("deploy").addEventListener("click", deploy);
  document.getElementById("github").addEventListener("click", openGithub);
  document.getElementById("reload").addEventListener("click", reloadPreview);

  window.addEventListener("beforeunload", function (event) {
    if (!state.dirty) return;
    event.preventDefault();
    event.returnValue = "";
  });

  fetch("/api/config")
    .then(function (r) {
      return r.json();
    })
    .then(function (data) {
      if (data.error) {
        panel.textContent = msg(data.error);
        return;
      }
      state.config = data.config;
      state.meta = data.meta;
      state.defaultLang = data.config.site.lang;
      state.lang = state.defaultLang;

      // The UI language is independent of the card's language. This
      // machine's remembered value takes priority, falling back to the
      // browser's setting if there isn't one.
      var available = (data.meta.languages || []).map(function (l) {
        return l.value;
      });
      state.uiLang = initialUiLang(available);
      document.documentElement.lang = state.uiLang;

      applyStaticText();
      renderUiLangPicker();
      renderLangTabs();
      render();
      showDiagnostics(data.diagnostics);
      reloadPreview();
      setStatus("");
    })
    .catch(function (err) {
      panel.textContent = t("editor.status.loadFailed", { message: err.message });
    });
})();
