/*
 * Card interactions — saving a contact (vCard), sharing the page, sharing a link.
 *
 * Written as progressive enhancement. Without JS the buttons stay hidden via
 * CSS, and links/contacts still work as plain <a> tags. If share_menu /
 * vcard_download under [features] are false, the renderer never outputs the
 * corresponding button in the first place, so we don't need to check for
 * their presence here.
 */
(function () {
  "use strict";

  document.documentElement.classList.remove("no-js");
  document.documentElement.classList.add("js");

  var toast = document.querySelector(".pf-toast");
  var toastTimer = null;

  /*
   * UI strings. The renderer picks out just the current language's strings
   * and drops them into #pf-strings. Hardcoding Korean in the script would
   * mean the toasts show up in Korean even on the English version of the page.
   */
  var strings = (function () {
    var node = document.getElementById("pf-strings");
    if (!node) return {};
    try {
      return JSON.parse(node.textContent);
    } catch (err) {
      return {};
    }
  })();

  function t(key, fallback) {
    return strings[key] || fallback;
  }

  function showToast(message) {
    if (!toast) return;
    toast.textContent = message;
    toast.classList.add("pf-toast--visible");
    clearTimeout(toastTimer);
    toastTimer = setTimeout(function () {
      toast.classList.remove("pf-toast--visible");
    }, 1800);
  }

  function copyToClipboard(text) {
    if (navigator.clipboard && window.isSecureContext) {
      return navigator.clipboard.writeText(text);
    }
    // The Clipboard API isn't available on file:// or http://.
    return new Promise(function (resolve, reject) {
      var area = document.createElement("textarea");
      area.value = text;
      area.setAttribute("readonly", "");
      area.style.position = "fixed";
      area.style.opacity = "0";
      document.body.appendChild(area);
      area.select();
      var ok = document.execCommand && document.execCommand("copy");
      document.body.removeChild(area);
      ok ? resolve() : reject(new Error("copy unsupported"));
    });
  }

  /** Uses the OS share sheet on mobile, otherwise copies to clipboard. */
  function shareOrCopy(title, url) {
    if (navigator.share) {
      navigator.share({ title: title, url: url }).catch(function () {
        /* User canceled — nothing to report. */
      });
      return;
    }
    copyToClipboard(url).then(
      function () {
        showToast(t("toast.copied", "링크를 복사했어요"));
      },
      function () {
        showToast(t("toast.copy_failed", "복사에 실패했어요"));
      }
    );
  }

  // ─── vCard ────────────────────────────────────────────────────────────────

  function readVcardData() {
    var node = document.getElementById("pf-vcard-data");
    if (!node) return null;
    try {
      return JSON.parse(node.textContent);
    } catch (err) {
      return null;
    }
  }

  /**
   * Builds a vCard 3.0 text blob.
   *
   * CRLF line endings and escaping semicolons/commas are spec requirements.
   * Using LF alone can leave iOS Contacts unable to open the file.
   */
  function buildVcard(data) {
    function esc(value) {
      return String(value).replace(/([\\;,])/g, "\\$1").replace(/\n/g, "\\n");
    }

    var lines = ["BEGIN:VCARD", "VERSION:3.0"];
    lines.push("FN:" + esc(data.name));
    // N is the structured name (family;given;...). Splitting rules for
    // Korean names are ambiguous, so we put the full name in the family-name
    // slot and let FN handle the display.
    lines.push("N:" + esc(data.name) + ";;;;");
    if (data.tagline) lines.push("TITLE:" + esc(data.tagline));
    if (data.email) lines.push("EMAIL;TYPE=INTERNET:" + esc(data.email));
    if (data.phone) lines.push("TEL;TYPE=CELL:" + esc(data.phone));
    if (data.url) lines.push("URL:" + esc(data.url));
    if (data.location) lines.push("ADR;TYPE=WORK:;;" + esc(data.location) + ";;;;");
    lines.push("END:VCARD");

    return lines.join("\r\n") + "\r\n";
  }

  function downloadVcard() {
    var data = readVcardData();
    if (!data || !data.name) {
      showToast(t("toast.vcard_failed", "연락처 정보를 읽을 수 없어요"));
      return;
    }

    // The BOM is needed so some Windows address books don't mangle Korean text.
    var blob = new Blob(["﻿" + buildVcard(data)], {
      type: "text/vcard;charset=utf-8",
    });
    var url = URL.createObjectURL(blob);

    var link = document.createElement("a");
    link.href = url;
    link.download = data.name + ".vcf";
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);

    // Revoking immediately could make the blob disappear before the download actually starts.
    setTimeout(function () {
      URL.revokeObjectURL(url);
    }, 1000);

    showToast(t("toast.vcard_saved", "연락처를 저장했어요"));
  }

  // ─── Events ───────────────────────────────────────────────────────────────

  document.addEventListener("click", function (event) {
    var action = event.target.closest("[data-action]");
    if (action) {
      event.preventDefault();
      if (action.dataset.action === "vcard") {
        downloadVcard();
      } else if (action.dataset.action === "share-page") {
        shareOrCopy(document.title, location.href);
      }
      return;
    }

    var shareButton = event.target.closest(".pf-card__share");
    if (shareButton) {
      event.preventDefault();
      var card = shareButton.closest(".pf-card");
      var link = card && card.querySelector(".pf-card__link");
      if (link) {
        // Use only the first text node so the subtitle/badge don't get pulled in too.
        var title = (link.firstChild && link.firstChild.textContent || "").trim();
        shareOrCopy(title || link.textContent.trim(), link.href);
      }
    }
  });

  // Exposed so the local editor UI's preview and tests can reuse these.
  // The page's own behavior doesn't depend on this object.
  window.profileit = {
    buildVcard: buildVcard,
    readVcardData: readVcardData,
  };
})();
