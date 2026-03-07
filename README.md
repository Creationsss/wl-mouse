# wl-mouse

CLI tool for WLmouse gaming mice. Configure DPI, polling rate, LOD, and other settings without the browser-based web app.

## Supported Devices

BEAST MINI PRO, BEAST X PRO, BEAST X, BEAST MAX, BEAST MINI, SWORD X, YING, STRIDER, BEAST MIAO, HUAN (wired + wireless/dongle).

## Install

Download a prebuilt binary from [releases](https://heliopolis.live/creations/wl-mouse/-/releases).

### Nix (Flakes)

#### Run without installing

```bash
nix run git+https://heliopolis.live/creations/wl-mouse.git -- --help
```

#### Development Shell

Includes all system dependencies and Python libraries:

```bash
nix develop
wl-mouse --help
```

#### Install to user profile

```bash
nix profile install git+https://heliopolis.live/creations/wl-mouse.git
```

#### Add to NixOS Configuration (recommended)

```nix
# In your flake.nix or configuration.nix
{
  inputs.wl-mouse.url = "git+https://heliopolis.live/creations/wl-mouse.git";

  outputs = { self, nixpkgs, wl-mouse, ... }: {
    nixosConfigurations.your-hostname = nixpkgs.lib.nixosSystem {
      modules = [
        ({ pkgs, ... }: {
          environment.systemPackages = [
            wl-mouse.packages.${pkgs.system}.default
          ];
        })
      ];
    };
  };
}
```

Or build from source:

```
cargo install --path .
```

Requires `libudev-dev` (or `eudev-libudev-devel` on Void) for hidapi.

## Usage

The device is auto-detected. Use `-d /dev/hidrawX` to override.

```
wl-mouse list
wl-mouse info
wl-mouse profile
wl-mouse dpi
wl-mouse dpi set 1 800
wl-mouse dpi set 2 1600 --y-dpi 1200
wl-mouse dpi active 3
wl-mouse polling-rate
wl-mouse polling-rate 1000
wl-mouse lod
wl-mouse lod 2
wl-mouse debounce
wl-mouse debounce 4
wl-mouse angle-snap on
wl-mouse motion-sync off
wl-mouse ripple-control on
wl-mouse sleep-time 5
wl-mouse sleep-time 0
wl-mouse reset
```

## Permissions

On Linux, you need read/write access to the hidraw device. Add a udev rule:

```
# /etc/udev/rules.d/99-wlmouse.rules
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="36a7", MODE="0666"
```

Then reload: `sudo udevadm control --reload-rules && sudo udevadm trigger`

## License

AGPL-3.0-or-later
