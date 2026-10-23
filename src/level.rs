//! Stage scripts and the director that plays them back.
//!
//! Each stage is a timeline of events: single enemy spawns (formations are
//! expanded into these when the script is built), islands with ground
//! targets, the boss warning and the boss itself.

use crate::art::Art;
use crate::audio::{PlaySfx, Sfx};
use crate::background::{spawn_island, Backdrop, THEMES};
use crate::boss::{spawn_boss, BOSSES};
use crate::common::*;
use crate::enemy::{spawn_enemy, Difficulty, EnemyKind, Move, SpawnCtx};
use crate::pickups::PowerKind;
use crate::rng::Rng;
use crate::state::{AppState, Run};
use bevy::prelude::*;

#[derive(Clone, Copy)]
pub enum Layout {
    /// A few turrets.
    Turrets(u32),
    /// Turrets plus a hut with a survivor.
    Outpost,
    /// Huts and fuel depots, lightly defended.
    Village,
    /// A heavily defended base.
    Fortress,
}

#[derive(Clone, Copy)]
pub enum Ev {
    One {
        kind: EnemyKind,
        pos: Vec2,
        mv: Move,
        drop: Option<PowerKind>,
    },
    Island {
        x: f32,
        r: f32,
        layout: Layout,
    },
    Warning,
    Boss,
}

/// Banner text shown in the middle of the screen (stage title, warnings).
#[derive(Resource, Default)]
pub struct Banner {
    pub title: String,
    pub subtitle: String,
    pub timer: f32,
    pub warning: bool,
}

#[derive(Resource, Default)]
pub struct Director {
    pub t: f32,
    events: Vec<(f32, Ev)>,
    next: usize,
    rng: Rng0,
    pub boss_time: f32,
}

#[derive(Default)]
struct Rng0(Option<Rng>);

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Director>()
            .init_resource::<Banner>()
            .add_systems(OnEnter(AppState::Playing), start_stage)
            .add_systems(Update, run_director.in_set(GameSet::Spawn))
            .add_systems(Update, tick_banner);
    }
}

fn start_stage(mut commands: Commands, run: Res<Run>, mut banner: ResMut<Banner>) {
    let events = build_script(run.stage);
    let boss_time = events
        .iter()
        .find(|(_, e)| matches!(e, Ev::Boss))
        .map(|(t, _)| *t)
        .unwrap_or(0.0);
    commands.insert_resource(Director {
        t: 0.0,
        events,
        next: 0,
        rng: Rng0(Some(Rng::new(1000 + run.stage as u64))),
        boss_time,
    });
    commands.insert_resource(ScrollSpeed::default());
    *banner = Banner {
        title: format!("STAGE {}", run.stage + 1),
        subtitle: THEMES[run.stage.min(THEMES.len() - 1)].name.to_uppercase(),
        timer: 3.0,
        warning: false,
    };
}

fn tick_banner(time: Res<Time>, mut banner: ResMut<Banner>) {
    banner.timer = (banner.timer - time.delta_secs()).max(0.0);
}

