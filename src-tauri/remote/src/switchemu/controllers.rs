//! Player-1 input config for yuzu-family Switch emulators.
//!
//! # Why this is harder than RetroArch
//!
//! RetroArch takes *positional* button numbers, so
//! [`crate::retroarch::controllers`] can write a fixed table. The yuzu family
//! does not: each binding is a serialised param package that names the pad by
//! SDL GUID and carries the pad's **raw SDL joystick button/axis index** —
//! the runtime read path is `event.jbutton.button` straight off
//! `SDL_EVENT_JOYSTICK_BUTTON_DOWN` (eden
//! `src/input_common/drivers/sdl_driver.cpp:578-602`), not a normalised
//! gamepad enum. Those raw indices differ per device *and* per platform: an
//! Xbox pad's A is button 0 on Windows and Linux, but the Steam Deck's
//! built-in pad reports A as button **3**, and its D-pad as buttons 16-19
//! where the Xbox pad uses hat 0.
//!
//! Guessing a table would therefore write a config that parses cleanly and
//! silently does nothing — the exact failure mode this module exists to avoid.
//! So Drop resolves the raw indices the same way SDL itself does: by looking
//! the pad's GUID up in SDL's controller database, which is vendored beside
//! this file (`gamecontrollerdb.txt`, zlib licence, from
//! <https://github.com/mdqinc/SDL_GameControllerDB>). If the pad is not in the
//! database, Drop writes **nothing** rather than something plausible.
//!
//! # What gets written
//!
//! Every key is written together with its `\default` twin set to `false`.
//! Without the twin the emulator's string reader defaults `\default` to *true*
//! and throws the value away (`src/frontend_common/config.cpp:790-810`), which
//! is why a config that looks correct in a text editor can still boot to
//! keyboard defaults.
//!
//! The Switch→SDL button assignment is Eden's own default binding, not Drop's
//! invention: `NativeButton::A → SDL_GAMEPAD_BUTTON_EAST`,
//! `B → SOUTH`, `X → NORTH`, `Y → WEST` (`sdl_driver.cpp:1013-1032`). It is
//! positional, so the physical button in the Switch's A position acts as A.

use log::{info, warn};
use serde::Serialize;

/// SDL's controller database, vendored so binding resolution does not depend
/// on the host having SDL installed or on network access at launch time.
const GAMECONTROLLER_DB: &str = include_str!("gamecontrollerdb.txt");

/// The database's platform tag for the host Drop is running on.
#[cfg(target_os = "windows")]
const DB_PLATFORM: &str = "Windows";
#[cfg(target_os = "linux")]
const DB_PLATFORM: &str = "Linux";
#[cfg(target_os = "macos")]
const DB_PLATFORM: &str = "Mac OS X";
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
const DB_PLATFORM: &str = "Unknown";

/// How Drop obtained the pad's SDL GUID. Recorded in the launch trace because
/// a GUID that does not match the one SDL reports inside the emulator is the
/// one failure that leaves no trace anywhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuidSource {
    /// Read directly off the input backend, already in SDL GUID layout.
    Observed,
    /// Rebuilt from the pad's USB vendor/product id because the backend does
    /// not expose a GUID (Windows Gaming Input reports a nil UUID for anything
    /// it recognises as a gamepad). Bus is USB and version is zero, matching
    /// how the same crate builds it for non-WGI devices.
    DerivedFromVidPid,
}

/// The connected pad, as far as Drop can see it from its own process.
///
/// Built by the `process` crate (which owns the input backend) and passed in,
/// so this crate stays free of an input dependency.
#[derive(Debug, Clone, Serialize)]
pub struct PadIdentity {
    /// 32 lower-case hex chars, no dashes — `Common::UUID::RawString()` form,
    /// with bytes 2-3 (SDL's controller-name CRC) zeroed exactly as the
    /// emulator's `GetGUID` does (`sdl_driver.cpp:19-26`).
    pub guid: String,
    /// Ordinal among connected joysticks sharing this GUID
    /// (`sdl_driver.cpp:279-284`), *not* the device index.
    pub port: u32,
    /// OS-reported name, for logs only.
    pub name: String,
    pub guid_source: GuidSource,
}

// ── SDL mapping database ─────────────────────────────────────────────────

