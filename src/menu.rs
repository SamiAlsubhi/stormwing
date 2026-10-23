//! Title screen, mission select, hangar (upgrades) and results screens.

use crate::art::Art;
use crate::audio::{PlaySfx, Sfx};
use crate::background::THEMES;
use crate::boss::BOSSES;
use crate::common::z;
use crate::save::{Profile, Upgrade, STAGE_COUNT, UPGRADES, UPGRADE_COUNT};
use crate::state::{AppState, Outcome, PlayState, Run, MEDAL_NAMES};
use crate::touch::TouchInput;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::Pressed;
use bevy::ui_widgets::{Activate, Button};

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Start,
    Hangar,
    Quit,
    Menu,
    Launch(usize),
    Buy(usize),
    Retry,
    Resume,
    Abort,
}

/// Greyed-out buttons still react to hover but do nothing useful.
#[derive(Component)]
pub struct Disabled;

#[derive(Component)]
struct HangarRoot;

#[derive(Component)]
struct MenuPlane;

const PANEL: Color = Color::srgba(0.02, 0.05, 0.11, 0.82);
const ACCENT: Color = Color::srgb(0.45, 0.85, 1.0);
const GOLD: Color = Color::srgb(1.0, 0.85, 0.35);
const DIM: Color = Color::srgba(1.0, 1.0, 1.0, 0.55);

const BTN: Color = Color::srgb(0.08, 0.2, 0.32);
const BTN_HOVER: Color = Color::srgb(0.13, 0.33, 0.5);
const BTN_PRESS: Color = Color::srgb(0.2, 0.5, 0.7);
const BTN_DISABLED: Color = Color::srgb(0.12, 0.13, 0.16);

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Menu), spawn_main_menu)
            .add_systems(OnEnter(AppState::StageSelect), spawn_stage_select)
            .add_systems(OnEnter(AppState::Hangar), spawn_hangar_screen)
            .add_systems(OnEnter(AppState::Results), spawn_results)
            .add_systems(Update, (style_buttons, menu_keys, bob_menu_plane, scale_ui))
            .add_observer(on_activate);
    }
}

// ---------------------------------------------------------------------------
// Widgets
// ---------------------------------------------------------------------------

pub const SHADOW_ALPHA: f32 = 0.55;

pub fn text(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
        TextShadow {
            offset: Vec2::new(0.0, (size / 14.0).clamp(1.0, 3.0)),
            color: Color::srgba(0.0, 0.0, 0.0, SHADOW_ALPHA),
        },
    )
}

pub fn button(label: &str, action: Action, width: f32) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        action,
        Node {
            width: px(width),
            height: px(50),
            border: UiRect::all(px(2)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(10)),
            ..default()
        },
        BorderColor::all(ACCENT.with_alpha(0.5)),
        BackgroundColor(BTN),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Px(20.0),
                ..default()
            },
            TextColor(Color::WHITE),
            Pickable::IGNORE,
        )],
    )
}

fn small_button(label: &str, action: Action, width: f32, enabled: bool) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        action,
        Node {
            width: px(width),
            height: px(38),
            border: UiRect::all(px(2)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(8)),
            ..default()
        },
        BorderColor::all(ACCENT.with_alpha(if enabled { 0.5 } else { 0.15 })),
        BackgroundColor(if enabled { BTN } else { BTN_DISABLED }),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Px(16.0),
                ..default()
            },
            TextColor(if enabled { Color::WHITE } else { DIM }),
            Pickable::IGNORE,
        )],
    )
}

fn screen_root(state: AppState) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(12),
            ..default()
        },
        DespawnOnExit(state),
    )
}

fn panel() -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(10),
            padding: UiRect::all(px(20)),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(16)),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(ACCENT.with_alpha(0.25)),
    )
}

fn star_count(art: &Art, stars: u32, size: f32) -> impl Bundle {
    (
        Node {
            align_items: AlignItems::Center,
            column_gap: px(6),
            ..default()
        },
        children![
            (
                ImageNode::new(art.tex_star_icon.clone()),
                Node {
                    width: px(size),
                    height: px(size),
                    ..default()
                },
            ),
            text(stars.to_string(), size, GOLD),
        ],
    )
}

