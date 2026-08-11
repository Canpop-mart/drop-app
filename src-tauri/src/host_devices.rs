//! Host display and audio endpoint enumeration for Sunshine streaming.
//!
//! Two jobs, both feeding `sunshine.conf`:
//!
//! * **Displays** — which monitor Sunshine captures (`output_name`) and which
//!   GPU it captures it from (`adapter_name`).
//! * **Audio** — which endpoint Sunshine captures (`audio_sink`) and which
//!   virtual device it temporarily makes default (`virtual_sink`).
//!
//! Everything Windows-specific is behind `#[cfg(windows)]`; the Linux/Deck
//! build gets empty lists so the streaming code compiles and no-ops there
//! (Sunshine hosting is Windows-only in Drop today).

use serde::{Deserialize, Serialize};

// ── Public shapes ─────────────────────────────────────────────────────

/// One capturable display, as offered to the settings picker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DisplayEntry {
    /// Sunshine's own stable display identity, e.g.
    /// `{326cc288-6d34-54f5-8c7d-5cb7fa9e2a49}`. This is what goes into
    /// `output_name` and what Drop persists — see `display_device_id`.
    pub device_id: String,
    /// The GDI name, e.g. `\\.\DISPLAY1`. Shown for disambiguation only; it is
    /// **not** persisted, because it renumbers across reboots and replugs.
    pub display: String,
    /// GPU the display hangs off, e.g. `NVIDIA GeForce RTX 4070 Ti SUPER`.
    /// Empty when `dxgi-info.exe` could not be run.
    pub adapter: String,
    /// Current mode, e.g. `2560x1440`. Empty when unknown.
    pub resolution: String,
    /// Monitor name from its EDID, e.g. `DELL S2725DS`. Falls back to the GDI
    /// name when the driver has no friendly name.
    pub friendly_name: String,
    /// Whether this is the Windows primary display right now.
    pub primary: bool,
}

/// One active audio render endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AudioSinkEntry {
    /// The exact endpoint name Sunshine matches on, e.g.
    /// `Speakers (Realtek USB Audio)`.
    pub name: String,
    /// Whether Windows reports the endpoint as ready to play right now. A
    /// disabled or unplugged endpoint is still listed (so the picker is never
    /// empty) but is worth flagging in the UI.
    pub active: bool,
    /// Whether Windows currently plays through this endpoint.
    pub default: bool,
    /// Whether this looks like Steam's virtual streaming sink, i.e. the device
    /// that belongs in `virtual_sink` rather than `audio_sink`.
    pub virtual_sink: bool,
}

/// Substring that identifies Steam's virtual streaming sink.
///
/// The real endpoint name carries an output-form prefix — on this machine it is
/// `Speakers (Steam Streaming Speakers)` — so the full name has to be read off
/// the device rather than assumed. Sunshine matches `virtual_sink` literally,
/// and a name that is merely close is a silent no-op.
#[cfg(windows)]
const STEAM_VIRTUAL_SINK_MARKER: &str = "Steam Streaming Speakers";

// ── dxgi-info.exe parsing ─────────────────────────────────────────────

/// One `OUTPUT` block from `dxgi-info.exe`, carrying its adapter's name down.
#[cfg(any(windows, test))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DxgiOutput {
    pub adapter: String,
    pub display: String,
    pub resolution: String,
}

