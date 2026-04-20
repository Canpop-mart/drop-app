use log::warn;

/// Schemes we will hand off to the system browser. Anything else (file://,
/// javascript:, UNC paths, exotic handlers) is rejected so a malicious server
/// response can't use us as a launcher.
const ALLOWED_SCHEMES: &[&str] = &["http", "https", "mailto"];

pub fn webbrowser_open<T: AsRef<str>>(url: T) {
    let raw = url.as_ref();
    match url::Url::parse(raw) {
        Ok(parsed) => {
            let scheme = parsed.scheme().to_ascii_lowercase();
            if !ALLOWED_SCHEMES.iter().any(|s| *s == scheme) {
                warn!("refusing to open url with scheme {scheme:?}: {raw}");
                return;
            }
        }
        Err(e) => {
            warn!("refusing to open unparseable url {raw:?}: {e}");
            return;
        }
    }

    if let Err(e) = webbrowser::open(raw) {
        warn!("could not open web browser to url {raw} with error {e}");
    }
}
