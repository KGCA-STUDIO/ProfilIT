/*
 * 명함 인터랙션 — 연락처 저장(vCard), 페이지 공유, 링크 공유.
 *
 * 점진적 향상(progressive enhancement)으로 짰습니다. JS 가 없으면 버튼들은
 * CSS 에서 숨겨진 채 남고, 링크·연락처는 평범한 <a> 로 그대로 동작합니다.
 * [features] 의 share_menu / vcard_download 가 false 면 렌더러가 해당 버튼을
 * 아예 출력하지 않으므로, 여기서는 존재 여부를 따지지 않습니다.
 */
(function () {
  "use strict";

  document.documentElement.classList.remove("no-js");
  document.documentElement.classList.add("js");

  var toast = document.querySelector(".pf-toast");
  var toastTimer = null;

  /*
   * UI 문구. 렌더러가 현재 언어의 문구만 골라 #pf-strings 에 넣어줍니다.
   * 스크립트에 한국어를 박아두면 영어판에서 토스트만 한국어로 나옵니다.
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
    // file:// 이나 http:// 에서는 Clipboard API 를 쓸 수 없습니다.
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

  /** 모바일이면 OS 공유 시트, 아니면 클립보드 복사. */
  function shareOrCopy(title, url) {
    if (navigator.share) {
      navigator.share({ title: title, url: url }).catch(function () {
        /* 사용자가 취소한 경우 — 알릴 것이 없습니다. */
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
   * vCard 3.0 텍스트를 만듭니다.
   *
   * CRLF 줄바꿈과 세미콜론·콤마 이스케이프는 규격 요구사항입니다. LF 만 쓰면
   * iOS 연락처가 파일을 열지 못하는 경우가 있습니다.
   */
  function buildVcard(data) {
    function esc(value) {
      return String(value).replace(/([\\;,])/g, "\\$1").replace(/\n/g, "\\n");
    }

    var lines = ["BEGIN:VCARD", "VERSION:3.0"];
    lines.push("FN:" + esc(data.name));
    // N 은 구조화된 이름(성;이름;...)입니다. 한국어 이름은 분리 규칙이
    // 모호해서 성 자리에 전체를 넣고 FN 으로 표시하게 둡니다.
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

    // BOM 을 붙여야 일부 윈도우 주소록이 한글을 깨뜨리지 않습니다.
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

    // revoke 를 즉시 하면 다운로드가 시작되기 전에 blob 이 사라질 수 있습니다.
    setTimeout(function () {
      URL.revokeObjectURL(url);
    }, 1000);

    showToast(t("toast.vcard_saved", "연락처를 저장했어요"));
  }

  // ─── 이벤트 ───────────────────────────────────────────────────────────────

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
        // 부제·배지가 함께 들어가지 않도록 첫 텍스트 노드만 씁니다.
        var title = (link.firstChild && link.firstChild.textContent || "").trim();
        shareOrCopy(title || link.textContent.trim(), link.href);
      }
    }
  });

  // 로컬 편집 UI 의 프리뷰와 테스트에서 재사용할 수 있도록 노출합니다.
  // 페이지 동작 자체는 이 객체에 의존하지 않습니다.
  window.profileit = {
    buildVcard: buildVcard,
    readVcardData: readVcardData,
  };
})();