/// One resolved element of an SDL mapping line's right-hand side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SdlBinding {
    Button(u32),
    /// `hN.M`, where M is an `SDL_HAT_*` bit: 1=up, 2=right, 4=down, 8=left.
    Hat { hat: u32, mask: u32 },
    /// `aN`, `+aN`, `-aN`, `aN~`. `positive` is false for `-aN`, which is the
    /// half of the axis the input sits on; `~` inverts the whole axis.
    Axis {
        axis: u32,
        positive: bool,
        inverted: bool,
    },
}

/// The subset of an SDL mapping line Drop needs, resolved to raw indices.
#[derive(Debug, Clone, Default)]
pub struct PadMapping {
    /// Human name from the database entry, for logs.
    pub display_name: String,
    entries: Vec<(String, SdlBinding)>,
}

impl PadMapping {
    fn get(&self, element: &str) -> Option<SdlBinding> {
        self.entries
            .iter()
            .find(|(k, _)| k == element)
            .map(|(_, b)| *b)
    }
}

/// Parses one `aN` / `+aN` / `-aN` / `aN~` / `bN` / `hN.M` value.
fn parse_binding(raw: &str) -> Option<SdlBinding> {
    let mut s = raw.trim();
    let mut positive = true;
    if let Some(rest) = s.strip_prefix('+') {
        s = rest;
    } else if let Some(rest) = s.strip_prefix('-') {
        positive = false;
        s = rest;
    }
    let mut inverted = false;
    if let Some(rest) = s.strip_suffix('~') {
        inverted = true;
        s = rest;
    }

    if let Some(rest) = s.strip_prefix('b') {
        return rest.parse().ok().map(SdlBinding::Button);
    }
    if let Some(rest) = s.strip_prefix('a') {
        return rest.parse().ok().map(|axis| SdlBinding::Axis {
            axis,
            positive,
            inverted,
        });
    }
    if let Some(rest) = s.strip_prefix('h') {
        let (hat, mask) = rest.split_once('.')?;
        return Some(SdlBinding::Hat {
            hat: hat.parse().ok()?,
            mask: mask.parse().ok()?,
        });
    }
    None
}

/// Parses a `gamecontrollerdb.txt` line into `(guid, platform, mapping)`.
/// Returns `None` for comments, blanks and malformed lines.
fn parse_db_line(line: &str) -> Option<(String, String, PadMapping)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let mut fields = line.split(',');
    let guid = fields.next()?.trim().to_lowercase();
    let display_name = fields.next()?.trim().to_string();

    let mut platform = String::new();
    let mut entries = Vec::new();
    for field in fields {
        let Some((key, value)) = field.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key == "platform" {
            platform = value.trim().to_string();
            continue;
        }
        if let Some(binding) = parse_binding(value) {
            entries.push((key.to_string(), binding));
        }
    }
    Some((
        guid,
        platform,
        PadMapping {
            display_name,
            entries,
        },
    ))
}

/// SDL GUID layout: bytes 4-5 are the USB vendor id and bytes 8-9 the product
/// id, both little-endian, i.e. hex chars 8..12 and 16..20 of the raw string.
/// Returns `None` if `guid` is not 32 hex chars.
fn vid_pid(guid: &str) -> Option<(&str, &str)> {
    if guid.len() != 32 || !guid.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some((&guid[8..12], &guid[16..20]))
}

/// Looks `guid` up in SDL's database for the current platform.
///
/// Two tiers, and no third:
///
/// 1. **Exact GUID.** Unambiguous — this is what SDL itself does.
/// 2. **Same vendor + product**, but only when every candidate entry agrees on
///    the bindings Drop writes. Needed because the GUID's version and driver
///    bytes vary with how the pad is attached, and on Windows Drop has to
///    reconstruct the GUID with a zero version. If the candidates disagree
///    (the DualSense, for instance, maps differently under the kernel's
///    `hid-playstation` driver than under generic HID) this returns `None`
///    rather than picking one.
pub fn lookup_pad_mapping(guid: &str) -> Option<PadMapping> {
    let guid = guid.to_lowercase();
    let mut vid_pid_candidates: Vec<PadMapping> = Vec::new();
    let want = vid_pid(&guid);

    for line in GAMECONTROLLER_DB.lines() {
        let Some((db_guid, platform, mapping)) = parse_db_line(line) else {
            continue;
        };
        if platform != DB_PLATFORM {
            continue;
        }
        if db_guid == guid {
            return Some(mapping);
        }
        if let (Some(want), Some(got)) = (want, vid_pid(&db_guid))
            && want == got
        {
            vid_pid_candidates.push(mapping);
        }
    }

    let first = vid_pid_candidates.first()?;
    let unanimous = vid_pid_candidates.iter().all(|m| {
        SWITCH_BUTTON_ELEMENTS
            .iter()
            .map(|(_, e)| *e)
            .chain(STICK_ELEMENTS.iter().flat_map(|(_, x, y)| [*x, *y]))
            .all(|element| m.get(element) == first.get(element))
    });
    if !unanimous {
        warn!(
            "[SWITCHEMU] {} database entries share vendor/product with {guid} but disagree on \
             bindings — refusing to guess",
            vid_pid_candidates.len()
        );
        return None;
    }
    Some(first.clone())
}

