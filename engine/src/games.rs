//! Game detection: which open windows are games, what they're called, and which one to record.
//!
//! Detection only looks at windows and program files (what's open, where it's installed, whether
//! it fills the screen). It never touches a game's process, so anti-cheat has nothing to object to.
//!
//! A window is a game when, in order:
//!  1. its program is on the person's "never a game" list: no;
//!  2. its program is on their "always a game" list: yes;
//!  3. it's a known launcher, client, browser, player or part of Windows: no
//!     (the League of Legends client is a launcher; only the match window is the game);
//!  4. it's a well-known game, by program name: yes;
//!  5. it's installed in a game library folder (Steam, Epic, Riot, EA, Ubisoft, GOG, Xbox, Amazon): yes;
//!  6. it fills its monitor with no title bar (full screen or borderless), and isn't part of Windows:
//!     yes. This is the guess; the lists above override it.

use crate::platform::TopWindow;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Programs that are never games (lowercase program file names).
const NOT_GAMES: &[&str] = &[
    // Launchers and clients
    "leagueclient.exe", "leagueclientux.exe", "leagueclientuxrender.exe", "riotclientservices.exe",
    "riotclientux.exe", "riotclientuxrender.exe", "riot client.exe", "steam.exe", "steamwebhelper.exe",
    "epicgameslauncher.exe", "epiconlinservicesuserhelper.exe", "battle.net.exe", "agent.exe",
    "eadesktop.exe", "eabackgroundservice.exe", "origin.exe", "upc.exe", "ubisoftconnect.exe",
    "ubisoftgamelauncher.exe", "galaxyclient.exe", "xboxpcapp.exe", "gamingservices.exe",
    "amazon games.exe", "playnite.desktopapp.exe", "playnite.fullscreenapp.exe", "itch.exe",
    "rockstarservice.exe", "launcher.exe", "socialclubhelper.exe", "minecraftlauncher.exe",
    // Chat, music, streaming and recording
    "discord.exe", "discordptb.exe", "discordcanary.exe", "spotify.exe", "teams.exe", "ms-teams.exe",
    "slack.exe", "zoom.exe", "obs64.exe", "obs32.exe", "streamlabs obs.exe", "medal.exe",
    "outplayed.exe", "overwolf.exe", "nvidia app.exe", "nvidia share.exe", "nvcontainer.exe",
    "radeonsoftware.exe", "encore.exe", "ffmpeg.exe", "whatsapp.exe", "telegram.exe",
    // Browsers and players (full screen video isn't a game)
    "chrome.exe", "msedge.exe", "firefox.exe", "opera.exe", "opera_gx.exe", "brave.exe", "vivaldi.exe",
    "arc.exe", "librewolf.exe", "zen.exe", "vlc.exe", "mpc-hc64.exe", "mpc-hc.exe", "mpc-be64.exe",
    "mpv.exe", "potplayermini64.exe", "potplayermini.exe", "wmplayer.exe", "microsoft.media.player.exe",
    "plex.exe", "netflix.exe", "kodi.exe",
    // Windows and everyday tools
    "explorer.exe", "applicationframehost.exe", "searchhost.exe", "startmenuexperiencehost.exe",
    "shellexperiencehost.exe", "textinputhost.exe", "lockapp.exe", "widgets.exe", "msedgewebview2.exe",
    "systemsettings.exe", "taskmgr.exe", "mmc.exe", "notepad.exe", "code.exe", "devenv.exe",
    "winword.exe", "excel.exe", "powerpnt.exe", "outlook.exe", "onenote.exe", "acrobat.exe",
    "acrord32.exe", "photos.exe", "snippingtool.exe", "screenclippinghost.exe", "wallpaper64.exe",
    "wallpaper32.exe", "lively.exe", "rainmeter.exe", "steelseriesgg.exe", "lghub.exe", "icue.exe",
    "windowsterminal.exe", "cmd.exe", "powershell.exe", "pwsh.exe",
    // Screenshot and screen tools (they cover the screen for a moment while you pick an area)
    "lightshot.exe", "sharex.exe", "greenshot.exe", "snagit32.exe", "snagiteditor.exe",
    "picpick.exe", "flameshot.exe", "screenpresso.exe", "gyazo.exe", "gyazowin.exe",
    "screentogif.exe", "powertoys.exe", "powertoys.screenruler.exe", "powertoys.colorpickerui.exe",
    "magnify.exe", "osk.exe",
];

