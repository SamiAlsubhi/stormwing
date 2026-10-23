//! In-stage HUD: score, stars, hull, bombs, power, boss health and banners.

use crate::art::Art;
use crate::boss::BossStatus;
use crate::level::{Banner, Director};
use crate::menu::{button, text, Action, SHADOW_ALPHA};
use crate::player::{Player, MAX_POWER};
use crate::state::{AppState, PlayState, Run};
use crate::touch::TouchInput;
use bevy::prelude::*;

#[derive(Component)]
struct ScoreText;
#[derive(Component)]
struct StarText;
#[derive(Component)]
struct HullFill;
#[derive(Component)]
struct BombText;
#[derive(Component)]
struct PowerText;
#[derive(Component)]
struct BossPanel;
#[derive(Component)]
struct BossName;
#[derive(Component)]
struct BossFill;
#[derive(Component)]
struct ProgressFill;
#[derive(Component)]
struct BannerTitle;
#[derive(Component)]
struct BannerSub;

const PANEL: Color = Color::srgba(0.02, 0.05, 0.1, 0.55);
const ACCENT: Color = Color::srgb(0.45, 0.85, 1.0);
const GOLD: Color = Color::srgb(1.0, 0.85, 0.35);

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Playing), spawn_hud)
            .add_systems(OnEnter(PlayState::Paused), spawn_pause)
            .add_systems(Update, update_hud.run_if(in_state(AppState::Playing)));
    }
}

/// The play field always maps to a 600x900 block of UI space (see
/// `menu::scale_ui`), so the HUD lives in a centred frame of that size and
/// lines up with the arena whatever the window shape.
fn arena_frame() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: percent(100),
        height: percent(100),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

fn spawn_hud(mut commands: Commands, art: Res<Art>, touch: Res<TouchInput>) {
    let hint = if touch.show_touch_hints() {
        "Lift finger to pause   2nd finger: bomb"
    } else {
        "SPACE bomb  ESC pause  M mute"
    };
    commands
        .spawn((arena_frame(), Pickable::IGNORE, DespawnOnExit(AppState::Playing)))
        .with_children(|outer| {
            outer
                .spawn((
                    Node {
                        width: px(600),
                        height: px(900),
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|root| {
            // Stage progress along the very top edge.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: px(0),
                    left: px(0),
                    width: percent(100),
                    height: px(4),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            ))
            .with_child((
                ProgressFill,
                Node {
                    width: percent(0),
                    height: percent(100),
                    ..default()
                },
                BackgroundColor(ACCENT.with_alpha(0.8)),
            ));

            // Score (top-left).
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: px(12),
                    left: px(14),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                children![
                    text("SCORE", 13.0, ACCENT),
                    (ScoreText, text("0", 28.0, Color::WHITE)),
                ],
            ));

            // Stars (top-right).
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: px(14),
                    right: px(14),
                    align_items: AlignItems::Center,
                    column_gap: px(6),
                    padding: UiRect::axes(px(10), px(4)),
                    border_radius: BorderRadius::all(px(14)),
                    ..default()
                },
                BackgroundColor(PANEL),
                children![
                    (
                        ImageNode::new(art.tex_star_icon.clone()),
                        Node {
                            width: px(24),
                            height: px(24),
                            ..default()
                        },
                    ),
                    (StarText, text("0", 24.0, GOLD)),
                ],
            ));

            // Boss health (top-centre).
            root.spawn((
                BossPanel,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(70),
                    left: percent(12),
                    width: percent(76),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(4),
                    display: Display::None,
                    ..default()
                },
            ))
            .with_children(|p| {
                p.spawn((BossName, text("", 16.0, Color::srgb(1.0, 0.55, 0.45))));
                p.spawn((
                    Node {
                        width: percent(100),
                        height: px(12),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(6)),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                ))
                .with_child((
                    BossFill,
                    Node {
                        width: percent(100),
                        height: percent(100),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.95, 0.3, 0.25)),
                ));
            });

            // Hull, bombs, power (bottom-left).
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: px(14),
                    left: px(14),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                    padding: UiRect::all(px(8)),
                    border_radius: BorderRadius::all(px(8)),
                    ..default()
                },
                BackgroundColor(PANEL),
            ))
            .with_children(|p| {
                p.spawn(text("HULL", 12.0, ACCENT));
                p.spawn((
                    Node {
                        width: px(170),
                        height: px(12),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(6)),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.5)),
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                ))
                .with_child((
                    HullFill,
                    Node {
                        width: percent(100),
                        height: percent(100),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.35, 0.9, 0.45)),
                ));
                p.spawn((BombText, text("BOMBS 2", 15.0, Color::WHITE)));
                p.spawn((PowerText, text("POWER", 15.0, Color::WHITE)));
            });

            // Controls reminder (bottom-right).
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: px(16),
                    right: px(14),
                    ..default()
                },
                text(hint, 12.0, Color::srgba(1.0, 1.0, 1.0, 0.45)),
            ));

            // Centre banner.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: percent(34),
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(6),
                    ..default()
                },
                children![
                    (BannerTitle, text("", 52.0, Color::WHITE)),
                    (BannerSub, text("", 20.0, ACCENT)),
                ],
            ));
                });
        });
}

