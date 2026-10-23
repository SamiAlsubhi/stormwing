//! Persistent pilot profile: banked stars, hangar upgrades, medals and records.
//! Stored as a tiny `key=value` text file in the platform's app-data folder.

use bevy::prelude::*;

pub const STAGE_COUNT: usize = 3;
pub const UPGRADE_COUNT: usize = 5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Upgrade {
    Cannon = 0,
    FireRate = 1,
    Hull = 2,
    Missiles = 3,
    Magnet = 4,
}

pub struct UpgradeInfo {
    pub name: &'static str,
    pub blurb: &'static str,
    pub costs: &'static [u32],
}

pub const UPGRADES: [UpgradeInfo; UPGRADE_COUNT] = [
    UpgradeInfo {
        name: "Main Cannon",
        blurb: "More streams and harder-hitting rounds",
        costs: &[150, 400, 900, 1800, 3500],
    },
    UpgradeInfo {
        name: "Fire Rate",
        blurb: "Faster cannon cycling",
        costs: &[200, 550, 1200, 2400],
    },
    UpgradeInfo {
        name: "Hull Plating",
        blurb: "Absorb more hits before going down",
        costs: &[120, 320, 750, 1500, 3000],
    },
    UpgradeInfo {
        name: "Seeker Missiles",
        blurb: "Wing-mounted homing missiles",
        costs: &[450, 1100, 2500],
    },
    UpgradeInfo {
        name: "Star Magnet",
        blurb: "Pull in stars from further away",
        costs: &[100, 300, 800],
    },
];

impl Upgrade {
    pub fn max_level(self) -> u8 {
        UPGRADES[self as usize].costs.len() as u8
    }
}

#[derive(Resource, Clone, Debug)]
pub struct Profile {
    pub stars: u32,
    pub upgrades: [u8; UPGRADE_COUNT],
    /// Number of stages the pilot may select (1..=STAGE_COUNT).
    pub unlocked: usize,
    pub cleared: [bool; STAGE_COUNT],
    pub medals: [[bool; 4]; STAGE_COUNT],
    pub best: [u32; STAGE_COUNT],
    pub muted: bool,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            stars: 0,
            upgrades: [0; UPGRADE_COUNT],
            unlocked: 1,
            cleared: [false; STAGE_COUNT],
            medals: [[false; 4]; STAGE_COUNT],
            best: [0; STAGE_COUNT],
            muted: false,
        }
    }
}

impl Profile {
    pub fn level(&self, u: Upgrade) -> u8 {
        self.upgrades[u as usize]
    }

    /// Cost of the next level, or `None` when maxed out.
    pub fn next_cost(&self, u: Upgrade) -> Option<u32> {
        UPGRADES[u as usize].costs.get(self.level(u) as usize).copied()
    }

    pub fn try_buy(&mut self, u: Upgrade) -> bool {
        match self.next_cost(u) {
            Some(cost) if self.stars >= cost => {
                self.stars -= cost;
                self.upgrades[u as usize] += 1;
                true
            }
            _ => false,
        }
    }

    pub fn medal_count(&self) -> usize {
        self.medals.iter().flatten().filter(|m| **m).count()
    }

    pub fn load() -> Self {
        let mut p = Profile::default();
        let Some(text) = storage::read() else { return p };
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let nums: Vec<u32> = value.split(',').filter_map(|s| s.trim().parse().ok()).collect();
            let first = nums.first().copied().unwrap_or(0);
            match key.trim() {
                "stars" => p.stars = first,
                "unlocked" => p.unlocked = (first as usize).clamp(1, STAGE_COUNT),
                "muted" => p.muted = first != 0,
                "upgrades" => {
                    for (i, n) in nums.iter().enumerate().take(UPGRADE_COUNT) {
                        p.upgrades[i] = (*n as u8).min(UPGRADES[i].costs.len() as u8);
                    }
                }
                "best" => {
                    for (i, n) in nums.iter().enumerate().take(STAGE_COUNT) {
                        p.best[i] = *n;
                    }
                }
                "cleared" => {
                    for (i, n) in nums.iter().enumerate().take(STAGE_COUNT) {
                        p.cleared[i] = *n != 0;
                    }
                }
                k if k.starts_with("medals") => {
                    if let Ok(stage) = k["medals".len()..].parse::<usize>() {
                        if stage < STAGE_COUNT {
                            for (i, n) in nums.iter().enumerate().take(4) {
                                p.medals[stage][i] = *n != 0;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        p
    }

    pub fn save(&self) {
        let join = |v: &[u32]| v.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(",");
        let mut out = String::new();
        out += &format!("stars={}\n", self.stars);
        out += &format!("unlocked={}\n", self.unlocked);
        out += &format!("muted={}\n", self.muted as u32);
        out += &format!(
            "upgrades={}\n",
            join(&self.upgrades.map(|u| u as u32))
        );
        out += &format!("best={}\n", join(&self.best));
        out += &format!("cleared={}\n", join(&self.cleared.map(|c| c as u32)));
        for (i, m) in self.medals.iter().enumerate() {
            out += &format!("medals{}={}\n", i, join(&m.map(|b| b as u32)));
        }
        storage::write(&out);
    }
}

/// Where the profile text lives: a file on desktop, `localStorage` on the web.
#[cfg(not(target_arch = "wasm32"))]
mod storage {
    use bevy::log::warn;
    use std::path::PathBuf;

    pub fn path() -> Option<PathBuf> {
        // Lets tests use a throwaway profile instead of the real one.
        if let Some(p) = std::env::var_os("STORMWING_PROFILE") {
            return Some(PathBuf::from(p));
        }
        let base = if cfg!(target_os = "macos") {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
        } else if cfg!(target_os = "windows") {
            std::env::var_os("APPDATA").map(PathBuf::from)
        } else {
            std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        }?;
        Some(base.join("Stormwing").join("profile.txt"))
    }

    pub fn read() -> Option<String> {
        std::fs::read_to_string(path()?).ok()
    }

    pub fn write(text: &str) {
        let Some(path) = path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&path, text) {
            warn!("Could not save profile to {}: {e}", path.display());
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod storage {
    const KEY: &str = "stormwing-profile";

    fn local() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub fn read() -> Option<String> {
        local()?.get_item(KEY).ok()?
    }

    pub fn write(text: &str) {
        if let Some(storage) = local() {
            let _ = storage.set_item(KEY, text);
        }
    }
}

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Profile::load());
    }
}