#[allow(clippy::too_many_arguments)]
fn run_director(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<Art>,
    diff: Res<Difficulty>,
    backdrop: Res<Backdrop>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut director: ResMut<Director>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    mut banner: ResMut<Banner>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    director.t += time.delta_secs();
    let theme = &THEMES[backdrop.theme];
    let mut island_rng = director.rng.0.take().unwrap_or_else(|| Rng::new(1));
    let stage = run.stage.min(BOSSES.len() - 1);
    while director.next < director.events.len() && director.events[director.next].0 <= director.t {
        let ev = director.events[director.next].1;
        director.next += 1;
        let mut ctx = SpawnCtx {
            art: &art,
            diff: *diff,
            run: &mut run,
            rng: &mut rng,
        };
        match ev {
            Ev::One { kind, pos, mv, drop } => {
                spawn_enemy(&mut commands, &mut ctx, kind, pos, mv, drop);
            }
            Ev::Island { x, r, layout } => {
                let y = HALF_H + r + 40.0;
                spawn_island(&mut commands, &mut meshes, &art, theme, &mut island_rng, Vec2::new(x, y), r);
                let centre = Vec2::new(x, y);
                let ground = |island_rng: &mut Rng, k: f32| {
                    centre + Vec2::from_angle(island_rng.range(0.0, std::f32::consts::TAU)) * r * k
                };
                let mut spawn_ground = |kind: EnemyKind, p: Vec2, ctx: &mut SpawnCtx| {
                    spawn_enemy(&mut commands, ctx, kind, p, Move::Ground(Vec2::ZERO), None);
                };
                match layout {
                    Layout::Turrets(n) => {
                        for i in 0..n {
                            let a = i as f32 / n as f32 * std::f32::consts::TAU + island_rng.range(0.0, 0.5);
                            spawn_ground(EnemyKind::Turret, centre + Vec2::from_angle(a) * r * 0.45, &mut ctx);
                        }
                    }
                    Layout::Outpost => {
                        spawn_ground(EnemyKind::Turret, centre + Vec2::new(-r * 0.4, r * 0.2), &mut ctx);
                        spawn_ground(EnemyKind::Turret, centre + Vec2::new(r * 0.4, -r * 0.1), &mut ctx);
                        spawn_ground(EnemyKind::Hut, centre + Vec2::new(0.0, -r * 0.35), &mut ctx);
                    }
                    Layout::Village => {
                        spawn_ground(EnemyKind::Hut, centre + Vec2::new(-r * 0.35, 0.0), &mut ctx);
                        spawn_ground(EnemyKind::Depot, centre + Vec2::new(r * 0.3, r * 0.3), &mut ctx);
                        spawn_ground(EnemyKind::Depot, centre + Vec2::new(r * 0.25, -r * 0.35), &mut ctx);
                        let p = ground(&mut island_rng, 0.1);
                        spawn_ground(EnemyKind::Turret, p, &mut ctx);
                    }
                    Layout::Fortress => {
                        for (dx, dy) in [(-0.45, 0.4), (0.45, 0.4), (-0.45, -0.4), (0.45, -0.4)] {
                            spawn_ground(EnemyKind::Turret, centre + Vec2::new(dx, dy) * r, &mut ctx);
                        }
                        spawn_ground(EnemyKind::Hut, centre + Vec2::new(-r * 0.12, 0.0), &mut ctx);
                        spawn_ground(EnemyKind::Hut, centre + Vec2::new(r * 0.2, -r * 0.05), &mut ctx);
                        spawn_ground(EnemyKind::Depot, centre + Vec2::new(0.0, r * 0.5), &mut ctx);
                    }
                }
            }
            Ev::Warning => {
                *banner = Banner {
                    title: "WARNING".into(),
                    subtitle: format!("{} APPROACHING", BOSSES[stage].name),
                    timer: 3.6,
                    warning: true,
                };
                sfx.write(PlaySfx::new(Sfx::Warning));
            }
            Ev::Boss => {
                spawn_boss(&mut commands, &mut ctx, stage);
            }
        }
    }
    director.rng.0 = Some(island_rng);
}

/// Jumps the director forward (used by the dev harness).
pub fn skip_to(director: &mut Director, t: f32) {
    while director.next < director.events.len() && director.events[director.next].0 < t {
        director.next += 1;
    }
    director.t = t;
}

// ---------------------------------------------------------------------------
// Script building
// ---------------------------------------------------------------------------

struct Script {
    t: f32,
    ev: Vec<(f32, Ev)>,
    rng: Rng,
}

const TOP: f32 = HALF_H + 60.0;

impl Script {
    fn new(seed: u64) -> Self {
        Self {
            t: 0.0,
            ev: Vec::new(),
            rng: Rng::new(seed),
        }
    }

    fn wait(&mut self, dt: f32) -> &mut Self {
        self.t += dt;
        self
    }

    fn push(&mut self, delay: f32, e: Ev) {
        self.ev.push((self.t + delay, e));
    }

    fn one(&mut self, delay: f32, kind: EnemyKind, pos: Vec2, mv: Move, drop: Option<PowerKind>) {
        self.push(delay, Ev::One { kind, pos, mv, drop });
    }

    /// V formation diving straight down.
    fn vee(&mut self, x: f32, n: u32, speed: f32) -> &mut Self {
        let mid = (n as f32 - 1.0) / 2.0;
        for i in 0..n {
            let k = i as f32 - mid;
            let pos = Vec2::new(x + k * 52.0, TOP + k.abs() * 46.0);
            self.one(0.0, EnemyKind::Dart, pos, Move::Linear(Vec2::new(-k * 6.0, -speed)), None);
        }
        self
    }

    /// A snaking column.
    fn column(&mut self, x: f32, n: u32, kind: EnemyKind, amp: f32) -> &mut Self {
        for i in 0..n {
            let mv = Move::Sine {
                x0: x,
                amp,
                freq: 1.8,
                vy: -190.0,
                phase: i as f32 * 0.55,
            };
            self.one(i as f32 * 0.38, kind, Vec2::new(x, TOP), mv, None);
        }
        self
    }