/// Well-known games, by program file name (some protect their process, so their install folder
/// can't always be read), with the name to use.
const KNOWN_GAMES: &[(&str, &str)] = &[
    ("league of legends.exe", "League of Legends"),
    ("valorant-win64-shipping.exe", "VALORANT"),
    ("fortniteclient-win64-shipping.exe", "Fortnite"),
    ("cs2.exe", "Counter-Strike 2"),
    ("dota2.exe", "Dota 2"),
    ("overwatch.exe", "Overwatch 2"),
    ("rocketleague.exe", "Rocket League"),
    ("r5apex.exe", "Apex Legends"),
    ("r5apex_dx12.exe", "Apex Legends"),
    ("warframe.x64.exe", "Warframe"),
    ("gta5.exe", "Grand Theft Auto V"),
    ("gta5_enhanced.exe", "Grand Theft Auto V"),
    ("eldenring.exe", "ELDEN RING"),
    ("cod.exe", "Call of Duty"),
    ("destiny2.exe", "Destiny 2"),
    ("rainbowsix.exe", "Rainbow Six Siege"),
    ("rainbowsix_vulkan.exe", "Rainbow Six Siege"),
    ("minecraft.windows.exe", "Minecraft"),
    ("robloxplayerbeta.exe", "Roblox"),
    ("deadlock.exe", "Deadlock"),
    ("marvel-win64-shipping.exe", "Marvel Rivals"),
    ("pathofexile.exe", "Path of Exile"),
    ("pathofexile_x64.exe", "Path of Exile"),
    ("pathofexile_x64steam.exe", "Path of Exile"),
    ("wow.exe", "World of Warcraft"),
    ("ffxiv_dx11.exe", "FINAL FANTASY XIV Online"),
];

/// Folders games are installed in (lowercase, as found in a program's path).
const LIBRARIES: &[&str] = &[
    "\\steamapps\\common\\",
    "\\epic games\\",
    "\\riot games\\",
    "\\ea games\\",
    "\\origin games\\",
    "\\ubisoft game launcher\\games\\",
    "\\gog galaxy\\games\\",
    "\\gog games\\",
    "\\xboxgames\\",
    "\\amazon games\\library\\",
];

/// Helper programs that sit in game folders but aren't the game.
const HELPER_WORDS: &[&str] = &["crash", "anticheat", "anti-cheat", "easyanticheat", "battleye", "launcher", "updater", "setup", "install", "redist", "vc_redist", "dxsetup", "uninst"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Why {
    Always,
    Known,
    Library,
    Fullscreen,
}

/// Whether `w` is a game, and why. `always` and `never` are program file names (any case).
pub fn classify(w: &TopWindow, always: &[String], never: &[String]) -> Option<Why> {
    let exe = w.exe.to_ascii_lowercase();
    if exe.is_empty() {
        return None;
    }
    if never.iter().any(|n| n.eq_ignore_ascii_case(&exe)) {
        return None;
    }
    if always.iter().any(|a| a.eq_ignore_ascii_case(&exe)) {
        return Some(Why::Always);
    }
    if NOT_GAMES.contains(&exe.as_str()) {
        return None;
    }
    if KNOWN_GAMES.iter().any(|(e, _)| *e == exe) || title_game(w).is_some() {
        return Some(Why::Known);
    }
    let path = w.path.to_ascii_lowercase();
    if LIBRARIES.iter().any(|l| path.contains(l)) && !HELPER_WORDS.iter().any(|h| exe.contains(h)) {
        return Some(Why::Library);
    }
    let in_windows = path.contains("\\windows\\") || path.contains("\\windowsapps\\microsoft.");
    if w.covers_monitor && !w.captioned && !w.minimised && !in_windows && !HELPER_WORDS.iter().any(|h| exe.contains(h)) {
        return Some(Why::Fullscreen);
    }
    None
}

/// Programs that run games written for them (Java runs Minecraft), whose own name and
/// description say nothing about the game: the window's title does.
const RUNTIMES: &[&str] = &["javaw.exe", "java.exe"];

pub fn is_runtime(exe: &str) -> bool {
    RUNTIMES.iter().any(|r| r.eq_ignore_ascii_case(exe))
}

/// A game known by its window's title, run by a runtime: Minecraft's window is called
/// "Minecraft 1.21.4" or "Minecraft* 1.21.4 - Singleplayer", whichever launcher started it.
fn title_game(w: &TopWindow) -> Option<&'static str> {
    (is_runtime(&w.exe) && w.title.trim_start().starts_with("Minecraft")).then_some("Minecraft")
}

