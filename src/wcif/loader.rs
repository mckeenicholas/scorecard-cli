use super::model::Competition;
use std::error::Error;
use std::path::Path;

/// Loader responsible for retrieving WCIF data from local files or the WCA website API.
pub struct WcifLoader;

impl WcifLoader {
    /// Loads a WCIF Competition from either a local file path or a WCA competition ID.
    pub fn load(source: &str) -> Result<Competition, Box<dyn Error>> {
        let path = Path::new(source);
        if path.exists() {
            Self::load_from_file(path)
        } else {
            Self::fetch_from_wca(source)
        }
    }

    /// Reads WCIF JSON directly from a file into memory and deserializes from bytes.
    pub fn load_from_file(path: &Path) -> Result<Competition, Box<dyn Error>> {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("failed to read file {}: {}", path.display(), e))?;
        let comp: Competition = serde_json::from_slice(&bytes)
            .map_err(|e| format!("failed to decode WCIF JSON from file: {}", e))?;
        Ok(comp)
    }

    /// Fetches the public WCIF JSON for a competition ID from the WCA API.
    pub fn fetch_from_wca(comp_id: &str) -> Result<Competition, Box<dyn Error>> {
        let encoded_comp_id = urlencoding::encode(comp_id);
        let api_url = format!(
            "https://www.worldcubeassociation.org/api/v0/competitions/{}/wcif/public",
            encoded_comp_id
        );

        let client = reqwest::blocking::Client::builder()
            .user_agent("fast-scorecard-gen/0.1.0 (https://github.com)")
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

        let bytes = resp
            .bytes()
            .map_err(|e| format!("failed to read response bytes from WCA API: {}", e))?;
        let comp: Competition = serde_json::from_slice(&bytes)
            .map_err(|e| format!("failed to parse public WCIF JSON: {}", e))?;

        Ok(comp)
    }
}

// Simple urlencoding helper to avoid adding external dependencies
mod urlencoding {
    pub fn encode(input: &str) -> String {
        let mut encoded = String::with_capacity(input.len());
        for b in input.bytes() {
            match b {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    encoded.push(b as char);
                }
                _ => {
                    encoded.push_str(&format!("%{:02X}", b));
                }
            }
        }
        encoded
    }
}
