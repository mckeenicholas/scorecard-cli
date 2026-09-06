use super::model::Competition;
use std::path::Path;

/// Expands leading `~` or `~/` to the user's home directory.
pub fn expand_tilde<P: AsRef<Path>>(path: P) -> std::path::PathBuf {
    let p = path.as_ref();
    if let Some(s) = p.to_str() {
        if s == "~" {
            if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
            {
                return std::path::PathBuf::from(home);
            }
        } else if let Some(rest) = s.strip_prefix("~/")
            && let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
        {
            return std::path::PathBuf::from(home).join(rest);
        }
    }
    p.to_path_buf()
}

/// Error encountered while loading WCIF competition data from files or the WCA API.
#[derive(Debug)]
pub enum WcifLoadError {
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    Json(serde_json::Error),
    Http(ureq::Error),
    NotFound(String),
    ApiStatus {
        status: u16,
        comp_id: String,
    },
}

impl std::fmt::Display for WcifLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WcifLoadError::Io { path, source } => {
                write!(f, "Failed to read WCIF file '{}': {source}", path.display())
            }
            WcifLoadError::Json(e) => write!(f, "Failed to parse WCIF JSON: {e}"),
            WcifLoadError::Http(e) => write!(f, "WCA API HTTP error: {e}"),
            WcifLoadError::NotFound(comp_id) => {
                write!(
                    f,
                    "Competition '{comp_id}' not found on WCA website (API returned 404)"
                )
            }
            WcifLoadError::ApiStatus { status, comp_id } => {
                write!(
                    f,
                    "WCA API request for '{comp_id}' failed with HTTP {status}"
                )
            }
        }
    }
}

impl std::error::Error for WcifLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            WcifLoadError::Io { source, .. } => Some(source),
            WcifLoadError::Json(e) => Some(e),
            WcifLoadError::Http(e) => Some(e),
            WcifLoadError::NotFound(_) | WcifLoadError::ApiStatus { .. } => None,
        }
    }
}

impl From<serde_json::Error> for WcifLoadError {
    fn from(err: serde_json::Error) -> Self {
        WcifLoadError::Json(err)
    }
}

impl From<ureq::Error> for WcifLoadError {
    fn from(err: ureq::Error) -> Self {
        WcifLoadError::Http(err)
    }
}

/// Loader responsible for retrieving WCIF data from local files or the WCA website API.
pub struct WcifLoader;

impl WcifLoader {
    /// Loads a WCIF Competition from either a local file path or a WCA competition ID.
    pub fn load(source: &str) -> Result<Competition, WcifLoadError> {
        let expanded = expand_tilde(source);
        if expanded.exists() {
            Self::load_from_file(&expanded)
        } else {
            Self::fetch_from_wca(source)
        }
    }

    /// Reads WCIF JSON directly from a file into memory and deserializes from bytes.
    pub fn load_from_file(path: &Path) -> Result<Competition, WcifLoadError> {
        let bytes = std::fs::read(path).map_err(|e| WcifLoadError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let comp = Competition::from_json_bytes(&bytes)?;
        Ok(comp)
    }

    /// Fetches the public WCIF JSON for a competition ID from the WCA API.
    pub fn fetch_from_wca(comp_id: &str) -> Result<Competition, WcifLoadError> {
        let spinner = crate::progress::create_spinner(format!("Fetching WCIF for '{comp_id}'..."));
        let bytes = Self::fetch_wca_api_bytes(comp_id, &spinner)?;
        spinner.finish_and_clear();

        let comp = Competition::from_json_bytes(&bytes)?;
        Ok(comp)
    }

    fn fetch_wca_api_bytes(
        comp_id: &str,
        spinner: &indicatif::ProgressBar,
    ) -> Result<Vec<u8>, WcifLoadError> {
        let api_url = format!(
            "https://www.worldcubeassociation.org/api/v0/competitions/{comp_id}/wcif/public"
        );

        let agent: ureq::Agent = ureq::config::Config::builder()
            .user_agent("fast-scorecard-gen/0.1.0 (https://github.com/mckeenicholas/scorecard-cli)")
            .http_status_as_error(false)
            .build()
            .into();

        let mut resp = agent.get(&api_url).call()?;

        let status = resp.status().as_u16();
        if status == 404 {
            return Err(WcifLoadError::NotFound(comp_id.to_string()));
        }
        if !resp.status().is_success() {
            return Err(WcifLoadError::ApiStatus {
                status,
                comp_id: comp_id.to_string(),
            });
        }

        spinner.set_message(format!("Downloading and parsing WCIF for '{comp_id}'..."));

        let bytes = resp.body_mut().read_to_vec()?;
        Ok(bytes)
    }
}
