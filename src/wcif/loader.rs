use super::model::Competition;
use std::error::Error;
use std::path::Path;

/// Expands leading `~` or `~/` to the user's home directory.
pub fn expand_tilde<P: AsRef<Path>>(path: P) -> std::path::PathBuf {
    let p = path.as_ref();
    if let Some(s) = p.to_str() {
        if s == "~" {
            if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
                return std::path::PathBuf::from(home);
            }
        } else if let Some(rest) = s.strip_prefix("~/") {
            if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
                return std::path::PathBuf::from(home).join(rest);
            }
        }
    }
    p.to_path_buf()
}

/// Loader responsible for retrieving WCIF data from local files or the WCA website API.
pub struct WcifLoader;

impl WcifLoader {
    /// Loads a WCIF Competition from either a local file path or a WCA competition ID.
    pub fn load(source: &str) -> Result<Competition, Box<dyn Error>> {
        let expanded = expand_tilde(source);
        if expanded.exists() {
            Self::load_from_file(&expanded)
        } else {
            Self::fetch_from_wca(source)
        }
    }

    /// Reads WCIF JSON directly from a file into memory and deserializes from bytes.
    pub fn load_from_file(path: &Path) -> Result<Competition, Box<dyn Error>> {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("failed to read file {}: {}", path.display(), e))?;
        let comp = Competition::from_json_bytes(&bytes)
            .map_err(|e| format!("failed to decode WCIF JSON from file: {}", e))?;
        Ok(comp)
    }

    /// Fetches the public WCIF JSON for a competition ID from the WCA API.
    pub fn fetch_from_wca(comp_id: &str) -> Result<Competition, Box<dyn Error>> {
        let spinner =
            crate::progress::create_spinner(format!("Fetching WCIF for '{}'...", comp_id));
        let bytes = Self::fetch_wca_api_bytes(comp_id, &spinner)?;
        spinner.finish_and_clear();

        let comp = Competition::from_json_bytes(&bytes)
            .map_err(|e| format!("failed to parse public WCIF JSON: {}", e))?;
        Ok(comp)
    }

    fn fetch_wca_api_bytes(
        comp_id: &str,
        spinner: &indicatif::ProgressBar,
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        let api_url = format!(
            "https://www.worldcubeassociation.org/api/v0/competitions/{}/wcif/public",
            comp_id
        );

        let client = reqwest::blocking::Client::builder()
            .user_agent("fast-scorecard-gen/0.1.0 (https://github.com/mckeenicholas/scorecard-cli)")
            .build()?;

        let resp = client
            .get(&api_url)
            .send()
            .map_err(|e| format!("failed to fetch WCIF from WCA API: {}", e))?;

        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(format!(
                "competition {} not found on WCA website (API returned 404)",
                comp_id
            )
            .into());
        }
        if !status.is_success() {
            return Err(format!("WCA API request failed with status: {}", status).into());
        }

        spinner.set_message(format!("Downloading and parsing WCIF for '{}'...", comp_id));

        let bytes = resp
            .bytes()
            .map_err(|e| format!("failed to read response bytes from WCA API: {}", e))?;

        Ok(bytes.to_vec())
    }
}