    /// A curving sweep in from one side and out the other.
    fn sweep(&mut self, from_left: bool, y: f32, n: u32, kind: EnemyKind, drop_last: Option<PowerKind>) -> &mut Self {
        let s = if from_left { -1.0 } else { 1.0 };
        let p = [
            Vec2::new(s * (HALF_W + 70.0), y + 120.0),
            Vec2::new(s * 120.0, y + 40.0),
            Vec2::new(-s * 60.0, y - 160.0),
            Vec2::new(-s * (HALF_W + 140.0), y - 360.0),
        ];
        for i in 0..n {
            let drop = if i == n - 1 { drop_last } else { None };
            self.one(i as f32 * 0.32, kind, p[0], Move::Curve { p, dur: 3.4, exit_speed: 280.0 }, drop);
        }
        self
    }

    /// Two mirrored sweeps at once.
    fn pincer(&mut self, y: f32, n: u32, kind: EnemyKind) -> &mut Self {
        self.sweep(true, y, n, kind, None);
        self.sweep(false, y, n, kind, None)
    }

    fn gunship(&mut self, x: f32, drop: Option<PowerKind>) -> &mut Self {
        let leave = Vec2::new(if x < 0.0 { -180.0 } else { 180.0 }, 140.0);
        let mv = Move::Hover {
            target: Vec2::new(x, HALF_H - 260.0 - self.rng.range(0.0, 60.0)),
            hold: 6.5,
            leave,
        };
        self.one(0.0, EnemyKind::Gunship, Vec2::new(x, TOP + 30.0), mv, drop);
        self
    }

    fn bomber(&mut self, x: f32, drop: Option<PowerKind>) -> &mut Self {
        self.one(0.0, EnemyKind::Bomber, Vec2::new(x, TOP + 90.0), Move::Linear(Vec2::new(0.0, -55.0)), drop);
        self
    }

    fn chasers(&mut self, n: u32) -> &mut Self {
        for i in 0..n {
            let x = self.rng.range(-HALF_W + 60.0, HALF_W - 60.0);
            self.one(i as f32 * 0.4, EnemyKind::Chaser, Vec2::new(x, TOP), Move::Chase { speed: 340.0, turn: 2.6 }, None);
        }
        self
    }

    fn boats(&mut self, x: f32, n: u32) -> &mut Self {
        for i in 0..n {
            let pos = Vec2::new(x + i as f32 * 90.0 * x.signum() * -1.0, TOP + 40.0 + i as f32 * 80.0);
            self.one(0.0, EnemyKind::Boat, pos, Move::Ground(Vec2::new(0.0, 22.0)), None);
        }
        self
    }

    fn convoy(&mut self, x: f32, n: u32) -> &mut Self {
        for i in 0..n {
            self.one(0.0, EnemyKind::Tank, Vec2::new(x, TOP + i as f32 * 70.0), Move::Ground(Vec2::new(0.0, -35.0)), None);
        }
        self
    }

    fn island(&mut self, x: f32, r: f32, layout: Layout) -> &mut Self {
        self.push(0.0, Ev::Island { x, r, layout });
        self
    }

    fn finish(mut self) -> Vec<(f32, Ev)> {
        self.wait(4.0);
        self.push(0.0, Ev::Warning);
        self.wait(4.0);
        self.push(0.0, Ev::Boss);
        self.ev.sort_by(|a, b| a.0.total_cmp(&b.0));
        self.ev
    }
}

use EnemyKind::*;
use PowerKind::*;

