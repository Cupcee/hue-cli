# hue

A command-line tool for controlling Philips Hue lights over your local Wi-Fi network.

## Requirements

- A Philips Hue bridge on your local network
- Rust toolchain (to build from source)

## Installation

```sh
cargo install --path .
```

## Setup

Run once to connect to your bridge:

```sh
hue init
```

This will:
1. Auto-discover the bridge on your network
2. Prompt you to press the physical link button on the bridge
3. Register the app and save credentials to `~/.config/hue-cli/config.json`

If auto-discovery fails, provide the bridge IP manually:

```sh
hue init --bridge-ip 192.168.1.100
```

You only need to do this once. The credentials are stored permanently and reused for every subsequent command.

## Commands

### List rooms

```sh
hue groups
```

Shows all rooms configured in your Hue app. Use these names with the other commands.

### Brightness

```sh
hue dim "living room" 80   # set to 80%
hue dim "living room" 0    # turn off
hue dim "living room" 100  # full brightness
```

Level is a percentage from 0 to 100. Setting 0 turns the lights off.

### Color

```sh
hue rgb "living room" 255 128 0    # warm orange
hue rgb "living room" 0 0 255      # blue
hue rgb "living room" 255 255 255  # white
```

Values are standard RGB, 0–255 per channel. The tool converts to the CIE xy color space the Hue API requires.

### On / Off

```sh
hue on "living room"
hue off "living room"
```

### Presets

Presets let you save a named lighting configuration and apply it with a single command.

**Save a preset** (creates or replaces):

```sh
hue preset save partymode --group "living room" --dim 60 --rgb 255,0,128
```

Both `--dim` and `--rgb` are optional, but at least one must be provided.

**Add more rooms to an existing preset:**

```sh
hue preset add partymode --group "kitchen" --dim 40 --rgb 200,0,100
```

**Apply a preset:**

```sh
hue preset apply partymode
```

**List all presets:**

```sh
hue preset list
```

**Inspect a preset:**

```sh
hue preset show partymode
```

**Delete a preset:**

```sh
hue preset delete partymode
```

## Notes

- Room names are case-insensitive — `"Living Room"` and `"living room"` both work.
- The tool uses the [Hue API v2](https://developers.meethue.com/develop/hue-api-v2/) over HTTPS. The bridge uses a self-signed certificate, which the tool accepts automatically.
- Credentials are stored in plain text at `~/.config/hue-cli/config.json`. **Anyone on your Wi-Fi network can control your lights**, and access to that file gives them a ready-made API key to do so.