// ── Switch button ↔ SDL element table ────────────────────────────────────

/// `(qt-config key suffix, SDL database element)`.
///
/// The pairing is Eden's own `GetDefaultButtonBinding`
/// (`sdl_driver.cpp:1013-1032`) plus the two triggers it handles as axes
/// (`sdl_driver.cpp:964-967`). SL/SR and the motion keys are deliberately
/// absent: they are Joy-Con-only and Drop configures a Pro Controller.
const SWITCH_BUTTON_ELEMENTS: &[(&str, &str)] = &[
    ("button_a", "b"),
    ("button_b", "a"),
    ("button_x", "y"),
    ("button_y", "x"),
    ("button_lstick", "leftstick"),
    ("button_rstick", "rightstick"),
    ("button_l", "leftshoulder"),
    ("button_r", "rightshoulder"),
    ("button_zl", "lefttrigger"),
    ("button_zr", "righttrigger"),
    ("button_plus", "start"),
    ("button_minus", "back"),
    ("button_dleft", "dpleft"),
    ("button_dup", "dpup"),
    ("button_dright", "dpright"),
    ("button_ddown", "dpdown"),
    ("button_home", "guide"),
    ("button_screenshot", "misc1"),
];

/// `(qt-config key suffix, X element, Y element)`.
const STICK_ELEMENTS: &[(&str, &str, &str)] = &[
    ("lstick", "leftx", "lefty"),
    ("rstick", "rightx", "righty"),
];

/// The SDL input engine's registered name (`src/input_common/main.cpp:94`).
const ENGINE: &str = "sdl";

/// Stick tuning. These are the emulator's own read-side defaults
/// (`src/input_common/input_poller.cpp:914-916`), written explicitly so a
/// stale value from an earlier config cannot linger.
const STICK_DEADZONE: &str = "0.150000";
const STICK_RANGE: &str = "0.950000";

/// `SDL_HAT_*` bit → the direction name the emulator's parser accepts
/// (`GetHatButtonName`, `sdl_driver.cpp:1226-1239`).
fn hat_direction(mask: u32) -> Option<&'static str> {
    match mask {
        1 => Some("up"),
        2 => Some("right"),
        4 => Some("down"),
        8 => Some("left"),
        _ => None,
    }
}