fn medal(art: &Art, earned: bool, size: f32) -> impl Bundle {
    (
        ImageNode::new(art.tex_medal.clone()).with_color(if earned {
            Color::srgb(1.0, 0.8, 0.3)
        } else {
            Color::srgba(0.5, 0.55, 0.6, 0.4)
        }),
        Node {
            width: px(size),
            height: px(size),
            ..default()
        },
    )
}

fn style_buttons(
    mut q: Query<
        (&Hovered, Has<Pressed>, Has<Disabled>, &mut BackgroundColor, &mut BorderColor),
        With<Action>,
    >,
) {
    for (hovered, pressed, disabled, mut bg, mut border) in &mut q {
        let (color, edge) = match (disabled, pressed, hovered.get()) {
            (true, _, _) => (BTN_DISABLED, ACCENT.with_alpha(0.15)),
            (false, true, _) => (BTN_PRESS, Color::WHITE),
            (false, false, true) => (BTN_HOVER, ACCENT),
            _ => (BTN, ACCENT.with_alpha(0.5)),
        };
        if bg.0 != color {
            bg.0 = color;
        }
        border.set_all(edge);
    }
}

/// Scale the UI with the window so the layout looks the same at any size.
fn scale_ui(windows: Query<&Window>, mut scale: ResMut<UiScale>) {
    let Ok(w) = windows.single() else { return };
    let s = (w.height() / 900.0).min(w.width() / 600.0).clamp(0.5, 3.0);
    if (scale.0 - s).abs() > 0.01 {
        scale.0 = s;
    }
}

// ---------------------------------------------------------------------------
// Screens
// ---------------------------------------------------------------------------

fn spawn_main_menu(mut commands: Commands, art: Res<Art>, profile: Res<Profile>, touch: Res<TouchInput>) {
    commands.spawn((
        Mesh2d(art.player.clone()),
        MeshMaterial2d(art.mat_base.clone()),
        Transform::from_xyz(0.0, -440.0, z::PLAYER).with_scale(Vec3::splat(1.4)),
        MenuPlane,
        DespawnOnExit(AppState::Menu),
        children![
            (
                Mesh2d(art.player.clone()),
                MeshMaterial2d(art.mat_shadow.clone()),
                Transform::from_xyz(30.0, -40.0, -14.0).with_scale(Vec3::splat(0.8)),
            ),
            (
                Sprite {
                    image: art.tex_glow.clone(),
                    custom_size: Some(Vec2::new(16.0, 40.0)),
                    color: Color::linear_rgb(3.5, 1.8, 0.7),
                    ..default()
                },
                Transform::from_xyz(0.0, -52.0, -0.1),
            ),
        ],
    ));

    commands.spawn(screen_root(AppState::Menu)).with_children(|root| {
        root.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                margin: UiRect::bottom(px(30)),
                ..default()
            },
            children![
                text("STORMWING", 72.0, GOLD),
                text("ARCADE SKY COMBAT", 18.0, ACCENT),
            ],
        ));
        root.spawn(panel()).with_children(|p| {
            p.spawn(star_count(&art, profile.stars, 26.0));
            p.spawn(text(
                format!("Medals {}/{}", profile.medal_count(), STAGE_COUNT * 4),
                15.0,
                DIM,
            ));
            p.spawn(Node {
                height: px(6),
                ..default()
            });
            p.spawn(button("Start Mission", Action::Start, 280.0));
            p.spawn(button("Hangar", Action::Hangar, 280.0));
            // Browsers can't be closed from inside the page.
            if !cfg!(target_arch = "wasm32") {
                p.spawn(button("Quit", Action::Quit, 280.0));
            }
        });
        let hints: &[&str] = if touch.show_touch_hints() {
            &[
                "Drag one finger anywhere to fly",
                "Lift your finger to pause",
                "Tap with a second finger to drop a bomb",
            ]
        } else {
            &[
                "Move: WASD / arrows / hold left mouse / gamepad",
                "Shift: precise flying   Space / right click: bomb",
                "Esc: pause   M: mute",
            ]
        };
        root.spawn(Node {
            margin: UiRect::top(px(16)),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(4),
            ..default()
        })
        .with_children(|h| {
            for line in hints {
                h.spawn(text(*line, 14.0, DIM));
            }
        });
    });
}