fn build_script(stage: usize) -> Vec<(f32, Ev)> {
    let mut s = Script::new(500 + stage as u64);
    match stage {
        0 => {
            s.wait(3.0).vee(0.0, 5, 230.0);
            s.wait(3.0).sweep(true, 320.0, 5, Swooper, None);
            s.wait(2.5).sweep(false, 220.0, 5, Swooper, Some(Weapon));
            s.wait(1.5).island(-170.0, 115.0, Layout::Outpost);
            s.wait(3.0).column(-190.0, 4, Dart, 70.0).column(190.0, 4, Dart, 70.0);
            s.wait(4.0).gunship(0.0, Some(Weapon));
            s.wait(3.0).island(180.0, 100.0, Layout::Turrets(2));
            s.wait(3.5).vee(-170.0, 3, 260.0).vee(170.0, 3, 260.0);
            s.wait(2.5).boats(220.0, 2);
            s.wait(2.5).pincer(300.0, 4, Swooper);
            s.wait(3.0).island(-150.0, 125.0, Layout::Village);
            s.wait(1.5).chasers(4);
            s.wait(3.5).bomber(-90.0, Some(Shield));
            s.wait(4.0).column(0.0, 6, Swooper, 150.0);
            s.wait(4.0).gunship(-170.0, None).gunship(170.0, Some(Repair));
            s.wait(2.0).island(160.0, 110.0, Layout::Outpost);
            s.wait(5.0).vee(0.0, 7, 250.0);
            s.wait(2.5).boats(-220.0, 2);
            s.wait(2.0).sweep(true, 260.0, 6, Dart, None);
            s.wait(2.0).sweep(false, 360.0, 6, Dart, Some(Bomb));
            s.wait(2.0).island(-160.0, 140.0, Layout::Fortress);
            s.wait(4.0).chasers(6);
            s.wait(4.0).gunship(0.0, Some(Weapon));
            s.wait(4.0).pincer(280.0, 5, Swooper);
            s.wait(3.0).bomber(120.0, Some(Repair));
            s.wait(5.0).vee(-160.0, 5, 270.0).vee(160.0, 5, 270.0);
        }
        1 => {
            s.wait(3.0).vee(0.0, 5, 250.0);
            s.wait(2.0).convoy(-200.0, 3);
            s.wait(2.5).sweep(false, 320.0, 6, Swooper, Some(Weapon));
            s.wait(2.0).island(160.0, 120.0, Layout::Outpost);
            s.wait(3.0).chasers(5);
            s.wait(3.0).gunship(-150.0, None).gunship(150.0, Some(Weapon));
            s.wait(4.0).column(-200.0, 5, Dart, 80.0).column(200.0, 5, Dart, 80.0);
            s.wait(2.5).island(-170.0, 130.0, Layout::Village);
            s.wait(2.0).convoy(180.0, 4);
            s.wait(3.0).bomber(0.0, Some(Shield));
            s.wait(3.0).pincer(300.0, 5, Dart);
            s.wait(3.0).island(150.0, 110.0, Layout::Turrets(3));
            s.wait(2.5).chasers(7);
            s.wait(3.5).gunship(0.0, Some(Repair));
            s.wait(2.0).sweep(true, 240.0, 6, Swooper, None);
            s.wait(3.0).island(-140.0, 140.0, Layout::Fortress);
            s.wait(2.0).convoy(-220.0, 4);
            s.wait(3.0).vee(-150.0, 5, 270.0).vee(150.0, 5, 270.0);
            s.wait(3.5).bomber(-140.0, None).bomber(140.0, Some(Bomb));
            s.wait(5.0).column(0.0, 7, Swooper, 170.0);
            s.wait(3.0).island(170.0, 120.0, Layout::Outpost);
            s.wait(2.5).gunship(-170.0, Some(Weapon)).gunship(170.0, None);
            s.wait(4.0).chasers(8);
            s.wait(3.0).pincer(260.0, 6, Swooper);
            s.wait(3.0).island(-150.0, 120.0, Layout::Village);
            s.wait(3.0).vee(0.0, 9, 280.0);
        }
        _ => {
            s.wait(3.0).pincer(320.0, 5, Swooper);
            s.wait(3.0).chasers(6);
            s.wait(2.0).island(-160.0, 120.0, Layout::Outpost);
            s.wait(2.5).gunship(0.0, Some(Weapon));
            s.wait(3.0).vee(-160.0, 5, 280.0).vee(160.0, 5, 280.0);
            s.wait(2.0).boats(220.0, 3);
            s.wait(3.0).bomber(-120.0, Some(Shield));
            s.wait(2.0).column(150.0, 6, Dart, 90.0);
            s.wait(3.0).island(170.0, 130.0, Layout::Fortress);
            s.wait(2.5).sweep(true, 300.0, 7, Swooper, Some(Weapon));
            s.wait(2.5).sweep(false, 200.0, 7, Dart, None);
            s.wait(3.0).gunship(-170.0, None).gunship(170.0, Some(Repair));
            s.wait(3.0).chasers(9);
            s.wait(3.0).island(-150.0, 120.0, Layout::Village);
            s.wait(2.0).boats(-220.0, 3);
            s.wait(3.0).bomber(0.0, Some(Bomb));
            s.wait(3.0).pincer(260.0, 6, Dart);
            s.wait(3.0).island(150.0, 110.0, Layout::Turrets(4));
            s.wait(2.5).column(-150.0, 7, Swooper, 120.0).column(150.0, 7, Swooper, 120.0);
            s.wait(4.0).gunship(-180.0, Some(Weapon)).gunship(0.0, None).gunship(180.0, None);
            s.wait(4.0).island(-160.0, 140.0, Layout::Fortress);
            s.wait(2.0).chasers(10);
            s.wait(3.5).bomber(-150.0, None).bomber(150.0, Some(Repair));
            s.wait(4.0).vee(0.0, 9, 300.0);
            s.wait(2.5).pincer(320.0, 7, Swooper);
        }
    }
    s.finish()
}
