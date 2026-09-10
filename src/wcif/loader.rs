use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::Error as IoError;
use std::path::{Path, PathBuf};
use std::{env, fs};

use ureq::Agent;
use ureq::config::Config;

use super::model::Competition;
use crate::progress;

const DOWNLOAD_FETCH_LIMIT: u64 = 100 * 1024 * 1024; // 100 MB

/// Expands leading `~` or `~/` to the user's home directory.
pub fn expand_tilde<P: AsRef<Path>>(path: P) -> PathBuf {
    let p = path.as_ref();
    if let Some(s) = p.to_str() {
        if s == "~" {
            if let Some(home) = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE")) {
                return PathBuf::from(home);
            }
        } else if let Some(rest) = s.strip_prefix("~/")
            && let Some(home) = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE"))
        {
            return PathBuf::from(home).join(rest);
        }
    }
    p.to_path_buf()
}

/// Error encountered while loading WCIF competition data from files or the WCA API.
#[derive(Debug)]
pub enum WcifLoadError {
    Io { path: PathBuf, source: IoError },
    Json(serde_json::Error),
    Http(ureq::Error),
    NotFound(String),
    ApiStatus { status: u16, comp_id: String },
}

impl Display for WcifLoadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            WcifLoadError::Io { path, source } => {
                write!(f, "Failed to read WCIF file '{}': {source}", path.display())
            }
            WcifLoadError::Json(e) => write!(f, "Failed to parse WCIF JSON: {e}"),
            WcifLoadError::Http(e) => write!(f, "WCA API HTTP error::Error: {e}"),
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

impl Error for WcifLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
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
        let bytes = fs::read(path).map_err(|e| WcifLoadError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let comp = Competition::from_json_bytes(&bytes)?;
        Ok(comp)
    }

    /// Fetches the public WCIF JSON for a competition ID from the WCA API.
    pub fn fetch_from_wca(comp_id: &str) -> Result<Competition, WcifLoadError> {
        let spinner = progress::create_spinner(format!("Fetching WCIF for '{comp_id}'..."));
        let comp = Self::fetch_wca_api_competition(comp_id, &spinner)?;
        spinner.finish_and_clear();
        Ok(comp)
    }

    fn fetch_wca_api_competition(
        comp_id: &str,
        spinner: &indicatif::ProgressBar,
    ) -> Result<Competition, WcifLoadError> {
        let api_url = format!(
            "https://www.worldcubeassociation.org/api/v0/competitions/{comp_id}/wcif/public"
        );

        let agent: Agent = Config::builder()
            .user_agent("fast-scorecard-gen/0.1.0 (https://github.com/mckeenicholas/scorecard-cli)")
            .http_status_as_error(false)
            .build()
            .into();

        let mut resp = agent.get(&api_url).call()?;

        let status = resp.status().as_u16();
        if status == 404 {
            return Err(WcifLoadError::NotFound(comp_id.to_owned()));
        }
        if !resp.status().is_success() {
            return Err(WcifLoadError::ApiStatus {
                status,
                comp_id: comp_id.to_owned(),
            });
        }

        spinner.set_message(format!("Downloading and parsing WCIF for '{comp_id}'..."));

        let comp: Competition = resp
            .body_mut()
            .with_config()
            .limit(DOWNLOAD_FETCH_LIMIT)
            .read_json()?;
        Ok(comp)
    }
}