/// The name to show and to name files and folders after. `description` is the program's own
/// description (e.g. "League of Legends (TM) Client"), if it has one.
pub fn game_name(w: &TopWindow, description: &str) -> String {
    let exe = w.exe.to_ascii_lowercase();
    if let Some((_, name)) = KNOWN_GAMES.iter().find(|(e, _)| *e == exe) {
        return (*name).to_string();
    }
    if let Some(name) = title_game(w) {
        return name.to_string();
    }
    if is_runtime(&exe) {
        // "OpenJDK Platform binary" names Java, not the game: the window's title is better.
        let from_title = clean_name(&w.title);
        if !from_title.is_empty() {
            return from_title;
        }
    }
    if let Some(name) = steam_name(Path::new(&w.path)) {
        return name;
    }
    if let Some(folder) = library_folder(&w.path) {
        let cleaned = clean_name(&folder);
        if !cleaned.is_empty() {
            return cleaned;
        }
    }
    let cleaned = clean_name(description);
    if !cleaned.is_empty() {
        return cleaned;
    }
    let from_title = clean_name(&w.title);
    if !from_title.is_empty() {
        return from_title;
    }
    clean_name(&w.exe)
}

/// Tidies a program description or file name into a game's name:
/// "League of Legends (TM) Client" -> "League of Legends", "Lethal-Win64-Shipping.exe" -> "Lethal".
pub fn clean_name(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    if s.to_ascii_lowercase().ends_with(".exe") {
        s.truncate(s.len() - 4);
    }
    for mark in ["(TM)", "(tm)", "(R)", "(r)", "™", "®", "©"] {
        s = s.replace(mark, " ");
    }
    for tail in ["-Win64-Shipping", "-WinGDK-Shipping", "-Win64-Test", "-Shipping", "_Win64", "-Win64", ".x64", "_x64", " Client"] {
        if s.len() > tail.len() && s.to_ascii_lowercase().ends_with(&tail.to_ascii_lowercase()) {
            s.truncate(s.len() - tail.len());
        }
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ").trim_matches(|c: char| c == '-' || c == '_' || c == ' ').to_string()
}

/// The folder right after a library folder: ...\Riot Games\League of Legends\Game -> "League of Legends".
fn library_folder(path: &str) -> Option<String> {
    let lower = path.to_ascii_lowercase();
    for lib in LIBRARIES {
        if let Some(i) = lower.find(lib) {
            let rest = &path[i + lib.len()..];
            let first = rest.split(['\\', '/']).next()?.trim();
            if !first.is_empty() && !first.contains('.') {
                return Some(first.to_string());
            }
        }
    }
    None
}

/// A Steam game's store name, from Steam's own install record (appmanifest_*.acf) for its folder.
pub fn steam_name(exe_path: &Path) -> Option<String> {
    let text = exe_path.to_string_lossy();
    let lower = text.to_ascii_lowercase();
    let marker = "\\steamapps\\common\\";
    let i = lower.find(marker)?;
    let steamapps = PathBuf::from(&text[..i + "\\steamapps".len()]);
    let installdir = text[i + marker.len()..].split(['\\', '/']).next()?.to_string();
    for entry in std::fs::read_dir(&steamapps).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
            continue;
        }
        let Ok(acf) = std::fs::read_to_string(entry.path()) else { continue };
        if acf_value(&acf, "installdir").map(|d| d.eq_ignore_ascii_case(&installdir)).unwrap_or(false) {
            return acf_value(&acf, "name").filter(|n| !n.trim().is_empty());
        }
    }
    None
}

/// Reads `"key"  "value"` from a Steam .acf file.
fn acf_value(acf: &str, key: &str) -> Option<String> {
    for line in acf.lines() {
        let parts: Vec<&str> = line.split('"').collect();
        // ["\t", key, "\t\t", value, ""]
        if parts.len() >= 4 && parts[1].eq_ignore_ascii_case(key) {
            return Some(parts[3].to_string());
        }
    }
    None
}

/// A game that's open.
#[derive(Debug, Clone, Serialize)]
pub struct Game {
    pub pid: u32,
    /// The window recorded (its biggest one).
    pub handle: u64,
    pub name: String,
    pub exe: String,
    #[serde(skip)]
    pub why: Option<Why>,
    /// When it was first seen (higher = newer).
    #[serde(skip)]
    opened: u64,
    /// When it last had the focus (0 = never).
    #[serde(skip)]
    focused: u64,
}