fn bob_menu_plane(time: Res<Time>, mut q: Query<&mut Transform, With<MenuPlane>>) {
    for mut t in &mut q {
        let s = time.elapsed_secs();
        t.translation.x = (s * 0.6).sin() * 30.0;
        t.translation.y = -440.0 + (s * 1.1).sin() * 10.0;
        t.scale.x = 1.4 * (1.0 - ((s * 0.6).cos() * 0.12).abs());
    }
}

fn spawn_stage_select(mut commands: Commands, art: Res<Art>, profile: Res<Profile>) {
    commands.spawn(screen_root(AppState::StageSelect)).with_children(|root| {
        root.spawn(text("SELECT MISSION", 40.0, Color::WHITE));
        root.spawn(star_count(&art, profile.stars, 22.0));
        for i in 0..STAGE_COUNT {
            let unlocked = i < profile.unlocked;
            root.spawn((
                Node {
                    width: px(520),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(12)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(if profile.cleared[i] { GOLD.with_alpha(0.5) } else { ACCENT.with_alpha(0.2) }),
            ))
            .with_children(|card| {
                card.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                    ..default()
                })
                .with_children(|col| {
                    col.spawn(text(format!("STAGE {}", i + 1), 14.0, ACCENT));
                    col.spawn(text(THEMES[i].name, 24.0, if unlocked { Color::WHITE } else { DIM }));
                    col.spawn(text(
                        if unlocked {
                            format!("Boss: {}   Best: {}", BOSSES[i].name, profile.best[i])
                        } else {
                            "Clear the previous stage to unlock".to_string()
                        },
                        13.0,
                        DIM,
                    ));
                    col.spawn(Node {
                        column_gap: px(6),
                        margin: UiRect::top(px(4)),
                        ..default()
                    })
                    .with_children(|m| {
                        for k in 0..4 {
                            m.spawn(medal(&art, profile.medals[i][k], 24.0));
                        }
                    });
                });
                let mut b = card.spawn(small_button(
                    if unlocked { "Launch" } else { "Locked" },
                    Action::Launch(i),
                    120.0,
                    unlocked,
                ));
                if !unlocked {
                    b.insert(Disabled);
                }
            });
        }
        root.spawn(Node {
            height: px(4),
            ..default()
        });
        root.spawn(button("Back", Action::Menu, 220.0));
        root.spawn(text("Press 1-3 to launch, Esc to go back", 13.0, DIM));
    });
}

fn spawn_hangar_screen(mut commands: Commands, art: Res<Art>, profile: Res<Profile>) {
    spawn_hangar(&mut commands, &art, &profile);
}

fn spawn_hangar(commands: &mut Commands, art: &Art, profile: &Profile) {
    commands
        .spawn((screen_root(AppState::Hangar), HangarRoot))
        .with_children(|root| {
            root.spawn(text("HANGAR", 40.0, Color::WHITE));
            root.spawn(star_count(art, profile.stars, 24.0));
            for i in 0..UPGRADE_COUNT {
                let u = upgrade_from_index(i);
                let info = &UPGRADES[i];
                let level = profile.level(u);
                let cost = profile.next_cost(u);
                let affordable = cost.is_some_and(|c| profile.stars >= c);
                root.spawn((
                    Node {
                        width: px(540),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        padding: UiRect::axes(px(14), px(10)),
                        border_radius: BorderRadius::all(px(12)),
                        ..default()
                    },
                    BackgroundColor(PANEL),
                ))
                .with_children(|row| {
                    row.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(3),
                        ..default()
                    })
                    .with_children(|col| {
                        col.spawn(text(info.name, 20.0, Color::WHITE));
                        col.spawn(text(info.blurb, 13.0, DIM));
                        col.spawn(Node {
                            column_gap: px(4),
                            margin: UiRect::top(px(3)),
                            ..default()
                        })
                        .with_children(|pips| {
                            for k in 0..u.max_level() {
                                pips.spawn((
                                    Node {
                                        width: px(22),
                                        height: px(8),
                                        border_radius: BorderRadius::all(px(3)),
                                        ..default()
                                    },
                                    BackgroundColor(if k < level { ACCENT } else { Color::srgba(1.0, 1.0, 1.0, 0.15) }),
                                ));
                            }
                        });
                    });
                    let label = match cost {
                        Some(c) => format!("{c}"),
                        None => "MAX".to_string(),
                    };
                    row.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: px(2),
                        ..default()
                    })
                    .with_children(|col| {
                        if cost.is_some() {
                            col.spawn(star_count(art, cost.unwrap_or(0), 16.0));
                        }
                        let mut b = col.spawn(small_button(
                            if cost.is_some() { "Upgrade" } else { &label },
                            Action::Buy(i),
                            120.0,
                            affordable,
                        ));
                        if !affordable {
                            b.insert(Disabled);
                        }
                    });
                });
            }
            root.spawn(Node {
                height: px(4),
                ..default()
            });
            root.spawn(Node {
                column_gap: px(12),
                ..default()
            })
            .with_children(|row| {
                row.spawn(button("Back", Action::Menu, 200.0));
                row.spawn(button("Missions", Action::Start, 200.0));
            });
        });
}