/// Parse `dxgi-info.exe` output into the attached outputs it lists.
///
/// The tool prints one `====== ADAPTER =====` block per DXGI adapter, each
/// followed by indented `====== OUTPUT ======` blocks:
///
/// ```text
/// ====== ADAPTER =====
/// Device Name      : NVIDIA GeForce RTX 4070 Ti SUPER
///     ====== OUTPUT ======
///     Output Name       : \\.\DISPLAY1
///     AttachedToDesktop : yes
///     Resolution        : 2560x1440
/// ```
///
/// Adapters with no outputs (a duplicate GPU entry, "Microsoft Basic Render
/// Driver") print an empty OUTPUT header and are skipped, as are outputs that
/// are not attached to the desktop.
#[cfg(any(windows, test))]
pub fn parse_dxgi_info(raw: &str) -> Vec<DxgiOutput> {
    /// The OUTPUT block currently being read. An adapter with no displays
    /// prints the header and nothing else, so `display` stays empty.
    #[derive(Default)]
    struct Pending {
        display: String,
        attached: bool,
        resolution: String,
    }

    let mut outputs: Vec<DxgiOutput> = Vec::new();
    let mut adapter = String::new();
    let mut pending: Option<Pending> = None;

    // A block only counts once its header has been left behind, so it is
    // committed at the next header or at end of input.
    fn commit(outputs: &mut Vec<DxgiOutput>, adapter: &str, pending: Option<Pending>) {
        let Some(p) = pending else { return };
        if p.attached && !p.display.is_empty() {
            outputs.push(DxgiOutput {
                adapter: adapter.to_string(),
                display: p.display,
                resolution: p.resolution,
            });
        }
    }

    for line in raw.lines() {
        let line = line.trim();
        if line.starts_with("======") && line.contains("ADAPTER") {
            commit(&mut outputs, &adapter, pending.take());
            adapter.clear();
            continue;
        }
        if line.starts_with("======") && line.contains("OUTPUT") {
            commit(&mut outputs, &adapter, pending.replace(Pending::default()));
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        // "Device Name" belongs to the adapter; output blocks say "Output
        // Name", so the two never collide.
        if key == "Device Name" {
            adapter = value.to_string();
            continue;
        }
        let Some(p) = pending.as_mut() else { continue };
        match key {
            "Output Name" => p.display = value.to_string(),
            "AttachedToDesktop" => p.attached = value.eq_ignore_ascii_case("yes"),
            "Resolution" => p.resolution = value.to_string(),
            _ => {}
        }
    }
    commit(&mut outputs, &adapter, pending);

    outputs
}

/// Run `dxgi-info.exe` and parse it. Returns an empty list on any failure —
/// the adapter name is a nice-to-have, and a missing tool must not stop the
/// display picker from listing displays.
#[cfg(windows)]
pub fn dxgi_outputs(tool: &std::path::Path) -> Vec<DxgiOutput> {
    if !tool.exists() {
        log::warn!(
            "[DISPLAY] {} is missing; display list will have no GPU names",
            tool.display()
        );
        return Vec::new();
    }
    let mut cmd = std::process::Command::new(tool);
    if let Some(dir) = tool.parent() {
        cmd.current_dir(dir);
    }
    hide_console(&mut cmd);
    match cmd.output() {
        Ok(out) => parse_dxgi_info(&String::from_utf8_lossy(&out.stdout)),
        Err(e) => {
            log::warn!("[DISPLAY] Failed to run {}: {e}", tool.display());
            Vec::new()
        }
    }
}

#[cfg(windows)]
fn hide_console(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

// ── Display enumeration ───────────────────────────────────────────────

/// List the displays Sunshine could capture, newest state each call.
///
/// `dxgi_tool` is the path to Sunshine's bundled `dxgi-info.exe`; it supplies
/// the GPU name and current mode, which `QueryDisplayConfig` does not.
pub fn list_displays(dxgi_tool: &std::path::Path) -> Result<Vec<DisplayEntry>, String> {
    #[cfg(windows)]
    {
        let dxgi = dxgi_outputs(dxgi_tool);
        let mut entries = windows_impl::enumerate_displays()?;
        for entry in &mut entries {
            if let Some(found) = dxgi.iter().find(|o| o.display == entry.display) {
                entry.adapter = found.adapter.clone();
                entry.resolution = found.resolution.clone();
            }
        }
        Ok(entries)
    }
    #[cfg(not(windows))]
    {
        let _ = dxgi_tool;
        Err("Listing host displays is only supported on Windows".to_string())
    }
}

// ── Audio enumeration ─────────────────────────────────────────────────

/// List the active audio render endpoints, marking the current default and
/// anything that looks like Steam's virtual streaming sink.
pub fn list_audio_sinks() -> Result<Vec<AudioSinkEntry>, String> {
    #[cfg(windows)]
    {
        windows_impl::enumerate_audio_sinks()
    }
    #[cfg(not(windows))]
    {
        Err("Listing host audio devices is only supported on Windows".to_string())
    }
}

/// The exact endpoint name of Steam's virtual streaming sink, if one is
/// installed. Used when the user has not pinned a `virtual_sink` by hand, so a
/// machine without Steam never gets a dead device name written into its config.
pub fn find_steam_virtual_sink() -> Option<String> {
    let sinks = list_audio_sinks().ok()?;
    sinks.into_iter().find(|s| s.virtual_sink).map(|s| s.name)
}

// ── Windows implementation ────────────────────────────────────────────

#[cfg(windows)]
mod windows_impl {
    use super::{AudioSinkEntry, DisplayEntry, STEAM_VIRTUAL_SINK_MARKER};

    use windows::core::{GUID, PCWSTR};
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
        SetupDiGetDeviceInstanceIdW, SetupDiGetDeviceInterfaceDetailW, SetupDiOpenDevRegKey,
        DICS_FLAG_GLOBAL, DIGCF_DEVICEINTERFACE, DIREG_DEV, HDEVINFO, SP_DEVICE_INTERFACE_DATA,
        SP_DEVICE_INTERFACE_DETAIL_DATA_W, SP_DEVINFO_DATA,
    };
    use windows::Win32::Devices::Display::{
        DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
        DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
        DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE, DISPLAYCONFIG_PATH_INFO,
        DISPLAYCONFIG_SOURCE_DEVICE_NAME,
        DISPLAYCONFIG_TARGET_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS, QDC_VIRTUAL_MODE_AWARE,
    };
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS};
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE,
        DEVICE_STATE_ACTIVE, DEVICE_STATE_DISABLED, DEVICE_STATE_UNPLUGGED,
    };
    use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PROPVARIANT};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
        STGM_READ,
    };
    use windows::Win32::System::Variant::VT_LPWSTR;
    use windows::Win32::System::Registry::{RegCloseKey, RegQueryValueExW, KEY_READ, REG_VALUE_TYPE};

    /// `GUID_DEVINTERFACE_MONITOR` — the device interface class every monitor
    /// registers under, and the only handle SetupAPI gives onto a display's
    /// instance ID and raw EDID.
    const GUID_DEVINTERFACE_MONITOR: GUID = GUID::from_values(
        0xe6f0_7b5f,
        0xee97,
        0x4a90,
        [0xb0, 0x76, 0x33, 0xf5, 0x7b, 0xf4, 0xea, 0xa7],
    );

    /// Query every active display path plus its modes.
    fn query_active_paths()
    -> Result<(Vec<DISPLAYCONFIG_PATH_INFO>, Vec<DISPLAYCONFIG_MODE_INFO>), String> {
        // QDC_VIRTUAL_MODE_AWARE matches what Sunshine asks for, so the paths
        // (and therefore the ids derived from them) line up with its own view.
        let flags = QDC_ONLY_ACTIVE_PATHS | QDC_VIRTUAL_MODE_AWARE;
        loop {
            let (mut path_count, mut mode_count) = (0u32, 0u32);
            let result =
                unsafe { GetDisplayConfigBufferSizes(flags, &mut path_count, &mut mode_count) };
            if result != ERROR_SUCCESS {
                return Err(format!("GetDisplayConfigBufferSizes failed: {}", result.0));
            }

            let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
            let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
            let result = unsafe {
                QueryDisplayConfig(
                    flags,
                    &mut path_count,
                    paths.as_mut_ptr(),
                    &mut mode_count,
                    modes.as_mut_ptr(),
                    None,
                )
            };
            // The display set can change between the sizing call and the query;
            // Windows says so with ERROR_INSUFFICIENT_BUFFER, and the fix is to
            // ask again rather than to give up.
            if result == ERROR_INSUFFICIENT_BUFFER {
                continue;
            }
            if result != ERROR_SUCCESS {
                return Err(format!("QueryDisplayConfig failed: {}", result.0));
            }
            paths.truncate(path_count as usize);
            modes.truncate(mode_count as usize);
            return Ok((paths, modes));
        }
    }

    /// Whether a path drives the primary display.
    ///
    /// Windows has no "primary" flag in the path data; the primary display is
    /// simply the one whose source mode sits at the desktop origin.
    fn is_primary(path: &DISPLAYCONFIG_PATH_INFO, modes: &[DISPLAYCONFIG_MODE_INFO]) -> bool {
        // QDC_VIRTUAL_MODE_AWARE overlays the plain `modeInfoIdx` with a
        // bitfield. MSVC packs bitfields from the low end, so the clone group
        // takes bits 0-15 and the source mode index bits 16-31; 0xFFFF there
        // means "this path has no source mode".
        let index = unsafe { path.sourceInfo.Anonymous.Anonymous._bitfield } >> 16;
        if index == 0xFFFF {
            return false;
        }
        let Some(mode) = modes.get(index as usize) else {
            return false;
        };
        if mode.infoType != DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE {
            return false;
        }
        let position = unsafe { mode.Anonymous.sourceMode.position };
        position.x == 0 && position.y == 0
    }

    /// Read a fixed-size, NUL-terminated UTF-16 field into a `String`.
    fn wide_field_to_string(field: &[u16]) -> String {
        let len = field.iter().position(|c| *c == 0).unwrap_or(field.len());
        String::from_utf16_lossy(&field[..len])
    }

    fn wide_field_to_wstring(field: &[u16]) -> Vec<u16> {
        let len = field.iter().position(|c| *c == 0).unwrap_or(field.len());
        field[..len].to_vec()
    }

    /// `\\.\DISPLAYn` for a path's source.
    fn source_gdi_name(path: &DISPLAYCONFIG_PATH_INFO) -> String {
        let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
        source.header.adapterId = path.sourceInfo.adapterId;
        source.header.id = path.sourceInfo.id;
        source.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
        source.header.size = std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
        if unsafe { DisplayConfigGetDeviceInfo(&mut source.header) } != ERROR_SUCCESS.0 as i32 {
            return String::new();
        }
        wide_field_to_string(&source.viewGdiDeviceName)
    }

    /// The monitor's device path plus its friendly name, for a path's target.
    fn target_device(path: &DISPLAYCONFIG_PATH_INFO) -> Option<(Vec<u16>, String)> {
        let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
        target.header.adapterId = path.targetInfo.adapterId;
        target.header.id = path.targetInfo.id;
        target.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
        target.header.size = std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
        if unsafe { DisplayConfigGetDeviceInfo(&mut target.header) } != ERROR_SUCCESS.0 as i32 {
            return None;
        }
        let device_path = wide_field_to_wstring(&target.monitorDevicePath);
        if device_path.is_empty() {
            return None;
        }
        let friendly = wide_field_to_string(&target.monitorFriendlyDeviceName);
        Some((device_path, friendly))
    }

    /// SetupAPI handle that closes itself.
    struct DevInfoList(HDEVINFO);

    impl Drop for DevInfoList {
        fn drop(&mut self) {
            unsafe {
                let _ = SetupDiDestroyDeviceInfoList(self.0);
            }
        }
    }

    /// Instance ID and raw EDID for a monitor device path, via SetupAPI.
    ///
    /// The instance ID keeps its trailing NUL: `SetupDiGetDeviceInstanceIdW`
    /// reports a size that counts the terminator, and Sunshine hashes the whole
    /// buffer including it. Trimming here would produce a different id and
    /// Sunshine would silently ignore the display we picked.
    fn instance_id_and_edid(device_path: &[u16]) -> Option<(Vec<u16>, Vec<u8>)> {
        let handle = unsafe {
            SetupDiGetClassDevsW(
                Some(&GUID_DEVINTERFACE_MONITOR),
                PCWSTR::null(),
                None,
                DIGCF_DEVICEINTERFACE,
            )
        }
        .ok()?;
        let dev_info = DevInfoList(handle);

        for index in 0.. {
            let mut interface_data = SP_DEVICE_INTERFACE_DATA {
                cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                ..Default::default()
            };
            if unsafe {
                SetupDiEnumDeviceInterfaces(
                    dev_info.0,
                    None,
                    &GUID_DEVINTERFACE_MONITOR,
                    index,
                    &mut interface_data,
                )
            }
            .is_err()
            {
                // Out of interfaces (or a broken one) — either way, done.
                break;
            }

            let mut info_data = SP_DEVINFO_DATA {
                cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
                ..Default::default()
            };
            let Some(interface_path) =
                interface_detail_path(&dev_info, &mut interface_data, &mut info_data)
            else {
                continue;
            };
            if !wide_eq_ignore_case(&interface_path, device_path) {
                continue;
            }

            let instance_id = device_instance_id(&dev_info, &info_data)?;
            let edid = device_edid(&dev_info, &info_data)?;
            return Some((instance_id, edid));
        }
        None
    }

    /// `SP_DEVICE_INTERFACE_DETAIL_DATA_W.DevicePath` for one interface.
    ///
    /// The struct is variable-length with a fixed `cbSize` header, so it has to
    /// be built inside a raw byte buffer rather than as a Rust value.
    fn interface_detail_path(
        dev_info: &DevInfoList,
        interface_data: &mut SP_DEVICE_INTERFACE_DATA,
        info_data: &mut SP_DEVINFO_DATA,
    ) -> Option<Vec<u16>> {
        let mut required: u32 = 0;
        // First call always "fails" with ERROR_INSUFFICIENT_BUFFER; it is only
        // there to report the size.
        let _ = unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                dev_info.0,
                interface_data,
                None,
                0,
                Some(&mut required),
                None,
            )
        };
        if required == 0 {
            return None;
        }

        let mut buffer = vec![0u8; required as usize];
        let detail = buffer.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
        unsafe {
            (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
        }
        unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                dev_info.0,
                interface_data,
                Some(detail),
                required,
                None,
                Some(info_data),
            )
        }
        .ok()?;

        // DevicePath is declared as a 1-element array but runs to the end of
        // the buffer, so read it as a NUL-terminated string from that offset.
        let path_ptr = unsafe { (*detail).DevicePath.as_ptr() };
        let max_chars = (required as usize)
            .saturating_sub(std::mem::offset_of!(SP_DEVICE_INTERFACE_DETAIL_DATA_W, DevicePath))
            / 2;
        let slice = unsafe { std::slice::from_raw_parts(path_ptr, max_chars) };
        let path = wide_field_to_wstring(slice);
        (!path.is_empty()).then_some(path)
    }

    fn device_instance_id(dev_info: &DevInfoList, info_data: &SP_DEVINFO_DATA) -> Option<Vec<u16>> {
        let mut required: u32 = 0;
        let _ = unsafe {
            SetupDiGetDeviceInstanceIdW(dev_info.0, info_data, None, Some(&mut required))
        };
        if required == 0 {
            return None;
        }
        // `required` counts the NUL terminator, and that terminator is part of
        // what Sunshine hashes — see `instance_id_and_edid`.
        let mut buffer = vec![0u16; required as usize];
        unsafe {
            SetupDiGetDeviceInstanceIdW(dev_info.0, info_data, Some(&mut buffer), None).ok()?;
        }
        Some(buffer)
    }

    fn device_edid(dev_info: &DevInfoList, info_data: &SP_DEVINFO_DATA) -> Option<Vec<u8>> {
        let key = unsafe {
            SetupDiOpenDevRegKey(
                dev_info.0,
                info_data,
                DICS_FLAG_GLOBAL.0,
                0,
                DIREG_DEV,
                KEY_READ.0,
            )
        }
        .ok()?;

        let mut size: u32 = 0;
        let name = windows::core::w!("EDID");
        let status = unsafe {
            RegQueryValueExW(
                key,
                name,
                None,
                None::<*mut REG_VALUE_TYPE>,
                None,
                Some(&mut size),
            )
        };
        let mut edid = Vec::new();
        if status == ERROR_SUCCESS && size > 0 {
            edid.resize(size as usize, 0u8);
            let status = unsafe {
                RegQueryValueExW(
                    key,
                    name,
                    None,
                    None::<*mut REG_VALUE_TYPE>,
                    Some(edid.as_mut_ptr()),
                    Some(&mut size),
                )
            };
            if status != ERROR_SUCCESS {
                edid.clear();
            }
        }
        unsafe {
            let _ = RegCloseKey(key);
        }
        (!edid.is_empty()).then_some(edid)
    }

    fn wide_eq_ignore_case(a: &[u16], b: &[u16]) -> bool {
        a.len() == b.len()
            && a.iter().zip(b).all(|(x, y)| {
                let lower = |c: u16| {
                    if (b'A' as u16..=b'Z' as u16).contains(&c) {
                        c + 32
                    } else {
                        c
                    }
                };
                lower(*x) == lower(*y)
            })
    }

    /// Reproduce Sunshine's display id: a UUID v5 (SHA-1, null namespace) over
    /// the monitor's EDID followed by the stable halves of its SetupAPI
    /// instance ID, falling back to the device path when SetupAPI has nothing.
    ///
    /// An instance ID looks like `DISPLAY\ACI27EC\5&4fd2de4&5&UID4352`. The
    /// fourth field is a counter that changes on driver reinstall, so Sunshine
    /// hashes everything before it and everything from the field after it, and
    /// drops the counter in between. Getting this byte-identical is the whole
    /// point: the id is what `output_name` is matched against, and a
    /// near-miss makes Sunshine quietly capture the wrong display instead.
    fn display_device_id(device_path: &[u16]) -> String {
        let mut data: Vec<u8> = Vec::new();

        if let Some((instance_id, edid)) = instance_id_and_edid(device_path) {
            let amp = |from: usize| {
                instance_id
                    .get(from..)
                    .and_then(|tail| tail.iter().position(|c| *c == u16::from(b'&')))
                    .map(|offset| from + offset)
            };
            let unstable = amp(0).and_then(|first| amp(first + 1));
            let semi_stable = unstable.and_then(|i| amp(i + 1));

            if let (Some(unstable), Some(semi_stable)) = (unstable, semi_stable) {
                data.extend_from_slice(&edid);
                push_wide(&mut data, &instance_id[..unstable]);
                push_wide(&mut data, &instance_id[semi_stable..]);
            }
        }

        if data.is_empty() {
            push_wide(&mut data, device_path);
        }

        format!("{{{}}}", uuid_v5_null_namespace(&data))
    }

    fn push_wide(out: &mut Vec<u8>, chars: &[u16]) {
        for c in chars {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }

    /// UUID v5 with the nil namespace, formatted lowercase without braces.
    fn uuid_v5_null_namespace(data: &[u8]) -> String {
        use sha1::{Digest, Sha1};

        let mut hasher = Sha1::new();
        hasher.update([0u8; 16]); // nil namespace, matching boost's default
        hasher.update(data);
        let digest = hasher.finalize();

        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&digest[..16]);
        bytes[6] = (bytes[6] & 0x0f) | 0x50; // version 5
        bytes[8] = (bytes[8] & 0x3f) | 0x80; // RFC 4122 variant

        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        )
    }

    pub fn enumerate_displays() -> Result<Vec<DisplayEntry>, String> {
        let (paths, modes) = query_active_paths()?;
        let mut entries: Vec<DisplayEntry> = Vec::new();

        for path in &paths {
            let Some((device_path, friendly)) = target_device(path) else {
                continue;
            };
            let display = source_gdi_name(path);
            let device_id = display_device_id(&device_path);
            if entries.iter().any(|e| e.device_id == device_id) {
                continue;
            }
            entries.push(DisplayEntry {
                device_id,
                primary: is_primary(path, &modes),
                friendly_name: if friendly.is_empty() {
                    display.clone()
                } else {
                    friendly
                },
                display,
                adapter: String::new(),
                resolution: String::new(),
            });
        }

        Ok(entries)
    }

    /// COM apartment that lasts exactly as long as the call needs it.
    struct ComGuard(bool);

    impl ComGuard {
        fn new() -> Self {
            // The pairing rule is per successful init, and `is_ok()` draws the
            // line in exactly the right place: S_OK and S_FALSE (already in this
            // apartment) both owe a CoUninitialize, while RPC_E_CHANGED_MODE
            // (already in a *different* apartment) initialised nothing and must
            // not be torn down here.
            let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            Self(hr.is_ok())
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }

    /// Read a `VT_LPWSTR` PROPVARIANT and free it.
    ///
    /// `IPropertyStore::GetValue` hands back an owned PROPVARIANT. The explicit
    /// `PropVariantClear` is belt and braces: windows-rs implements `Drop` for
    /// `PROPVARIANT` and calls it too, and since the first call leaves the
    /// struct VT_EMPTY the second is a documented no-op. Freeing at the point of
    /// the read keeps the ownership obvious.
    unsafe fn take_propvariant_string(value: &mut PROPVARIANT) -> String {
        let text = unsafe {
            let inner = &value.Anonymous.Anonymous;
            if inner.vt == VT_LPWSTR && !inner.Anonymous.pwszVal.is_null() {
                inner.Anonymous.pwszVal.to_string().unwrap_or_default()
            } else {
                String::new()
            }
        };
        unsafe {
            let _ = PropVariantClear(value);
        }
        text
    }

    pub fn enumerate_audio_sinks() -> Result<Vec<AudioSinkEntry>, String> {
        // COM apartment state is per-thread, so this runs on a thread of its
        // own rather than borrowing whichever runtime thread called in.
        std::thread::scope(|scope| {
            scope
                .spawn(|| unsafe { enumerate_audio_sinks_inner() })
                .join()
                .map_err(|_| "Audio enumeration thread panicked".to_string())?
        })
    }

    /// Everything except endpoints whose hardware is gone.
    ///
    /// Enumerating on `DEVICE_STATE_ACTIVE` alone is not enough. On the machine
    /// this was written against, Windows reports every render endpoint as
    /// Disabled with no default endpoint at all — Sunshine's own `audio-info`
    /// tool says exactly the same, so it is the audio service's view, not a bug
    /// here. Filtering strictly on Active there produces an empty picker, which
    /// is the failure this whole feature exists to prevent, so the real state
    /// is read per device and used to rank rather than to exclude.
    const LISTABLE_STATES: DEVICE_STATE = DEVICE_STATE(
        DEVICE_STATE_ACTIVE.0 | DEVICE_STATE_DISABLED.0 | DEVICE_STATE_UNPLUGGED.0,
    );

    unsafe fn enumerate_audio_sinks_inner() -> Result<Vec<AudioSinkEntry>, String> {
        let _com = ComGuard::new();

        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
                .map_err(|e| format!("Failed to create MMDeviceEnumerator: {e}"))?;

        let default_id = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }
            .ok()
            .and_then(|d| unsafe { d.GetId() }.ok())
            .and_then(|id| unsafe { id.to_string() }.ok());

        let collection = unsafe { enumerator.EnumAudioEndpoints(eRender, LISTABLE_STATES) }
            .map_err(|e| format!("Failed to enumerate audio endpoints: {e}"))?;
        let count = unsafe { collection.GetCount() }
            .map_err(|e| format!("Failed to count audio endpoints: {e}"))?;

        let mut sinks = Vec::new();
        for index in 0..count {
            let Ok(device) = (unsafe { collection.Item(index) }) else {
                continue;
            };
            let Ok(store) = (unsafe { device.OpenPropertyStore(STGM_READ) }) else {
                continue;
            };
            let Ok(mut value) = (unsafe { store.GetValue(&PKEY_Device_FriendlyName) }) else {
                continue;
            };
            let name = unsafe { take_propvariant_string(&mut value) };
            if name.is_empty() {
                continue;
            }
            let id = unsafe { device.GetId() }
                .ok()
                .and_then(|id| unsafe { id.to_string() }.ok());
            let active = unsafe { device.GetState() }
                .is_ok_and(|state| state == DEVICE_STATE_ACTIVE);
            sinks.push(AudioSinkEntry {
                active,
                default: id.is_some() && id == default_id,
                virtual_sink: name.contains(STEAM_VIRTUAL_SINK_MARKER),
                name,
            });
        }
        // Ready endpoints first, then the current default, so the picker's top
        // entries are the ones worth choosing.
        sinks.sort_by_key(|s| (!s.active, !s.default));
        Ok(sinks)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// Verbatim `dxgi-info.exe` output from a two-GPU machine: an NVIDIA card
    /// driving one monitor, an AMD iGPU driving a portrait one, plus a
    /// duplicate NVIDIA entry and the Basic Render Driver with no outputs.
    const REAL_OUTPUT: &str = r"====== ADAPTER =====
Device Name      : NVIDIA GeForce RTX 4070 Ti SUPER
Device Vendor ID : 0x000010DE
Device Device ID : 0x00002705
Device Video Mem : 16061 MiB
Device Sys Mem   : 0 MiB
Share Sys Mem    : 48315 MiB

    ====== OUTPUT ======
    Output Name       : \\.\DISPLAY1
    AttachedToDesktop : yes
    Resolution        : 2560x1440

====== ADAPTER =====
Device Name      : AMD Radeon(TM) Graphics
Device Vendor ID : 0x00001002
Device Device ID : 0x0000164E
Device Video Mem : 485 MiB
Device Sys Mem   : 0 MiB
Share Sys Mem    : 48315 MiB

    ====== OUTPUT ======
    Output Name       : \\.\DISPLAY5
    AttachedToDesktop : yes
    Resolution        : 1080x1920

====== ADAPTER =====
Device Name      : NVIDIA GeForce RTX 4070 Ti SUPER
Device Vendor ID : 0x000010DE
Device Device ID : 0x00002705

    ====== OUTPUT ======
====== ADAPTER =====
Device Name      : Microsoft Basic Render Driver
Device Vendor ID : 0x00001414
Device Device ID : 0x0000008C

    ====== OUTPUT ======
";

    #[test]
    fn every_attached_output_keeps_its_own_adapter() {
        let outputs = parse_dxgi_info(REAL_OUTPUT);
        assert_eq!(
            outputs,
            vec![
                DxgiOutput {
                    adapter: "NVIDIA GeForce RTX 4070 Ti SUPER".into(),
                    display: r"\\.\DISPLAY1".into(),
                    resolution: "2560x1440".into(),
                },
                DxgiOutput {
                    adapter: "AMD Radeon(TM) Graphics".into(),
                    display: r"\\.\DISPLAY5".into(),
                    resolution: "1080x1920".into(),
                },
            ]
        );
    }

    #[test]
    fn adapters_without_outputs_contribute_nothing() {
        let outputs = parse_dxgi_info(REAL_OUTPUT);
        assert!(!outputs.iter().any(|o| o.display.is_empty()));
        assert!(!outputs
            .iter()
            .any(|o| o.adapter == "Microsoft Basic Render Driver"));
    }

    #[test]
    fn detached_outputs_are_dropped() {
        let raw = r"====== ADAPTER =====
Device Name      : Some GPU

    ====== OUTPUT ======
    Output Name       : \\.\DISPLAY2
    AttachedToDesktop : no
    Resolution        : 1920x1080
";
        assert!(parse_dxgi_info(raw).is_empty());
    }

    #[test]
    fn several_outputs_on_one_adapter_all_survive() {
        let raw = r"====== ADAPTER =====
Device Name      : Some GPU

    ====== OUTPUT ======
    Output Name       : \\.\DISPLAY1
    AttachedToDesktop : yes
    Resolution        : 1920x1080

    ====== OUTPUT ======
    Output Name       : \\.\DISPLAY2
    AttachedToDesktop : yes
    Resolution        : 3840x2160
";
        let outputs = parse_dxgi_info(raw);
        assert_eq!(outputs.len(), 2);
        assert!(outputs.iter().all(|o| o.adapter == "Some GPU"));
        assert_eq!(outputs[1].resolution, "3840x2160");
    }

    #[test]
    fn an_output_missing_its_resolution_is_still_listed() {
        let raw = r"====== ADAPTER =====
Device Name      : Some GPU

    ====== OUTPUT ======
    Output Name       : \\.\DISPLAY1
    AttachedToDesktop : yes
";
        let outputs = parse_dxgi_info(raw);
        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].resolution.is_empty());
    }

    #[test]
    fn garbage_and_empty_input_parse_to_nothing() {
        assert!(parse_dxgi_info("").is_empty());
        assert!(parse_dxgi_info("not remotely dxgi output\nno colons here").is_empty());
        // Field lines with no enclosing OUTPUT block must not invent one.
        assert!(parse_dxgi_info("Output Name : \\\\.\\DISPLAY1").is_empty());
    }

    /// Hardware probe, not an assertion: prints what this machine reports so
    /// the display ids can be diffed against the ones Sunshine writes into its
    /// own log ("Currently available display devices:"). Ignored by default
    /// because the answer depends on the monitors that happen to be plugged in.
    ///
    ///     cargo test --lib host_devices -- --ignored --nocapture
    #[cfg(windows)]
    #[test]
    #[ignore = "depends on the monitors and sound devices attached to this PC"]
    fn probe_this_machine() {
        let tool = dirs::data_dir()
            .unwrap()
            .join("drop/tools/sunshine/dxgi-info.exe");
        for d in list_displays(&tool).unwrap() {
            println!("{d:?}");
        }
        for s in list_audio_sinks().unwrap() {
            println!("{s:?}");
        }
    }

    #[test]
    fn a_truncated_final_block_does_not_leak_the_previous_adapter() {
        let raw = r"====== ADAPTER =====
Device Name      : Some GPU

    ====== OUTPUT ======
    Output Name       : \\.\DISPLAY1
    AttachedToDesktop : yes
    Resolution        : 1920x1080

====== ADAPTER =====
Device Name      : Other GPU
";
        let outputs = parse_dxgi_info(raw);
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].adapter, "Some GPU");
    }
}