/// Which game wins when more than one is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Newest,
    Focused,
}

/// The games that are open and which one is being recorded.
#[derive(Debug, Default)]
pub struct Watch {
    pub games: Vec<Game>,
    pub locked: Option<u32>,
    counter: u64,
}

impl Watch {
    /// Updates the list from the games found now (window, name, why). Returns true if anything
    /// the interface shows changed.
    pub fn update(&mut self, found: Vec<(TopWindow, String, Why)>, foreground: u64) -> bool {
        let before: Vec<(u32, u64, String)> = self.games.iter().map(|g| (g.pid, g.handle, g.name.clone())).collect();
        // One entry per program run: its biggest window.
        let mut best: Vec<(TopWindow, String, Why)> = Vec::new();
        for (w, name, why) in found {
            match best.iter_mut().find(|(b, _, _)| b.pid == w.pid) {
                Some(b) if (w.width as u64 * w.height as u64) > (b.0.width as u64 * b.0.height as u64) => *b = (w, name, why),
                Some(_) => {}
                None => best.push((w, name, why)),
            }
        }
        self.games.retain(|g| best.iter().any(|(w, _, _)| w.pid == g.pid));
        for (w, name, why) in best {
            match self.games.iter_mut().find(|g| g.pid == w.pid) {
                Some(g) => {
                    g.handle = w.handle;
                    g.name = name;
                    g.why = Some(why);
                }
                None => {
                    self.counter += 1;
                    self.games.push(Game { pid: w.pid, handle: w.handle, name, exe: w.exe, why: Some(why), opened: self.counter, focused: 0 });
                }
            }
        }
        if foreground != 0 {
            if let Some(g) = self.games.iter_mut().find(|g| g.handle == foreground) {
                self.counter += 1;
                g.focused = self.counter;
            }
        }
        if let Some(pid) = self.locked {
            if !self.games.iter().any(|g| g.pid == pid) {
                self.locked = None;
            }
        }
        let after: Vec<(u32, u64, String)> = self.games.iter().map(|g| (g.pid, g.handle, g.name.clone())).collect();
        before != after
    }

    /// The game to record: the locked one, else the newest (or the last one used).
    pub fn active(&self, pick: Pick) -> Option<&Game> {
        if let Some(pid) = self.locked {
            if let Some(g) = self.games.iter().find(|g| g.pid == pid) {
                return Some(g);
            }
        }
        match pick {
            Pick::Newest => self.games.iter().max_by_key(|g| g.opened),
            Pick::Focused => self.games.iter().max_by_key(|g| (g.focused, g.opened)),
        }
    }

    /// Locks onto a game (None unlocks). False if that game isn't open.
    pub fn lock(&mut self, pid: Option<u32>) -> bool {
        match pid {
            None => {
                self.locked = None;
                true
            }
            Some(p) if self.games.iter().any(|g| g.pid == p) => {
                self.locked = Some(p);
                true
            }
            Some(_) => false,
        }
    }

    /// Locks onto the game whose window has the focus. Returns its name, or None if the window in
    /// front isn't a game.
    pub fn lock_window(&mut self, handle: u64) -> Option<String> {
        let g = self.games.iter().find(|g| g.handle == handle)?;
        let (pid, name) = (g.pid, g.name.clone());
        self.locked = Some(pid);
        Some(name)
    }

