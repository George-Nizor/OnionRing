# Changelog

## 0.1.0 (2026-10-10)

First version.

- Console-style home: the selected game's art fills the window, platform tabs across the top, a cover shelf (or grid) below.
- Adopts existing emulation folders in place: detects RPCS3, PCSX2, Dolphin, Citra/Azahar, DeSmuME/melonDS and more, and finds games by extension, folder names, disc headers (GameCube/Wii) and PARAM.SFO (PS3/PS4). Reads RPCS3's `games.yml`.
- Groups copies of the same game (ISO + extracted folder + the archive it came in) and offers to move redundant copies to a reversible Reclaim folder.
- Ports & recomps: 225 Windows ports/recomps/decomps from the reference catalog, installed from GitHub Releases with SHA-256 verification, versioned folders and saves kept across updates.
- Emulator installs from GitHub Releases, with portable data kept across updates.
- Box art from the libretro thumbnail project, PS3 disc art, typographic covers for everything else. Custom covers.
- Playtime tracking, favourites, hidden games, search, keyboard and controller navigation.
- Signed in-app updates from GitHub Releases.
