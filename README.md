# Stormwing

A vertical-scrolling arcade shoot-'em-up built with [Bevy](https://bevy.org) 0.20.

Fly a lone jet over three hand-scripted stages and fight a multi-part boss at the end of each. Collect the stars that destroyed enemies drop, rescue stranded survivors and earn medals. Between missions, spend your stars in the hangar on permanent upgrades.

All art, sound effects and music are generated procedurally at startup, so the game ships with no asset files.

**Play in your browser:** https://samialsubhi.github.io/stormwing/ (works on phones too)

## Running

```bash
cargo run --release
```

The first build compiles Bevy and takes a few minutes. Debug builds also run well (dependencies are optimized), and `cargo run --features dev` links Bevy dynamically for faster rebuilds while you develop.

## Controls

| Action | Keyboard / mouse | Gamepad |
| --- | --- | --- |
| Fly | WASD / arrow keys, or hold left mouse to steer to the cursor | Left stick / D-pad |
| Precise (slow) flying | Shift | - |
| Bomb (clears bullets, damages everything) | Space / X / B / right click | A / B |
| Pause | Esc / P | Start |
| Mute | M | - |

The cannon fires automatically.

On a touch screen, drag one finger anywhere to fly. The plane follows your finger's movement, so your finger never hides it. Lifting your finger pauses the game; touch the lower half of the screen to keep flying. Tap with a second finger to drop a bomb.

## How it plays

- **Stars** are the currency. You keep every star you pick up, even if the mission fails, so each attempt makes progress.
- **Power-ups** drop from gunships, bombers and some formation leaders:
  - **Weapon:** more cannon streams for the rest of the stage.
  - **Shield:** absorbs hits for 9 seconds.
  - **Repair:** restores hull.
  - **Bomb:** one extra bomb.
- **Survivors** appear when you destroy blue-roofed huts. Hover over them until the bar fills to rescue them.
- **Bosses** are shielded until you destroy their turrets. After that, the core cycles through spiral, fan and ring bullet patterns, and it gets more aggressive below half health.
- **Medals** are awarded per stage for destroying 70% of enemies, destroying every enemy, taking no damage and rescuing every survivor. Each medal grants bonus stars the first time you earn it.
- **Hangar upgrades:** Main Cannon, Fire Rate, Hull Plating, Seeker Missiles and Star Magnet.

Stages:

| Stage | Setting | Boss |
| --- | --- | --- |
| 1 | Coral Coast | Tidebreaker |
| 2 | Ember Dunes | Dune Colossus |
| 3 | Frostbite Reach | Aurora Citadel |

Clearing a stage unlocks the next one.

Progress is saved to:

| Platform | Save location |
| --- | --- |
| macOS | `~/Library/Application Support/Stormwing/profile.txt` |
| Windows | `%APPDATA%\Stormwing\profile.txt` |
| Linux | `~/.local/share/Stormwing/profile.txt` |

## Web build

The game also runs in the browser as WebAssembly (WebGL2). Every push to `main` is built by GitHub Actions (`.github/workflows/pages.yml`) and deployed to GitHub Pages.

To build it locally:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129   # must match wasm-bindgen in Cargo.lock
./build-web.sh                                     # writes ./dist
python3 -m http.server --directory dist 8080       # then open http://localhost:8080
```

On the web, progress is saved in the browser's `localStorage`.

## Project layout

| File | Purpose |
| --- | --- |
| `src/main.rs` | App setup, camera (bloom and a fixed 720×1080 play field) |
| `src/state.rs` | App states, pause, end-of-run rewards and medals |
| `src/level.rs` | Stage scripts and the director that plays them |
| `src/player.rs` | Movement, cannon patterns, seeker missiles, bombs |
| `src/enemy.rs` | Enemy types, movement patterns, guns |
| `src/boss.rs` | Multi-part bosses and their attack phases |
| `src/combat.rs` | Projectiles, collisions, damage, destruction |
| `src/pickups.rs` | Stars, power-ups, survivors |
| `src/background.rs` | Stage themes, procedural islands, clouds, roads |
| `src/effects.rs` | Particles, explosions, screen shake, flashes |
| `src/hud.rs` | In-game HUD and pause menu |
| `src/menu.rs` | Title, mission select, hangar and results screens |
| `src/touch.rs` | Touch controls: drag to fly, lift to pause, second finger to bomb |
| `src/art.rs`, `src/shapes.rs`, `src/textures.rs` | Procedural vector art and textures |
| `src/audio.rs` | Synthesized sound effects and the music loop |
| `src/save.rs` | Profile persistence (a file on desktop, `localStorage` on the web) and upgrade definitions |
| `src/dev.rs` | Test harness driven by environment variables (desktop only) |
| `web/index.html`, `build-web.sh` | Web page and build script for the WebAssembly version |

## Testing harness

Setting any of these environment variables turns on the harness in `src/dev.rs`:

| Variable | Effect |
| --- | --- |
| `STORMWING_STAGE=1..3` | Start straight in a stage |
| `STORMWING_SKIP=secs` | Fast-forward the stage script (the stage 1 boss arrives at about 88 s) |
| `STORMWING_GOD=1` | Invulnerable player flown by an autopilot |
| `STORMWING_SCREEN=hangar\|select\|results` | Open a menu screen on launch |
| `STORMWING_STARS=n` | Grant stars and unlock every stage |
| `STORMWING_SHOTS=dir` with `STORMWING_SHOT_AT=5,10` | Save screenshots at those times (seconds since launch) |
| `STORMWING_QUIT=secs` | Exit automatically |
| `STORMWING_MUTE=1` | Start muted |
| `STORMWING_FPS=1` | Log the frame rate |
| `STORMWING_PROFILE=path` | Use a separate save file |

Example: watch the first boss fight without touching your real save.

```bash
STORMWING_PROFILE=/tmp/test-profile.txt STORMWING_STAGE=1 STORMWING_SKIP=84 STORMWING_GOD=1 cargo run --release
```