    /// Moves to the next open game (in the order they opened) and locks it.
    pub fn next(&mut self, pick: Pick) -> Option<String> {
        if self.games.is_empty() {
            return None;
        }
        let mut order: Vec<&Game> = self.games.iter().collect();
        order.sort_by_key(|g| g.opened);
        let current = self.active(pick).map(|g| g.pid);
        let i = current.and_then(|c| order.iter().position(|g| g.pid == c)).map(|i| (i + 1) % order.len()).unwrap_or(0);
        let (pid, name) = (order[i].pid, order[i].name.clone());
        self.locked = Some(pid);
        Some(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(pid: u32, handle: u64, exe: &str, path: &str) -> TopWindow {
        TopWindow { handle, pid, title: String::new(), exe: exe.into(), path: path.into(), width: 1920, height: 1080, ..Default::default() }
    }

    #[test]
    fn league_client_is_not_a_game_but_the_match_is() {
        let client = win(1, 1, "LeagueClientUx.exe", r"C:\Riot Games\League of Legends\LeagueClientUx.exe");
        let game = win(2, 2, "League of Legends.exe", r"C:\Riot Games\League of Legends\Game\League of Legends.exe");
        assert_eq!(classify(&client, &[], &[]), None);
        assert_eq!(classify(&game, &[], &[]), Some(Why::Known));
        assert_eq!(game_name(&game, "League of Legends (TM) Client"), "League of Legends");
        // Even when Windows won't give its path.
        let hidden = win(3, 3, "League of Legends.exe", "");
        assert_eq!(classify(&hidden, &[], &[]), Some(Why::Known));
    }

    #[test]
    fn library_folders_and_helpers() {
        let g = win(1, 1, "Lethal.exe", r"D:\SteamLibrary\steamapps\common\Lethal Company\Lethal Company.exe");
        assert_eq!(classify(&g, &[], &[]), Some(Why::Library));
        let crash = win(2, 2, "UnityCrashHandler64.exe", r"D:\SteamLibrary\steamapps\common\Lethal Company\UnityCrashHandler64.exe");
        assert_eq!(classify(&crash, &[], &[]), None);
        let epic = win(3, 3, "Hades.exe", r"C:\Program Files\Epic Games\Hades\x64\Hades.exe");
        assert_eq!(classify(&epic, &[], &[]), Some(Why::Library));
        assert_eq!(game_name(&epic, ""), "Hades");
        let launcher = win(4, 4, "EpicGamesLauncher.exe", r"C:\Program Files (x86)\Epic Games\Launcher\Portal\Binaries\Win64\EpicGamesLauncher.exe");
        assert_eq!(classify(&launcher, &[], &[]), None);
    }

    #[test]
    fn full_screen_guess_and_lists() {
        let mut fs = win(1, 1, "SomeIndie.exe", r"E:\Games\SomeIndie\SomeIndie.exe");
        assert_eq!(classify(&fs, &[], &[]), None, "windowed, not in a library: not a game");
        fs.covers_monitor = true;
        assert_eq!(classify(&fs, &[], &[]), Some(Why::Fullscreen));
        fs.captioned = true;
        assert_eq!(classify(&fs, &[], &[]), None, "a maximised app with a title bar isn't a game");
        assert_eq!(classify(&fs, &["someindie.exe".into()], &[]), Some(Why::Always));
        let mut chrome = win(2, 2, "chrome.exe", r"C:\Program Files\Google\Chrome\Application\chrome.exe");
        chrome.covers_monitor = true;
        assert_eq!(classify(&chrome, &[], &[]), None, "full screen video isn't a game");
        let lol = win(3, 3, "League of Legends.exe", "");
        assert_eq!(classify(&lol, &[], &["League of Legends.exe".into()]), None, "never list wins");
        let mut photos = win(4, 4, "SomeViewer.exe", r"C:\Windows\System32\SomeViewer.exe");
        photos.covers_monitor = true;
        assert_eq!(classify(&photos, &[], &[]), None);
        let mut shot = win(5, 5, "Lightshot.exe", r"C:\Program Files (x86)\Skillbrains\lightshot\Lightshot.exe");
        shot.covers_monitor = true;
        assert_eq!(classify(&shot, &[], &[]), None, "a screenshot tool covering the screen isn't a game");
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("League of Legends (TM) Client"), "League of Legends");
        assert_eq!(clean_name("Lethal-Win64-Shipping.exe"), "Lethal");
        assert_eq!(clean_name("Warframe.x64.exe"), "Warframe");
        assert_eq!(clean_name("Terraria"), "Terraria");
        assert_eq!(clean_name("Sea of Thieves®"), "Sea of Thieves");
    }

    #[test]
    fn steam_store_name_from_manifest() {
        let root = std::env::temp_dir().join(format!("encore-steam-{}", std::process::id()));
        let steamapps = root.join("steamapps");
        std::fs::create_dir_all(steamapps.join("common").join("Lethal Company")).unwrap();
        std::fs::write(
            steamapps.join("appmanifest_1966720.acf"),
            "\"AppState\"\n{\n\t\"appid\"\t\t\"1966720\"\n\t\"name\"\t\t\"Lethal Company\"\n\t\"installdir\"\t\t\"Lethal Company\"\n}\n",
        )
        .unwrap();
        std::fs::write(steamapps.join("appmanifest_1.acf"), "\"AppState\"\n{\n\t\"name\"\t\t\"Other\"\n\t\"installdir\"\t\t\"Other\"\n}\n").unwrap();
        // Paths use backslashes on Windows; build one the same way.
        let exe = format!("{}\\steamapps\\common\\Lethal Company\\Lethal Company.exe", root.display());
        assert_eq!(steam_name(Path::new(&exe)).as_deref(), if cfg!(windows) { Some("Lethal Company") } else { None });
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn newest_wins_lock_holds_and_releases() {
        let mut w = Watch::default();
        let a = win(10, 100, "A.exe", "");
        let b = win(20, 200, "B.exe", "");
        w.update(vec![(a.clone(), "A".into(), Why::Known)], 0);
        assert_eq!(w.active(Pick::Newest).unwrap().name, "A");
        w.update(vec![(a.clone(), "A".into(), Why::Known), (b.clone(), "B".into(), Why::Known)], 0);
        assert_eq!(w.active(Pick::Newest).unwrap().name, "B", "the newest game wins");
        // Playing A (it has the focus) only matters in "the game I'm playing" mode.
        w.update(vec![(a.clone(), "A".into(), Why::Known), (b.clone(), "B".into(), Why::Known)], 100);
        assert_eq!(w.active(Pick::Newest).unwrap().name, "B");
        assert_eq!(w.active(Pick::Focused).unwrap().name, "A");
        // Lock A; a third game opening doesn't change it.
        assert!(w.lock(Some(10)));
        let c = win(30, 300, "C.exe", "");
        w.update(vec![(a.clone(), "A".into(), Why::Known), (b.clone(), "B".into(), Why::Known), (c.clone(), "C".into(), Why::Known)], 0);
        assert_eq!(w.active(Pick::Newest).unwrap().name, "A");
        // A closes: the lock goes and the newest open game takes over.
        w.update(vec![(b.clone(), "B".into(), Why::Known), (c.clone(), "C".into(), Why::Known)], 0);
        assert_eq!(w.locked, None);
        assert_eq!(w.active(Pick::Newest).unwrap().name, "C");
        // Next game moves along and locks.
        assert_eq!(w.next(Pick::Newest).as_deref(), Some("B"));
        assert_eq!(w.active(Pick::Newest).unwrap().name, "B");
        assert_eq!(w.lock_window(300).as_deref(), Some("C"));
        assert_eq!(w.lock_window(999), None, "the window in front isn't a game");
        // Everything closes.
        w.update(vec![], 0);
        assert!(w.active(Pick::Newest).is_none());
    }

    #[test]
    fn one_entry_per_program_using_its_biggest_window() {
        let mut w = Watch::default();
        let mut splash = win(10, 1, "G.exe", "");
        splash.width = 600;
        splash.height = 400;
        let main = win(10, 2, "G.exe", "");
        w.update(vec![(splash.clone(), "G".into(), Why::Known), (main.clone(), "G".into(), Why::Known)], 0);
        assert_eq!(w.games.len(), 1);
        assert_eq!(w.games[0].handle, 2);
    }

    #[test]
    fn minecraft_is_found_and_named_from_its_window() {
        let mc = |title: &str, exe: &str| TopWindow { title: title.into(), ..win(9, 90, exe, r"C:\Users\a\AppData\Roaming\PrismLauncher\java\bin\javaw.exe") };
        for title in ["Minecraft 1.21.4", "Minecraft* 1.21.4 - Singleplayer", "Minecraft* 1.20.1 - Multiplayer (3rd-party Server)"] {
            let w = mc(title, "javaw.exe");
            assert_eq!(classify(&w, &[], &[]), Some(Why::Known), "{title}");
            assert_eq!(game_name(&w, "OpenJDK Platform binary"), "Minecraft");
        }
        // Java running something else isn't a game on its own, and is named from its window.
        let other = mc("Some Tool", "javaw.exe");
        assert_eq!(classify(&other, &[], &[]), None);
        assert_eq!(game_name(&other, "OpenJDK Platform binary"), "Some Tool");
        // Added as a game by hand: still named from its window, never "OpenJDK Platform binary".
        assert_eq!(classify(&other, &["javaw.exe".into()], &[]), Some(Why::Always));
        // A program called Minecraft that isn't Java goes the usual way.
        assert_eq!(classify(&mc("Minecraft", "notepad.exe"), &[], &[]), None);
        // Never a game still wins.
        assert_eq!(classify(&mc("Minecraft 1.21.4", "javaw.exe"), &[], &["javaw.exe".into()]), None);
    }
}