fn upgrade_from_index(i: usize) -> Upgrade {
    match i {
        0 => Upgrade::Cannon,
        1 => Upgrade::FireRate,
        2 => Upgrade::Hull,
        3 => Upgrade::Missiles,
        _ => Upgrade::Magnet,
    }
}

fn spawn_results(mut commands: Commands, art: Res<Art>, run: Res<Run>, profile: Res<Profile>) {
    let won = run.outcome == Some(Outcome::Victory);
    commands.spawn(screen_root(AppState::Results)).with_children(|root| {
        root.spawn(text(
            if won { "MISSION COMPLETE" } else { "MISSION FAILED" },
            44.0,
            if won { GOLD } else { Color::srgb(1.0, 0.4, 0.35) },
        ));
        root.spawn(text(
            format!("Stage {} - {}", run.stage + 1, THEMES[run.stage.min(STAGE_COUNT - 1)].name),
            18.0,
            ACCENT,
        ));
        root.spawn(panel()).with_children(|p| {
            let line = |label: &str, value: String| {
                (
                    Node {
                        width: px(380),
                        justify_content: JustifyContent::SpaceBetween,
                        ..default()
                    },
                    children![text(label.to_string(), 18.0, DIM), text(value, 18.0, Color::WHITE)],
                )
            };
            p.spawn(line(
                "Score",
                format!("{}{}", run.score, if run.new_best { "  NEW BEST" } else { "" }),
            ));
            p.spawn(line("Stars collected", format!("{}", run.stars)));
            p.spawn(line(
                "Enemies destroyed",
                format!("{}%  ({}/{})", (run.kill_ratio() * 100.0).floor() as u32, run.kills, run.spawned),
            ));
            p.spawn(line("Survivors rescued", format!("{}/{}", run.rescued, run.survivors)));
            p.spawn(line("Hull damage", if run.damaged { "Yes".into() } else { "None".into() }));

            if won {
                p.spawn(Node {
                    height: px(6),
                    ..default()
                });
                for i in 0..4 {
                    let earned = profile.medals[run.stage.min(STAGE_COUNT - 1)][i];
                    p.spawn(Node {
                        width: px(380),
                        align_items: AlignItems::Center,
                        column_gap: px(10),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn(medal(&art, earned, 28.0));
                        row.spawn(text(MEDAL_NAMES[i], 16.0, if earned { Color::WHITE } else { DIM }));
                        if run.new_medals[i] {
                            row.spawn(text("NEW!", 16.0, GOLD));
                        }
                    });
                }
                if run.medal_bonus > 0 {
                    p.spawn(text(format!("Medal bonus: +{} stars", run.medal_bonus), 16.0, GOLD));
                }
                if run.unlocked_next {
                    p.spawn(text(
                        format!("Stage {} unlocked!", run.stage + 2),
                        18.0,
                        Color::srgb(0.6, 1.0, 0.6),
                    ));
                }
            } else {
                p.spawn(text("Stars you collected are kept. Upgrade and try again!", 14.0, DIM));
            }
            p.spawn(Node {
                height: px(6),
                ..default()
            });
            p.spawn(star_count(&art, profile.stars, 22.0));
        });
        root.spawn(Node {
            column_gap: px(10),
            ..default()
        })
        .with_children(|row| {
            row.spawn(button("Retry", Action::Retry, 160.0));
            row.spawn(button("Hangar", Action::Hangar, 160.0));
            row.spawn(button("Menu", Action::Menu, 160.0));
        });
        if won && run.stage + 1 < STAGE_COUNT {
            root.spawn(button("Next Stage", Action::Launch(run.stage + 1), 300.0));
        }
    });
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn on_activate(
    ev: On<Activate>,
    mut commands: Commands,
    actions: Query<(&Action, Has<Disabled>)>,
    art: Res<Art>,
    mut profile: ResMut<Profile>,
    mut run: ResMut<Run>,
    mut next: ResMut<NextState<AppState>>,
    next_play: Option<ResMut<NextState<PlayState>>>,
    hangar: Query<Entity, With<HangarRoot>>,
    mut sfx: MessageWriter<PlaySfx>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((&action, disabled)) = actions.get(ev.entity) else {
        return;
    };
    if disabled {
        sfx.write(PlaySfx::new(Sfx::Denied));
        return;
    }
    sfx.write(PlaySfx::new(Sfx::Click));
    match action {
        Action::Start => next.set(AppState::StageSelect),
        Action::Hangar => next.set(AppState::Hangar),
        Action::Quit => {
            exit.write(AppExit::Success);
        }
        Action::Menu => next.set(AppState::Menu),
        Action::Launch(i) => launch(&mut run, &mut next, &profile, i),
        Action::Retry => {
            let stage = run.stage;
            launch(&mut run, &mut next, &profile, stage);
        }
        Action::Buy(i) => {
            if profile.try_buy(upgrade_from_index(i)) {
                profile.save();
                sfx.write(PlaySfx::new(Sfx::PowerUp));
                for e in &hangar {
                    commands.entity(e).despawn();
                }
                spawn_hangar(&mut commands, &art, &profile);
            }
        }
        Action::Resume => {
            if let Some(mut np) = next_play {
                np.set(PlayState::Running);
            }
        }
        Action::Abort => next.set(AppState::StageSelect),
    }
}

fn launch(run: &mut Run, next: &mut NextState<AppState>, profile: &Profile, stage: usize) {
    if stage < profile.unlocked.min(STAGE_COUNT) {
        *run = Run::new(stage);
        next.set(AppState::Playing);
    }
}

fn menu_keys(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    state: Res<State<AppState>>,
    profile: Res<Profile>,
    mut run: ResMut<Run>,
    mut next: ResMut<NextState<AppState>>,
) {
    let confirm = keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter])
        || pads.iter().any(|p| p.just_pressed(GamepadButton::South));
    let back = keys.just_pressed(KeyCode::Escape) || pads.iter().any(|p| p.just_pressed(GamepadButton::East));
    match state.get() {
        AppState::Menu => {
            if confirm {
                next.set(AppState::StageSelect);
            } else if keys.just_pressed(KeyCode::KeyH) {
                next.set(AppState::Hangar);
            }
        }
        AppState::StageSelect => {
            let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3];
            for (i, k) in digits.iter().enumerate() {
                if keys.just_pressed(*k) {
                    launch(&mut run, &mut next, &profile, i);
                }
            }
            if confirm {
                let stage = (profile.unlocked - 1).min(STAGE_COUNT - 1);
                launch(&mut run, &mut next, &profile, stage);
            } else if back {
                next.set(AppState::Menu);
            }
        }
        AppState::Hangar => {
            if back {
                next.set(AppState::Menu);
            }
        }
        AppState::Results => {
            if confirm || keys.just_pressed(KeyCode::KeyR) {
                let stage = run.stage;
                launch(&mut run, &mut next, &profile, stage);
            } else if keys.just_pressed(KeyCode::KeyH) {
                next.set(AppState::Hangar);
            } else if back {
                next.set(AppState::Menu);
            }
        }
        AppState::Playing => {}
    }
}
