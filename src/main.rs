mod color;
mod config;
mod hue;

use std::io::{self, Write as _};

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};

use config::{Config, Preset, PresetAction};
use hue::HueClient;

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "hue",
    about = "Control Philips Hue lights from the command line",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Connect to a Hue bridge (run once to set up)
    Init {
        /// Bridge IP address (skips auto-discovery)
        #[arg(long)]
        bridge_ip: Option<String>,
    },

    /// List all light groups/rooms
    Groups,

    /// Set brightness for a group (0 = off, 1–100 = percentage)
    Dim {
        /// Group/room name (e.g. "living room")
        group: String,
        /// Brightness level 0–100
        #[arg(value_parser = clap::value_parser!(u8).range(0..=100))]
        level: u8,
    },

    /// Set RGB color for all lights in a group
    Rgb {
        /// Group/room name (e.g. "living room")
        group: String,
        /// Red component 0–255
        #[arg(value_parser = clap::value_parser!(u8))]
        r: u8,
        /// Green component 0–255
        #[arg(value_parser = clap::value_parser!(u8))]
        g: u8,
        /// Blue component 0–255
        #[arg(value_parser = clap::value_parser!(u8))]
        b: u8,
    },

    /// Turn a group on
    On {
        /// Group/room name
        group: String,
    },

    /// Turn a group off
    Off {
        /// Group/room name
        group: String,
    },

    /// Manage named presets
    Preset {
        #[command(subcommand)]
        command: PresetCommands,
    },
}

#[derive(Subcommand)]
enum PresetCommands {
    /// Save a new preset (replaces any existing preset with the same name)
    Save {
        /// Preset name (e.g. "partymode")
        name: String,
        /// Group/room this preset targets
        #[arg(long)]
        group: String,
        /// Brightness level 0–100
        #[arg(long, value_parser = clap::value_parser!(u8).range(0..=100))]
        dim: Option<u8>,
        /// RGB color as r,g,b  (e.g. 255,128,0)
        #[arg(long, value_name = "R,G,B")]
        rgb: Option<Rgb>,
    },

    /// Add another group action to an existing preset
    Add {
        /// Preset name
        name: String,
        /// Group/room to add
        #[arg(long)]
        group: String,
        /// Brightness level 0–100
        #[arg(long, value_parser = clap::value_parser!(u8).range(0..=100))]
        dim: Option<u8>,
        /// RGB color as r,g,b  (e.g. 255,128,0)
        #[arg(long, value_name = "R,G,B")]
        rgb: Option<Rgb>,
    },

    /// Apply a saved preset
    Apply {
        /// Preset name
        name: String,
    },

    /// List all saved presets
    List,

    /// Show the actions in a preset
    Show {
        /// Preset name
        name: String,
    },

    /// Delete a saved preset
    Delete {
        /// Preset name
        name: String,
    },
}

// ---------------------------------------------------------------------------
// Custom "r,g,b" argument type
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Rgb(u8, u8, u8);

impl std::str::FromStr for Rgb {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let parts: Vec<&str> = s.splitn(3, ',').collect();
        if parts.len() != 3 {
            return Err(format!("Expected R,G,B format (e.g. 255,128,0), got '{s}'"));
        }
        let parse = |v: &str| {
            v.trim()
                .parse::<u8>()
                .map_err(|_| format!("'{v}' is not a valid 0–255 value"))
        };
        Ok(Rgb(parse(parts[0])?, parse(parts[1])?, parse(parts[2])?))
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { bridge_ip } => cmd_init(bridge_ip),
        Commands::Groups => cmd_groups(),
        Commands::Dim { group, level } => cmd_dim(&group, level),
        Commands::Rgb { group, r, g, b } => cmd_rgb(&group, r, g, b),
        Commands::On { group } => cmd_on(&group),
        Commands::Off { group } => cmd_off(&group),
        Commands::Preset { command } => match command {
            PresetCommands::Save {
                name,
                group,
                dim,
                rgb,
            } => cmd_preset_save(&name, &group, dim, rgb, false),
            PresetCommands::Add {
                name,
                group,
                dim,
                rgb,
            } => cmd_preset_save(&name, &group, dim, rgb, true),
            PresetCommands::Apply { name } => cmd_preset_apply(&name),
            PresetCommands::List => cmd_preset_list(),
            PresetCommands::Show { name } => cmd_preset_show(&name),
            PresetCommands::Delete { name } => cmd_preset_delete(&name),
        },
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_client(config: &Config) -> Result<HueClient> {
    Ok(HueClient::new(
        config.require_bridge_ip()?,
        config.require_username()?,
    ))
}

fn prompt(msg: &str) -> Result<String> {
    print!("{msg}");
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf.trim().to_string())
}

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

