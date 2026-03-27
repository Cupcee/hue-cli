use std::collections::HashMap;

use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Bridge discovery & registration
// ---------------------------------------------------------------------------

pub fn discover_bridge() -> Result<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    let resp = client
        .get("https://discovery.meethue.com")
        .send()
        .map_err(|e| anyhow!("Discovery request failed: {e}"))?;

    let bridges: Vec<Value> = resp.json()?;
    bridges
        .first()
        .and_then(|b| b["internalipaddress"].as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("No bridges found via cloud discovery"))
}

/// Press the link button on the bridge, then call this.
pub fn register_app(bridge_ip: &str) -> Result<String> {
    let client = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let url = format!("http://{bridge_ip}/api");
    let body = json!({"devicetype": "hue-cli#rust"});
    let resp = client.post(&url).json(&body).send()?;
    let result: Vec<Value> = resp.json()?;

    let entry = result
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("Empty response from bridge"))?;

    if let Some(success) = entry.get("success") {
        return success["username"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("No username in success response"));
    }

    if let Some(error) = entry.get("error") {
        let desc = error["description"].as_str().unwrap_or("unknown error");
        return Err(anyhow!("Bridge error: {desc}"));
    }

    Err(anyhow!("Unexpected response from bridge"))
}

// ---------------------------------------------------------------------------
// API client
// ---------------------------------------------------------------------------

pub struct HueClient {
    bridge_ip: String,
    username: String,
    client: reqwest::blocking::Client,
}

#[derive(Debug, Deserialize)]
pub struct Group {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
}

impl HueClient {
    pub fn new(bridge_ip: impl Into<String>, username: impl Into<String>) -> Self {
        let client = reqwest::blocking::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("failed to build HTTP client");
        Self {
            bridge_ip: bridge_ip.into(),
            username: username.into(),
            client,
        }
    }

    fn base(&self) -> String {
        format!("http://{}/api/{}", self.bridge_ip, self.username)
    }

    // -----------------------------------------------------------------------
    // Groups
    // -----------------------------------------------------------------------

    pub fn get_groups(&self) -> Result<HashMap<String, Group>> {
        let url = format!("{}/groups", self.base());
        let resp = self.client.get(&url).send()?;
        Ok(resp.json()?)
    }

    pub fn find_group_id(&self, name: &str) -> Result<String> {
        let groups = self.get_groups()?;
        for (id, group) in &groups {
            if group.name.eq_ignore_ascii_case(name) {
                return Ok(id.clone());
            }
        }
        let available: Vec<&str> = groups.values().map(|g| g.name.as_str()).collect();
        Err(anyhow!(
            "Group '{}' not found. Available groups: {}",
            name,
            available.join(", ")
        ))
    }

    // -----------------------------------------------------------------------
    // Actions
    // -----------------------------------------------------------------------

    pub fn set_group_on(&self, group_id: &str, on: bool) -> Result<()> {
        let url = format!("{}/groups/{}/action", self.base(), group_id);
        let resp = self.client.put(&url).json(&json!({"on": on})).send()?;
        check_hue_response(resp)
    }

    /// level: 0 = off, 1–100 = brightness percentage
    pub fn set_group_brightness(&self, group_id: &str, level: u8) -> Result<()> {
        let url = format!("{}/groups/{}/action", self.base(), group_id);
        let body = if level == 0 {
            json!({"on": false})
        } else {
            // Map 1–100 → 1–254
            let bri = ((level as u16 * 254 + 99) / 100).min(254).max(1) as u8;
            json!({"on": true, "bri": bri})
        };
        let resp = self.client.put(&url).json(&body).send()?;
        check_hue_response(resp)
    }

    pub fn set_group_color(&self, group_id: &str, r: u8, g: u8, b: u8) -> Result<()> {
        let (x, y) = crate::color::rgb_to_xy(r, g, b);
        let url = format!("{}/groups/{}/action", self.base(), group_id);
        let resp = self
            .client
            .put(&url)
            .json(&json!({"on": true, "xy": [x, y]}))
            .send()?;
        check_hue_response(resp)
    }
}

fn check_hue_response(resp: reqwest::blocking::Response) -> Result<()> {
    let body: Vec<Value> = resp.json()?;
    for item in &body {
        if let Some(error) = item.get("error") {
            let desc = error["description"].as_str().unwrap_or("unknown error");
            return Err(anyhow!("Hue API error: {desc}"));
        }
    }
    Ok(())
}
