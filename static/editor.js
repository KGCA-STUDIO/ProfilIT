/*
 * ProfileIT 편집기.
 *
 * 설정 전체를 JSON 으로 들고 있다가 저장할 때 통째로 보냅니다. 서버가
 * toml_edit 으로 기존 파일에 병합하므로 주석은 보존됩니다.
 *
 * 번역 가능한 값(Text)은 문자열이거나 { ko: "...", en: "..." } 입니다.
 * 기본 언어를 고칠 때는 문자열 그대로 두고, 다른 언어를 처음 입력하는 순간
 * 표로 바꿉니다 — 번역하지 않은 항목의 TOML 을 어지럽히지 않기 위해서입니다.
 */
(function () {
  "use strict";

  var UI_LANG_KEY = "profileit.uiLang";

  var state = {
    config: null,
    meta: null,
    lang: null, // 지금 편집 중인 언어 (명함 내용)
    defaultLang: null,
    uiLang: null, // 편집기 화면 언어. 명함 언어와 별개입니다
    diagnostics: null, // 마지막 검증 결과. 화면 언어를 바꿀 때 다시 그립니다
    dirty: false,
  };

  var panel = document.getElementById("panel");

  // ─── 화면 문구 ────────────────────────────────────────────────────────────

  // 서버가 보낸 메시지를 화면 언어로 조립합니다.
  //
  // 서버는 완성된 문장이 아니라 `{key, args}` 를 보냅니다. 어느 언어로 읽을지
  // 정하는 쪽은 화면이기 때문입니다. 옛 형태(문자열)도 그대로 통과시킵니다.
  function msg(value) {
    if (!value) return "";
    if (typeof value === "string") return value;
    if (value.key) return t(value.key, value.args);
    return String(value);
  }

  // 폰트 목록. 서버는 이름 대신 키를 보내므로 여기서 화면 언어로 풉니다.
  function fontOptions() {
    return (state.meta.fonts || []).map(function (f) {
      return { value: f.value, label: t(f.labelKey), weights: f.weights };
    });
  }

  /**
   * 편집기 화면 문구.
   *
   * 명함 내용의 언어와 **별개**입니다. 한국어 화면으로 영어 명함을 쓰는 일이
   * 흔하기 때문입니다. 서버가 세 언어를 한 번에 주므로 전환에 왕복이 없습니다.
   */
  function t(key, args) {
    var table = (state.meta && state.meta.uiStrings && state.meta.uiStrings[state.uiLang]) || {};
    // 없는 키는 키 자체를 보여줍니다. 무엇이 빠졌는지 화면에서 바로 보입니다.
    var text = table[key] || key;

    Object.keys(args || {}).forEach(function (name) {
      var value = args[name];
      // "@키" 는 인자 자체가 번역 대상이라는 표시입니다 (예: "푸시" 실패).
      if (typeof value === "string" && value.charAt(0) === "@") {
        value = table[value.slice(1)] || value.slice(1);
      }
      text = text.split("{" + name + "}").join(value);
    });
    return text;
  }

  /** 이 PC 가 기억하는 화면 언어. 없으면 브라우저 설정을 따릅니다. */
  function initialUiLang(available) {
    var saved = null;
    try {
      saved = localStorage.getItem(UI_LANG_KEY);
    } catch (err) {
      // 저장소를 못 읽어도 기본값으로 굴러갑니다.
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
      // 기억하지 못해도 이번 실행은 문제없습니다.
    }
    document.documentElement.lang = code;
    applyStaticText();
    renderUiLangPicker();
    renderLangTabs();
    render();
    // 진단 줄도 서버 문구라 같이 갈아끼웁니다.
    if (state.diagnostics) showDiagnostics(state.diagnostics);
  }

  /** HTML 에 박혀 있는 문구를 현재 화면 언어로 채웁니다. */
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
   * 화면 언어 고르개.
   *
   * 명함 언어 탭과 나란히 두면 헷갈려서 오른쪽 끝에 작게 둡니다 — 한 번
   * 정하면 거의 바꾸지 않는 설정이기 때문입니다.
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

  // ─── 번역 가능한 값 ───────────────────────────────────────────────────────

  function readText(value) {
    if (value == null) return "";
    if (typeof value === "string") {
      // 평문은 모든 언어 공통입니다. 다른 언어 탭에서는 비워 두고,
      // placeholder 로 원문을 보여줘 번역할 대상을 알 수 있게 합니다.
      return state.lang === state.defaultLang ? value : "";
    }
    return value[state.lang] || "";
  }

  /** 기본 언어 원문. 번역 탭의 placeholder 로 씁니다. */
  function baseText(value) {
    if (value == null) return "";
    if (typeof value === "string") return value;
    return value[state.defaultLang] || "";
  }

  function writeText(value, next) {
    var isDefault = state.lang === state.defaultLang;

    if (value == null || typeof value === "string") {
      if (isDefault) return next;
      if (!next) return value; // 번역을 비우면 평문 그대로 둡니다
      var created = {};
      if (value) created[state.defaultLang] = value;
      created[state.lang] = next;
      return created;
    }

    var map = Object.assign({}, value);
    if (next) map[state.lang] = next;
    else delete map[state.lang];

    // 기본 언어 하나만 남으면 평문으로 되돌립니다.
    var keys = Object.keys(map);
    if (keys.length === 0) return "";
    if (keys.length === 1 && keys[0] === state.defaultLang) return map[state.defaultLang];
    return map;
  }

  // ─── 작은 DOM 도우미 ──────────────────────────────────────────────────────

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
   * 바깥 주소를 기본 브라우저로 엽니다.
   *
   * 데스크톱 앱의 웹뷰는 `window.open` 을 막습니다. 막지 않더라도 편집기 창이
   * 외부 사이트로 넘어가면 돌아올 방법이 마땅치 않아서, 서버에 부탁해 바깥
   * 브라우저로 보냅니다.
   */
  function openExternal(url) {
    if (!url) return;
    fetch("/api/open", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ url: url }),
    }).catch(function () {
      // 일반 브라우저에서 편집기를 열어둔 경우를 위한 대비책.
      window.open(url, "_blank");
    });
  }

  // target="_blank" 링크를 모두 가로챕니다. 새로 만드는 링크마다 따로
  // 처리하는 것보다, 한 곳에서 잡는 편이 빠뜨릴 여지가 없습니다.
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

  /** 값을 고치고 화면을 다시 그립니다. 구조가 바뀌는 편집에 씁니다. */
  function update(fn) {
    fn();
    changed();
    render();
  }

  // ─── 입력 위젯 ────────────────────────────────────────────────────────────

  /** 번역 가능한 한 줄 입력. */
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

  /** 번역하지 않는 한 줄 입력(URL, 경로 등). */
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
   * 이미지 필드. 경로를 직접 적을 수도 있고 파일을 골라 올릴 수도 있습니다.
   * 올린 파일은 서버가 assets/ 에 저장하고 상대 경로를 돌려줍니다.
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
        // 캐시 때문에 옛 사진이 남지 않도록 시각을 붙입니다.
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

  /** 항목 하나를 감싸는 상자. 위·아래 이동과 삭제가 붙습니다. */
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

  // ─── 화면 구성 ────────────────────────────────────────────────────────────

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
        // 플랫폼과 상관없이 늘 보여줍니다. custom 이 아닐 때 숨겨두면, 내장
        // 글리프 대신 진짜 로고를 넣고 싶은 사람이 칸이 있는 줄도 모릅니다.
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
       * 태그는 두 가지 모양입니다.
       *   "Rust" 또는 { ko: "요리" }           — 이모지 없음
       *   { icon: "🍳", text: ... }            — 이모지 있음
       * 편집기에서는 둘을 한 화면으로 보여주고, 이모지를 비우면 다시 앞쪽
       * 모양으로 되돌립니다. 이모지를 안 쓰는 태그의 TOML 을 어지럽히지
       * 않기 위해서입니다.
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
            // 기본 언어는 항상 포함되어야 합니다.
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

  // ─── 진단 · 저장 ──────────────────────────────────────────────────────────

  function showDiagnostics(list) {
    // 화면 언어를 바꾸면 이 줄들도 새 언어로 다시 그려야 합니다. 서버에 다시 묻지
    // 않아도 되도록 마지막 목록을 들고 있습니다.
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
   * GitHub Pages 로 올립니다. 서버가 빌드까지 알아서 다시 합니다.
   *
   * 저장하지 않은 변경이 있으면 먼저 알립니다 — 올린 뒤에야 빠진 것을
   * 알아차리면 되돌리기가 번거롭습니다.
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

  /** 올린 뒤 주소를 아래 진단 영역에 남깁니다. */
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
   * 실패했을 때 무엇을 해야 하는지 덧붙입니다.
   *
   * 서버가 보내는 오류 문구는 아직 한국어 고정이라 한국어로 맞춰봅니다.
   * 서버 메시지를 다국어로 만들면 이 비교도 키 기반으로 바꿔야 합니다.
   */
  // 배포 실패에 덧붙일 한 줄 안내.
  //
  // 예전에는 오류 문장에서 한국어 단어를 찾았습니다. 화면 언어가 셋이 되면서
  // 그 방식은 영어·일본어에서 아무것도 못 찾게 됩니다. 이제 키로 봅니다.
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


  // ─── GitHub 연결 ──────────────────────────────────────────────────────────

  var githubDialog = document.getElementById("github-dialog");
  var githubBody = document.getElementById("github-body");

  /** 연결 상태를 받아 대화상자를 엽니다. */
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

  /** 아직 연결 전. 토큰을 받습니다. */
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
        openGithub(); // 연결됐으니 저장소 목록으로 넘어갑니다
      })
      .catch(function (err) {
        renderGithub({ connected: false, error: err.message });
      });
  }

  /** 연결됨. 저장소를 고르거나 만듭니다. */
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
    // 명함은 공개해야 GitHub Pages 가 무료로 동작합니다.
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

  // ─── 시작 ─────────────────────────────────────────────────────────────────

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

      // 화면 언어는 명함 언어와 별개입니다. 이 PC 가 기억하는 값이 먼저이고,
      // 없으면 브라우저 설정을 따릅니다.
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