fn cmd_init(bridge_ip_arg: Option<String>) -> Result<()> {
    let mut config = Config::load()?;

    let bridge_ip = match bridge_ip_arg {
        Some(ip) => ip,
        None => {
            println!("Discovering Hue bridge...");
            match hue::discover_bridge() {
                Ok(ip) => {
                    println!("Found bridge at {ip}");
                    ip
                }
                Err(e) => {
                    eprintln!("Auto-discovery failed ({e}).");
                    prompt("Enter bridge IP address: ")?
                }
            }
        }
    };

    println!("Press the link button on your Hue bridge, then press Enter...");
    let _ = prompt("")?;

    println!("Registering...");
    let username = hue::register_app(&bridge_ip)?;

    config.bridge_ip = Some(bridge_ip.clone());
    config.username = Some(username);
    config.save()?;

    println!("Connected to bridge at {bridge_ip}. Configuration saved.");
    Ok(())
}

fn cmd_groups() -> Result<()> {
    let config = Config::load()?;
    let client = make_client(&config)?;
    let mut rooms = client.get_rooms()?;

    if rooms.is_empty() {
        println!("No rooms found.");
        return Ok(());
    }

    rooms.sort_by(|a, b| a.name.cmp(&b.name));
    println!("{}", "-".repeat(30));
    for room in &rooms {
        println!("{}", room.name);
    }
    Ok(())
}

fn cmd_dim(group: &str, level: u8) -> Result<()> {
    let config = Config::load()?;
    let client = make_client(&config)?;
    let id = client.find_group_id(group)?;
    client.set_group_brightness(&id, level)?;
    if level == 0 {
        println!("Turned off '{group}'.");
    } else {
        println!("Set brightness of '{group}' to {level}%.");
    }
    Ok(())
}

fn cmd_rgb(group: &str, r: u8, g: u8, b: u8) -> Result<()> {
    let config = Config::load()?;
    let client = make_client(&config)?;
    let id = client.find_group_id(group)?;
    client.set_group_color(&id, r, g, b)?;
    println!("Set color of '{group}' to rgb({r}, {g}, {b}).");
    Ok(())
}

fn cmd_on(group: &str) -> Result<()> {
    let config = Config::load()?;
    let client = make_client(&config)?;
    let id = client.find_group_id(group)?;
    client.set_group_on(&id, true)?;
    println!("Turned on '{group}'.");
    Ok(())
}

fn cmd_off(group: &str) -> Result<()> {
    let config = Config::load()?;
    let client = make_client(&config)?;
    let id = client.find_group_id(group)?;
    client.set_group_on(&id, false)?;
    println!("Turned off '{group}'.");
    Ok(())
}

fn cmd_preset_save(
    name: &str,
    group: &str,
    dim: Option<u8>,
    rgb: Option<Rgb>,
    append: bool,
) -> Result<()> {
    if dim.is_none() && rgb.is_none() {
        return Err(anyhow!("Specify at least --dim or --rgb"));
    }

    let mut config = Config::load()?;
    let action = PresetAction {
        group: group.to_string(),
        dim,
        rgb: rgb.map(|Rgb(r, g, b)| [r, g, b]),
    };

    if append {
        let preset = config
            .presets
            .get_mut(name)
            .ok_or_else(|| anyhow!("Preset '{name}' not found — use `preset save` to create it"))?;
        preset.actions.push(action);
        println!("Added group '{group}' to preset '{name}'.");
    } else {
        config.presets.insert(
            name.to_string(),
            Preset {
                actions: vec![action],
            },
        );
        println!("Saved preset '{name}'.");
    }

    config.save()
}

fn cmd_preset_apply(name: &str) -> Result<()> {
    let config = Config::load()?;
    let preset = config
        .presets
        .get(name)
        .ok_or_else(|| anyhow!("Preset '{name}' not found"))?
        .clone();

    let client = make_client(&config)?;

    for action in &preset.actions {
        let id = client.find_group_id(&action.group)?;
        if let Some(level) = action.dim {
            client.set_group_brightness(&id, level)?;
        }
        if let Some([r, g, b]) = action.rgb {
            client.set_group_color(&id, r, g, b)?;
        }
        println!("Applied to '{}'.", action.group);
    }

    println!("Preset '{name}' applied.");
    Ok(())
}

fn cmd_preset_list() -> Result<()> {
    let config = Config::load()?;
    if config.presets.is_empty() {
        println!("No presets saved.");
        return Ok(());
    }
    let mut names: Vec<_> = config.presets.keys().collect();
    names.sort();
    for name in names {
        let preset = &config.presets[name];
        let groups: Vec<&str> = preset.actions.iter().map(|a| a.group.as_str()).collect();
        println!("{name}  ({})", groups.join(", "));
    }
    Ok(())
}

fn cmd_preset_show(name: &str) -> Result<()> {
    let config = Config::load()?;
    let preset = config
        .presets
        .get(name)
        .ok_or_else(|| anyhow!("Preset '{name}' not found"))?;

    println!("Preset: {name}");
    for action in &preset.actions {
        print!("  group: {}", action.group);
        if let Some(d) = action.dim {
            print!("  dim: {d}%");
        }
        if let Some([r, g, b]) = action.rgb {
            print!("  rgb: ({r}, {g}, {b})");
        }
        println!();
    }
    Ok(())
}

fn cmd_preset_delete(name: &str) -> Result<()> {
    let mut config = Config::load()?;
    if config.presets.remove(name).is_none() {
        return Err(anyhow!("Preset '{name}' not found"));
    }
    config.save()?;
    println!("Deleted preset '{name}'.");
    Ok(())
}
