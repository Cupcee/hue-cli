use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Bridge discovery & registration (still uses v1 HTTP endpoint)
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
/// Registration still goes through the v1 HTTP endpoint — that's intentional,
/// the returned key works for v2 as well.
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
// v2 response envelope
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct V2Response<T> {
    data: Vec<T>,
    errors: Vec<V2Error>,
}

#[derive(Deserialize)]
struct V2Error {
    description: String,
}

// Internal room shape from GET /clip/v2/resource/room
#[derive(Deserialize)]
struct RoomResource {
    metadata: RoomMetadata,
    services: Vec<ServiceRef>,
}

#[derive(Deserialize)]
struct RoomMetadata {
    name: String,
}

#[derive(Deserialize)]
struct ServiceRef {
    rid: String,
    rtype: String,
}

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

pub struct Room {
    pub name: String,
    /// UUID of the grouped_light service that controls this room.
    pub grouped_light_id: String,
}

// ---------------------------------------------------------------------------
// API client
// ---------------------------------------------------------------------------

pub struct HueClient {
    bridge_ip: String,
    api_key: String,
    client: reqwest::blocking::Client,
}

impl HueClient {
    pub fn new(bridge_ip: impl Into<String>, api_key: impl Into<String>) -> Self {
        let client = reqwest::blocking::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("failed to build HTTP client");
        Self {
            bridge_ip: bridge_ip.into(),
            api_key: api_key.into(),
            client,
        }
    }

    fn base(&self) -> String {
        format!("https://{}/clip/v2", self.bridge_ip)
    }

    fn get(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        self.client
            .get(url)
            .header("hue-application-key", &self.api_key)
    }

    fn put(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        self.client
            .put(url)
            .header("hue-application-key", &self.api_key)
    }

    // -----------------------------------------------------------------------
    // Rooms
    // -----------------------------------------------------------------------

    pub fn get_rooms(&self) -> Result<Vec<Room>> {
        let url = format!("{}/resource/room", self.base());
        let resp = self.get(&url).send()?;
        let body: V2Response<RoomResource> = resp.json()?;
        check_errors(&body.errors)?;

        let rooms = body
            .data
            .into_iter()
            .filter_map(|r| {
                r.services
                    .into_iter()
                    .find(|s| s.rtype == "grouped_light")
                    .map(|gl| Room {
                        name: r.metadata.name,
                        grouped_light_id: gl.rid,
                    })
            })
            .collect();

        Ok(rooms)
    }

    /// Find the grouped_light UUID for a room by name (case-insensitive).
    pub fn find_group_id(&self, name: &str) -> Result<String> {
        let rooms = self.get_rooms()?;
        for room in &rooms {
            if room.name.eq_ignore_ascii_case(name) {
                return Ok(room.grouped_light_id.clone());
            }
        }
        let available: Vec<&str> = rooms.iter().map(|r| r.name.as_str()).collect();
        Err(anyhow!(
            "Room '{}' not found. Available rooms: {}",
            name,
            available.join(", ")
        ))
    }

    // -----------------------------------------------------------------------
    // Actions — all target /clip/v2/resource/grouped_light/<uuid>
    // -----------------------------------------------------------------------

    pub fn set_group_on(&self, grouped_light_id: &str, on: bool) -> Result<()> {
        let url = format!("{}/resource/grouped_light/{grouped_light_id}", self.base());
        let resp = self.put(&url).json(&json!({"on": {"on": on}})).send()?;
        check_v2_response(resp)
    }

    /// level: 0 = off, 1–100 = brightness percentage
    pub fn set_group_brightness(&self, grouped_light_id: &str, level: u8) -> Result<()> {
        let url = format!("{}/resource/grouped_light/{grouped_light_id}", self.base());
        let body = if level == 0 {
            json!({"on": {"on": false}})
        } else {
            // v2 brightness is a float 0.0–100.0 — maps directly to our percentage
            json!({
                "on": {"on": true},
                "dimming": {"brightness": level as f64}
            })
        };
        let resp = self.put(&url).json(&body).send()?;
        check_v2_response(resp)
    }

    /// mirek: 153 (cool/daylight) – 500 (warm/candlelight)
    pub fn set_group_color_temp(&self, grouped_light_id: &str, mirek: u16) -> Result<()> {
        let url = format!("{}/resource/grouped_light/{grouped_light_id}", self.base());
        let resp = self
            .put(&url)
            .json(&json!({
                "on": {"on": true},
                "color_temperature": {"mirek": mirek}
            }))
            .send()?;
        check_v2_response(resp)
    }

    pub fn set_group_color(&self, grouped_light_id: &str, r: u8, g: u8, b: u8) -> Result<()> {
        let (x, y) = crate::color::rgb_to_xy(r, g, b);
        let url = format!("{}/resource/grouped_light/{grouped_light_id}", self.base());
        let resp = self
            .put(&url)
            .json(&json!({
                "on": {"on": true},
                "color": {"xy": {"x": x, "y": y}}
            }))
            .send()?;
        check_v2_response(resp)
    }
}

// ---------------------------------------------------------------------------
// Response checking
// ---------------------------------------------------------------------------

fn check_errors(errors: &[V2Error]) -> Result<()> {
    if let Some(e) = errors.first() {
        return Err(anyhow!("Hue API error: {}", e.description));
    }
    Ok(())
}

fn check_v2_response(resp: reqwest::blocking::Response) -> Result<()> {
    let body: V2Response<Value> = resp.json()?;
    check_errors(&body.errors)
}