/// Serialises a param package. Pairs are `key:value` joined by `,`; the
/// emulator's parser is order-independent (the backing store is a hash map,
/// `src/common/param_package.h`) so the fixed order here is purely so that
/// two runs produce byte-identical output.
///
/// No escaping is applied: `$0`/`$1`/`$2` exist for `:`/`,`/`$`
/// (`src/common/param_package.cpp:18-28`) but none of the values Drop writes —
/// engine name, hex GUID, integers, decimals, `+`/`-`, hat directions — can
/// contain those characters.
fn param_package(pairs: &[(&str, String)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{k}:{v}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Builds the param package for a binding used as a *button*.
fn button_param(pad: &PadIdentity, binding: SdlBinding) -> Option<String> {
    let base = [
        ("engine", ENGINE.to_string()),
        ("guid", pad.guid.clone()),
        ("port", pad.port.to_string()),
    ];
    let mut pairs: Vec<(&str, String)> = base.to_vec();
    match binding {
        SdlBinding::Button(n) => pairs.push(("button", n.to_string())),
        SdlBinding::Hat { hat, mask } => {
            pairs.push(("hat", hat.to_string()));
            pairs.push(("direction", hat_direction(mask)?.to_string()));
        }
        // A trigger, or a pad whose face buttons are wired to an axis. Matches
        // BuildAnalogParamPackageForButton (sdl_driver.cpp:880-890).
        SdlBinding::Axis {
            axis,
            positive,
            inverted,
        } => {
            pairs.push(("axis", axis.to_string()));
            pairs.push(("threshold", "0.5".to_string()));
            let sign = if positive != inverted { "+" } else { "-" };
            pairs.push(("invert", sign.to_string()));
        }
    }
    Some(param_package(&pairs))
}

/// Builds the param package for an analog stick.
///
/// A stick must carry `axis_x` and `axis_y` and must *not* carry `button`, or
/// the device factory classifies it as a trigger instead
/// (`input_poller.cpp:1143-1183`). `invert_y` must be the literal `"+"` for a
/// non-inverted Y: the read side computes `inverted_y = (invert_y != "+")`
/// (`input_poller.cpp:938`), so anything else — including an absent key —
/// flips the stick.
fn stick_param(pad: &PadIdentity, x: SdlBinding, y: SdlBinding) -> Option<String> {
    let (SdlBinding::Axis {
        axis: axis_x,
        inverted: inv_x,
        ..
    }, SdlBinding::Axis {
        axis: axis_y,
        inverted: inv_y,
        ..
    }) = (x, y)
    else {
        return None;
    };
    Some(param_package(&[
        ("engine", ENGINE.to_string()),
        ("guid", pad.guid.clone()),
        ("port", pad.port.to_string()),
        ("axis_x", axis_x.to_string()),
        ("axis_y", axis_y.to_string()),
        ("deadzone", STICK_DEADZONE.to_string()),
        ("range", STICK_RANGE.to_string()),
        ("offset_x", "0.000000".to_string()),
        ("offset_y", "0.000000".to_string()),
        ("invert_x", if inv_x { "-" } else { "+" }.to_string()),
        ("invert_y", if inv_y { "-" } else { "+" }.to_string()),
    ]))
}

/// Emits `key=value` plus the `key\default=false` twin.
fn with_default_twin(out: &mut Vec<(String, String)>, key: &str, value: String) {
    out.push((key.to_string(), value));
    out.push((format!("{key}\\default"), "false".to_string()));
}

/// Builds every `[Controls]` entry Drop owns for player 0.
///
/// Returns an empty vec if the mapping yields no usable binding at all, so the
/// caller can skip the write instead of stamping a half-configured player over
/// whatever the user had.
pub fn build_player0_entries(pad: &PadIdentity, mapping: &PadMapping) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut bound = 0usize;

    for (suffix, element) in SWITCH_BUTTON_ELEMENTS {
        let key = format!("player_0_{suffix}");
        let Some(binding) = mapping.get(element) else {
            continue;
        };
        let Some(value) = button_param(pad, binding) else {
            continue;
        };
        with_default_twin(&mut out, &key, value);
        bound += 1;
    }

    for (suffix, x_element, y_element) in STICK_ELEMENTS {
        let (Some(x), Some(y)) = (mapping.get(x_element), mapping.get(y_element)) else {
            continue;
        };
        let Some(value) = stick_param(pad, x, y) else {
            continue;
        };
        with_default_twin(&mut out, &format!("player_0_{suffix}"), value);
        bound += 1;
    }

    if bound == 0 {
        return Vec::new();
    }

    // Pro Controller is index 0 of the ControllerType enum
    // (`src/common/settings_input.h:369-381`). Handheld (4) is deliberately
    // avoided: the config reader turns docked mode off when player 0 is
    // Handheld (`src/frontend_common/config.cpp`, ReadControlValues).
    with_default_twin(&mut out, "player_0_type", "0".to_string());
    with_default_twin(&mut out, "player_0_connected", "true".to_string());

    out
}

/// Logs a one-line summary of what a binding set covers.
pub fn log_binding_summary(pad: &PadIdentity, mapping: &PadMapping, entries: &[(String, String)]) {
    info!(
        "[SWITCHEMU] Bound {} key(s) for '{}' (db entry '{}', guid {} via {:?}, port {})",
        entries.len(),
        pad.name,
        mapping.display_name,
        pad.guid,
        pad.guid_source,
        pad.port,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad() -> PadIdentity {
        PadIdentity {
            guid: "030000005e0400008e02000020200000".into(),
            port: 0,
            name: "Test Pad".into(),
            guid_source: GuidSource::Observed,
        }
    }

    fn mapping_from(line: &str) -> PadMapping {
        parse_db_line(line).expect("line parses").2
    }

    #[test]
    fn parses_every_binding_form() {
        assert_eq!(parse_binding("b3"), Some(SdlBinding::Button(3)));
        assert_eq!(parse_binding("h0.4"), Some(SdlBinding::Hat { hat: 0, mask: 4 }));
        assert_eq!(
            parse_binding("a2"),
            Some(SdlBinding::Axis {
                axis: 2,
                positive: true,
                inverted: false
            })
        );
        assert_eq!(
            parse_binding("-a1"),
            Some(SdlBinding::Axis {
                axis: 1,
                positive: false,
                inverted: false
            })
        );
        assert_eq!(
            parse_binding("a3~"),
            Some(SdlBinding::Axis {
                axis: 3,
                positive: true,
                inverted: true
            })
        );
        assert_eq!(parse_binding("nonsense"), None);
    }

    #[test]
    fn param_package_matches_upstream_shape() {
        let m = mapping_from(
            "030000005e0400008e02000020200000,Test,a:b0,b:b1,x:b2,y:b3,\
             dpup:h0.1,lefttrigger:a2,leftx:a0,lefty:a1,platform:Linux,",
        );
        let p = pad();

        // Switch A is the EAST face button, which the database calls "b".
        let a = button_param(&p, m.get("b").unwrap()).unwrap();
        assert_eq!(
            a,
            "engine:sdl,guid:030000005e0400008e02000020200000,port:0,button:1"
        );

        let dup = button_param(&p, m.get("dpup").unwrap()).unwrap();
        assert_eq!(
            dup,
            "engine:sdl,guid:030000005e0400008e02000020200000,port:0,hat:0,direction:up"
        );

        let zl = button_param(&p, m.get("lefttrigger").unwrap()).unwrap();
        assert_eq!(
            zl,
            "engine:sdl,guid:030000005e0400008e02000020200000,port:0,axis:2,threshold:0.5,invert:+"
        );

        let stick = stick_param(&p, m.get("leftx").unwrap(), m.get("lefty").unwrap()).unwrap();
        assert_eq!(
            stick,
            "engine:sdl,guid:030000005e0400008e02000020200000,port:0,axis_x:0,axis_y:1,\
             deadzone:0.150000,range:0.950000,offset_x:0.000000,offset_y:0.000000,\
             invert_x:+,invert_y:+"
        );
    }

    #[test]
    fn every_key_gets_a_default_twin() {
        let m = mapping_from(
            "030000005e0400008e02000020200000,Test,a:b0,b:b1,leftx:a0,lefty:a1,platform:Linux,",
        );
        let entries = build_player0_entries(&pad(), &m);
        assert!(!entries.is_empty());
        for (key, _) in entries.iter().filter(|(k, _)| !k.ends_with("\\default")) {
            assert!(
                entries.iter().any(|(k, v)| k == &format!("{key}\\default") && v == "false"),
                "{key} has no \\default=false twin"
            );
        }
        assert!(entries.iter().any(|(k, v)| k == "player_0_type" && v == "0"));
        assert!(entries.iter().any(|(k, v)| k == "player_0_connected" && v == "true"));
    }

    #[test]
    fn unmapped_pad_produces_nothing() {
        let m = mapping_from("030000005e0400008e02000020200000,Test,platform:Linux,");
        assert!(build_player0_entries(&pad(), &m).is_empty());
    }

    #[test]
    fn steam_deck_is_in_the_vendored_database() {
        // The Deck's internal pad is the case a hand-written positional table
        // gets wrong: Switch A sits on SDL button 4, not 1, and the D-pad is
        // buttons rather than a hat. Its entry is Linux-only, so on other hosts
        // assert the parse against the line itself instead of the lookup.
        let line = GAMECONTROLLER_DB
            .lines()
            .find(|l| l.starts_with("03000000de2800000512000010010000,"))
            .expect("Steam Deck entry missing from the vendored database");
        let (_, platform, m) = parse_db_line(line).unwrap();
        assert_eq!(platform, "Linux");
        assert_eq!(m.get("b"), Some(SdlBinding::Button(4)));
        assert_eq!(m.get("dpup"), Some(SdlBinding::Button(16)));

        let a = button_param(&pad(), m.get("b").unwrap()).unwrap();
        assert!(a.ends_with(",button:4"), "{a}");

        if DB_PLATFORM == "Linux" {
            assert!(lookup_pad_mapping("03000000de2800000512000010010000").is_some());
        }
    }

    #[test]
    fn unknown_guid_has_no_mapping() {
        assert!(lookup_pad_mapping("ffffffffffffffffffffffffffffffff").is_none());
    }
}