#[allow(clippy::too_many_arguments)]
fn update_hud(
    time: Res<Time<Real>>,
    run: Res<Run>,
    boss: Res<BossStatus>,
    banner: Res<Banner>,
    director: Res<Director>,
    player: Query<&Player>,
    mut texts: ParamSet<(
        Query<&mut Text, With<ScoreText>>,
        Query<&mut Text, With<StarText>>,
        Query<&mut Text, With<BombText>>,
        Query<&mut Text, With<PowerText>>,
        Query<&mut Text, With<BossName>>,
        Query<(&mut Text, &mut TextColor, &mut TextShadow), With<BannerTitle>>,
        Query<(&mut Text, &mut TextColor, &mut TextShadow), With<BannerSub>>,
    )>,
    mut nodes: ParamSet<(
        Query<(&mut Node, &mut BackgroundColor), With<HullFill>>,
        Query<&mut Node, With<BossFill>>,
        Query<&mut Node, With<BossPanel>>,
        Query<&mut Node, With<ProgressFill>>,
    )>,
) {
    if let Ok(mut t) = texts.p0().single_mut() {
        set_text(&mut t, run.score.to_string());
    }
    if let Ok(mut t) = texts.p1().single_mut() {
        set_text(&mut t, run.stars.to_string());
    }
    let player = player.single().ok();
    let (bombs, power, hull) = player
        .map(|p| (p.bombs, p.power, (p.hp / p.max_hp).clamp(0.0, 1.0)))
        .unwrap_or((0, 0, 0.0));
    if let Ok(mut t) = texts.p2().single_mut() {
        set_text(&mut t, format!("BOMBS {bombs}"));
    }
    if let Ok(mut t) = texts.p3().single_mut() {
        let pips: String = (0..MAX_POWER).map(|i| if i < power { '#' } else { '-' }).collect();
        set_text(&mut t, format!("POWER [{pips}]"));
    }
    if let Ok(mut t) = texts.p4().single_mut() {
        let name = if boss.shielded {
            format!("{}  -  DESTROY THE TURRETS", boss.name)
        } else {
            boss.name.to_string()
        };
        set_text(&mut t, name);
    }

    // Banner fades in and out; warnings blink red.
    let k = banner.timer;
    let alpha = (k * 2.5).min(1.0) * if k > 0.0 { 1.0 } else { 0.0 };
    let blink = if banner.warning {
        0.55 + 0.45 * (time.elapsed_secs() * 10.0).sin().abs()
    } else {
        1.0
    };
    // The shadow fades with the text so no dark ghost is left behind.
    if let Ok((mut t, mut c, mut shadow)) = texts.p5().single_mut() {
        set_text(&mut t, banner.title.clone());
        let base = if banner.warning { Color::srgb(1.0, 0.25, 0.2) } else { Color::WHITE };
        c.0 = base.with_alpha(alpha * blink);
        shadow.color.set_alpha(SHADOW_ALPHA * alpha * blink);
    }
    if let Ok((mut t, mut c, mut shadow)) = texts.p6().single_mut() {
        set_text(&mut t, banner.subtitle.clone());
        c.0 = ACCENT.with_alpha(alpha);
        shadow.color.set_alpha(SHADOW_ALPHA * alpha);
    }

    if let Ok((mut n, mut bg)) = nodes.p0().single_mut() {
        n.width = percent(hull * 100.0);
        bg.0 = if hull < 0.34 {
            Color::srgb(0.95, 0.3, 0.2)
        } else if hull < 0.67 {
            Color::srgb(0.95, 0.8, 0.25)
        } else {
            Color::srgb(0.35, 0.9, 0.45)
        };
    }
    if let Ok(mut n) = nodes.p1().single_mut() {
        n.width = percent(boss.hp_frac * 100.0);
    }
    if let Ok(mut n) = nodes.p2().single_mut() {
        let want = if boss.active { Display::Flex } else { Display::None };
        if n.display != want {
            n.display = want;
        }
    }
    if let Ok(mut n) = nodes.p3().single_mut() {
        let k = if director.boss_time > 0.0 {
            (director.t / director.boss_time).clamp(0.0, 1.0)
        } else {
            0.0
        };
        n.width = percent(k * 100.0);
    }
}

/// Only touch the text when it changes; every write triggers a re-layout.
fn set_text(t: &mut Mut<Text>, value: String) {
    if t.0 != value {
        t.0 = value;
    }
}

fn spawn_pause(mut commands: Commands, touch: Res<TouchInput>) {
    let mut root = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            // On touch screens the menu sits in the top half: touching the
            // bottom half resumes the game.
            justify_content: if touch.enabled { JustifyContent::FlexStart } else { JustifyContent::Center },
            padding: UiRect::top(if touch.enabled { percent(16) } else { px(0) }),
            row_gap: px(14),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.02, 0.06, 0.6)),
        GlobalZIndex(10),
        DespawnOnExit(PlayState::Paused),
    ));
    root.with_children(|p| {
        p.spawn(text("PAUSED", 48.0, Color::WHITE));
        p.spawn(Node {
            height: px(10),
            ..default()
        });
        if touch.enabled {
            p.spawn(button("Abort Mission", Action::Abort, 260.0));
            p.spawn(Node {
                height: px(40),
                ..default()
            });
            p.spawn(text("Touch and hold the lower half", 22.0, ACCENT));
            p.spawn(text("of the screen to keep flying", 22.0, ACCENT));
        } else {
            p.spawn(button("Resume", Action::Resume, 260.0));
            p.spawn(button("Abort Mission", Action::Abort, 260.0));
            p.spawn(text("Press ESC to resume", 14.0, Color::srgba(1.0, 1.0, 1.0, 0.5)));
        }
    });
}
