//! Background decoration presets.
//!
//! These are purely decorative, so the renderer wraps them in
//! `aria-hidden="true"` and CSS applies `pointer-events: none`. Nothing here
//! should ever reach a screen reader — the artwork carries no meaning.
//!
//! To add a preset, add a variant to `DecorationPreset` and a matching branch
//! in each of the two functions below.

use maud::{Markup, PreEscaped};

use crate::config::DecorationPreset;

/// Decoration for the top of the screen. Based on `viewBox="0 0 400 190"`.
pub fn top(preset: DecorationPreset) -> Markup {
    let body = match preset {
        DecorationPreset::Confetti => {
            r##"<g fill="none" stroke-width="9" stroke-linecap="round"><path d="M8 96a46 46 0 0 1 92 0" stroke="#ff5b5b"/><path d="M22 96a32 32 0 0 1 64 0" stroke="#ffc93c"/><path d="M36 96a18 18 0 0 1 36 0" stroke="#4ec9e8"/></g><g fill="#ffd166"><circle cx="126" cy="34" r="11"/><circle cx="112" cy="52" r="11"/><circle cx="140" cy="52" r="11"/><circle cx="126" cy="70" r="11"/><circle cx="126" cy="52" r="8" fill="#fff6d8"/></g><path d="M300 22c14-16 40-6 40 14 0 18-24 32-40 46-16-14-40-28-40-46 0-20 26-30 40-14Z" fill="#ff5b7f"/><circle cx="300" cy="44" r="10" fill="#2fc4a8"/><path d="M370 8l6 16 17 1-13 11 4 17-14-9-15 9 4-17-13-11 17-1z" fill="#8a7bff"/><path d="M214 14l5 13 14 1-11 9 4 14-12-8-12 8 3-14-10-9 14-1z" fill="#4ec9e8"/><g><rect x="150" y="96" width="9" height="20" rx="3" fill="#ff8ab5" transform="rotate(24 154 106)"/><rect x="186" y="60" width="9" height="22" rx="3" fill="#ffc93c" transform="rotate(-38 190 71)"/><rect x="238" y="104" width="9" height="18" rx="3" fill="#2fc4a8" transform="rotate(52 242 113)"/><rect x="262" y="140" width="9" height="20" rx="3" fill="#8a7bff" transform="rotate(-18 266 150)"/><rect x="332" y="96" width="9" height="22" rx="3" fill="#4ec9e8" transform="rotate(40 336 107)"/><rect x="104" y="140" width="9" height="18" rx="3" fill="#ff5b7f" transform="rotate(-52 108 149)"/><rect x="358" y="150" width="9" height="20" rx="3" fill="#ffc93c" transform="rotate(16 362 160)"/><rect x="60" y="30" width="9" height="20" rx="3" fill="#2fc4a8" transform="rotate(-24 64 40)"/><circle cx="176" cy="132" r="6" fill="#8a7bff"/><circle cx="288" cy="96" r="6" fill="#ffc93c"/><circle cx="82" cy="168" r="6" fill="#4ec9e8"/></g>"##
        }
        DecorationPreset::Sparkle => {
            r##"<g fill="#ffd166"><path d="M60 20c3 18 8 23 26 26-18 3-23 8-26 26-3-18-8-23-26-26 18-3 23-8 26-26Z"/><path d="M330 34c2 13 6 17 19 19-13 2-17 6-19 19-2-13-6-17-19-19 13-2 17-6 19-19Z"/></g><g fill="#8a7bff"><path d="M200 8c3 20 9 26 29 29-20 3-26 9-29 29-3-20-9-26-29-29 20-3 26-9 29-29Z"/></g><g fill="#4ec9e8"><path d="M132 96c2 11 5 14 16 16-11 2-14 5-16 16-2-11-5-14-16-16 11-2 14-5 16-16Z"/><path d="M286 118c2 11 5 14 16 16-11 2-14 5-16 16-2-11-5-14-16-16 11-2 14-5 16-16Z"/></g><g fill="#ff8ab5"><path d="M376 100c2 9 4 11 13 13-9 2-11 4-13 13-2-9-4-11-13-13 9-2 11-4 13-13Z"/><path d="M22 128c2 9 4 11 13 13-9 2-11 4-13 13-2-9-4-11-13-13 9-2 11-4 13-13Z"/></g><g fill="#2fc4a8"><circle cx="248" cy="58" r="5"/><circle cx="96" cy="160" r="5"/><circle cx="352" cy="164" r="5"/></g>"##
        }
        DecorationPreset::Bubble => {
            r##"<g fill="#7cc7f0" opacity="0.55"><circle cx="46" cy="52" r="34"/><circle cx="150" cy="26" r="20"/><circle cx="330" cy="60" r="42"/><circle cx="246" cy="30" r="16"/></g><g fill="#a8e0f5" opacity="0.7"><circle cx="100" cy="118" r="24"/><circle cx="286" cy="132" r="18"/><circle cx="376" cy="130" r="26"/></g><g fill="#ffffff" opacity="0.6"><circle cx="36" cy="42" r="10"/><circle cx="320" cy="48" r="13"/><circle cx="92" cy="110" r="7"/></g><g fill="#ffd166" opacity="0.8"><circle cx="196" cy="96" r="9"/><circle cx="60" cy="150" r="7"/></g>"##
        }
    };

    PreEscaped(body.to_string())
}

