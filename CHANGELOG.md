# Changelog

## 0.1.5 (2026-10-11)

- Find: a new Get more tab that searches GitHub for ports, fan games and mashups with a Windows download, or takes a github.com link. Add one and OnionRing installs and updates it like any other port. Library and Systems link to it for systems of your own.
- Updates no longer repeat on every start. Emulators that publish a build per commit (RPCS3) update automatically at most every three days; the Updates tab still lists every release. Old downloads are cleared, keeping the two installed versions.
- A background update re-checks before installing, so it no longer reinstalls what you just updated by hand (which lost carried settings files like RPCS3's config.yml).
- An update whose download has a new layout fails without touching the working version. A newer release with no Windows download isn't offered.
- OnionRing doesn't restart for its own update while something is installing. After an update it says whether it worked, and an update that didn't take isn't retried on every start.
- A failed AnyPS5 reconversion keeps the program that worked.
- Closing the main window quits OnionRing, even with a browser game open.
- Background updates start right after first-run setup, not only after a restart.
- Scans skip OnionRing's own folders when the data folder sits inside an emulation folder.
- Systems of your own: the controller icon fits the library list, the game page shows it without a black box, and search finds games by system name.
- Pixel theme with reduced motion: cards no longer stay half faded.
- Projects added from Find keep the files they write next to themselves (saves, settings) across updates.
- A game's chosen emulator and launch options stay with the same program when another copy of that emulator is found.
- Settings has a GitHub section: an optional token lifts GitHub's limit from 60 to 5,000 requests an hour, which Find and update checks share.
- Keyboard and controller: Back closes a Get more sheet before leaving the page, and focus returns to the card you opened it from. Escape in a text box keeps what you typed, and arrow keys move out of one-line boxes. Get more and the library start on the selected tab or filter, Stats starts on its content, and arrow scrolling respects reduced motion.
- Long titles wrap and end in an ellipsis on covers and the game page; long version numbers no longer push names out of the Updates list.

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