/// Decoration for the bottom of the screen. Based on `viewBox="0 0 400 150"`.
pub fn bottom(preset: DecorationPreset) -> Markup {
    let body = match preset {
        DecorationPreset::Confetti => {
            r##"<g fill="none" stroke-width="9" stroke-linecap="round"><path d="M310 142a44 44 0 0 1 88 0" stroke="#ff5b5b"/><path d="M324 142a30 30 0 0 1 60 0" stroke="#ffc93c"/><path d="M338 142a16 16 0 0 1 32 0" stroke="#2fc4a8"/></g><path d="M40 92c9-10 26-4 26 9 0 12-15 21-26 30-11-9-26-18-26-30 0-13 17-19 26-9Z" fill="#7cc7f0"/><g fill="#ffd166"><circle cx="96" cy="112" r="10"/><circle cx="84" cy="128" r="10"/><circle cx="108" cy="128" r="10"/><circle cx="96" cy="128" r="7" fill="#fff6d8"/></g><g><rect x="150" y="110" width="9" height="20" rx="3" fill="#8a7bff" transform="rotate(-28 154 120)"/><rect x="196" y="124" width="9" height="18" rx="3" fill="#ff8ab5" transform="rotate(44 200 133)"/><rect x="248" y="104" width="9" height="22" rx="3" fill="#2fc4a8" transform="rotate(-14 252 115)"/><rect x="278" y="128" width="9" height="18" rx="3" fill="#ffc93c" transform="rotate(30 282 137)"/><circle cx="172" cy="138" r="6" fill="#4ec9e8"/><circle cx="226" cy="134" r="6" fill="#ffc93c"/><path d="M132 96l5 13 14 1-11 9 4 14-12-8-12 8 3-14-10-9 14-1z" fill="#ff5b7f"/></g>"##
        }
        DecorationPreset::Sparkle => {
            r##"<g fill="#ffd166"><path d="M340 84c3 18 8 23 26 26-18 3-23 8-26 26-3-18-8-23-26-26 18-3 23-8 26-26Z"/></g><g fill="#8a7bff"><path d="M64 92c2 13 6 17 19 19-13 2-17 6-19 19-2-13-6-17-19-19 13-2 17-6 19-19Z"/></g><g fill="#4ec9e8"><path d="M196 104c2 11 5 14 16 16-11 2-14 5-16 16-2-11-5-14-16-16 11-2 14-5 16-16Z"/></g><g fill="#ff8ab5"><path d="M268 118c2 9 4 11 13 13-9 2-11 4-13 13-2-9-4-11-13-13 9-2 11-4 13-13Z"/><path d="M128 126c2 9 4 11 13 13-9 2-11 4-13 13-2-9-4-11-13-13 9-2 11-4 13-13Z"/></g><g fill="#2fc4a8"><circle cx="24" cy="126" r="5"/><circle cx="300" cy="96" r="5"/><circle cx="384" cy="136" r="5"/></g>"##
        }
        DecorationPreset::Bubble => {
            r##"<g fill="#7cc7f0" opacity="0.55"><circle cx="356" cy="108" r="40"/><circle cx="250" cy="136" r="22"/><circle cx="52" cy="120" r="34"/></g><g fill="#a8e0f5" opacity="0.7"><circle cx="150" cy="130" r="26"/><circle cx="300" cy="142" r="18"/></g><g fill="#ffffff" opacity="0.6"><circle cx="344" cy="96" r="12"/><circle cx="42" cy="110" r="9"/></g><g fill="#ffd166" opacity="0.8"><circle cx="206" cy="122" r="8"/><circle cx="108" cy="142" r="6"/></g>"##
        }
    };

    PreEscaped(body.to_string())
}
