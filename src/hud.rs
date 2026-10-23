//! In-stage HUD: score, stars, segmented hull bar, bombs, power, shield,
//! boss health, stage progress, enemy health bars, banners and the pause
//! screen.

use crate::art::Art;
use crate::boss::BossStatus;
use crate::common::{Health, MainCamera};
use crate::enemy::Enemy;
use crate::level::{Banner, Director};
use crate::menu::{button, text, Action, SHADOW_ALPHA};
use crate::player::{Player, MAX_POWER};
use crate::state::{AppState, PauseFlow, PlayState, Run};
use crate::touch::TouchInput;
use bevy::prelude::*;

#[derive(Component)]
struct ScoreText;
#[derive(Component)]
struct StarText;
#[derive(Component)]
struct HullBar;
#[derive(Component)]
struct HullSegment(usize);
#[derive(Component)]
struct HullPercent;
#[derive(Component)]
struct ShieldRow;
#[derive(Component)]
struct ShieldFill;
#[derive(Component)]
struct BombIcon(u8);
#[derive(Component)]
struct PowerPip(u8);
#[derive(Component)]
struct PowerLabel;
#[derive(Component)]
struct BossPanel;
#[derive(Component)]
struct BossName;
#[derive(Component)]
struct BossFill;
#[derive(Component)]
struct BossShieldTag;
#[derive(Component)]
struct ProgressFill;
#[derive(Component)]
struct BannerTitle;
#[derive(Component)]
struct BannerSub;
#[derive(Component)]
struct PauseMask;
#[derive(Component)]
struct LowHullVignette;
/// Small health bar floating over a damaged enemy.
#[derive(Component)]
struct EnemyBar {
    target: Entity,
    width: f32,
}
#[derive(Component)]
struct EnemyBarFill;

const PANEL: Color = Color::srgba(0.02, 0.05, 0.1, 0.62);
const EDGE: Color = Color::srgba(0.45, 0.85, 1.0, 0.25);
const ACCENT: Color = Color::srgb(0.45, 0.85, 1.0);
const GOLD: Color = Color::srgb(1.0, 0.85, 0.35);
const HULL_W: f32 = 196.0;
const MAX_SEGMENTS: usize = 16;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Playing), spawn_hud)
            .add_systems(OnEnter(PlayState::Paused), spawn_pause)
            .add_systems(
                Update,
                (update_hud, update_hull, update_overlays, update_enemy_bars).run_if(in_state(AppState::Playing)),
            );
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

fn panel(node: Node) -> impl Bundle {
    (
        Node {
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(10)),
            ..node
        },
        BackgroundColor(PANEL),
        BorderColor::all(EDGE),
    )
}

fn spawn_hud(mut commands: Commands, art: Res<Art>, touch: Res<TouchInput>, profile: Res<crate::save::Profile>) {
    let hint = if touch.show_touch_hints() {
        "Lift finger to pause   2nd finger: bomb"
    } else {
        "SPACE bomb  ESC pause  M mute"
    };
    let hull = profile.level(crate::save::Upgrade::Hull) as usize;
    let max_hp = (6 + 2 * hull).clamp(1, MAX_SEGMENTS);

    // Full-screen layers: low-hull vignette and the pause mask.
    commands.spawn((
        LowHullVignette,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        vignette(0.0),
        Pickable::IGNORE,
        DespawnOnExit(AppState::Playing),
    ));
    commands.spawn((
        PauseMask,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.02, 0.06, 0.0)),
        GlobalZIndex(8),
        Pickable::IGNORE,
        DespawnOnExit(AppState::Playing),
    ));

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
                    // Stage progress along the very top, ending at the boss.
                    root.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            top: px(0),
                            left: px(0),
                            width: percent(100),
                            height: px(5),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.4)),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            ProgressFill,
                            Node {
                                width: percent(0),
                                height: percent(100),
                                ..default()
                            },
                            BackgroundGradient::from(LinearGradient {
                                angle: LinearGradient::TO_RIGHT,
                                stops: vec![ACCENT.with_alpha(0.3).into(), ACCENT.into(), Color::WHITE.into()],
                                ..default()
                            }),
                        ));
                        bar.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                right: px(-2),
                                top: px(-3),
                                width: px(10),
                                height: px(10),
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(1.0, 0.3, 0.25)),
                        ));
                    });

                    // Score (top-left).
                    root.spawn(panel(Node {
                        position_type: PositionType::Absolute,
                        top: px(14),
                        left: px(14),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::axes(px(12), px(6)),
                        ..default()
                    }))
                    .with_children(|p| {
                        p.spawn(text("SCORE", 11.0, ACCENT));
                        p.spawn((ScoreText, text("0", 26.0, Color::WHITE)));
                    });

                    // Stars (top-right).
                    root.spawn(panel(Node {
                        position_type: PositionType::Absolute,
                        top: px(14),
                        right: px(14),
                        align_items: AlignItems::Center,
                        column_gap: px(6),
                        padding: UiRect::axes(px(12), px(8)),
                        border_radius: BorderRadius::all(px(18)),
                        ..default()
                    }))
                    .with_children(|p| {
                        p.spawn((
                            ImageNode::new(art.tex_star_icon.clone()),
                            Node {
                                width: px(26),
                                height: px(26),
                                ..default()
                            },
                        ));
                        p.spawn((StarText, text("0", 24.0, GOLD)));
                    });

                    // Boss health (top-centre).
                    root.spawn((
                        BossPanel,
                        Node {
                            position_type: PositionType::Absolute,
                            top: px(80),
                            left: percent(10),
                            width: percent(80),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: px(5),
                            display: Display::None,
                            ..default()
                        },
                    ))
                    .with_children(|p| {
                        p.spawn(Node {
                            column_gap: px(10),
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn((BossName, text("", 16.0, Color::srgb(1.0, 0.6, 0.5))));
                            row.spawn((
                                BossShieldTag,
                                Node {
                                    padding: UiRect::axes(px(6), px(1)),
                                    border_radius: BorderRadius::all(px(4)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.3, 0.7, 1.0, 0.25)),
                                children![text("SHIELDED", 11.0, ACCENT)],
                            ));
                        });
                        p.spawn((
                            Node {
                                width: percent(100),
                                height: px(14),
                                border: UiRect::all(px(2)),
                                border_radius: BorderRadius::all(px(7)),
                                padding: UiRect::all(px(1)),
                                ..default()
                            },
                            BorderColor::all(Color::srgba(1.0, 0.75, 0.6, 0.7)),
                            BackgroundColor(Color::srgba(0.1, 0.0, 0.0, 0.6)),
                        ))
                        .with_child((
                            BossFill,
                            Node {
                                width: percent(100),
                                height: percent(100),
                                border_radius: BorderRadius::all(px(5)),
                                ..default()
                            },
                            BackgroundGradient::from(LinearGradient {
                                angle: LinearGradient::TO_RIGHT,
                                stops: vec![
                                    Color::srgb(0.75, 0.08, 0.06).into(),
                                    Color::srgb(1.0, 0.4, 0.15).into(),
                                    Color::srgb(1.0, 0.75, 0.3).into(),
                                ],
                                ..default()
                            }),
                        ));
                    });

                    // Status panel (bottom-left): hull, shield, bombs, power.
                    root.spawn(panel(Node {
                        position_type: PositionType::Absolute,
                        bottom: px(14),
                        left: px(14),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(6),
                        padding: UiRect::all(px(10)),
                        ..default()
                    }))
                    .with_children(|p| {
                        p.spawn(Node {
                            width: px(HULL_W),
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn(text("HULL", 11.0, ACCENT));
                            row.spawn((HullPercent, text("100%", 11.0, Color::WHITE)));
                        });
                        p.spawn((
                            HullBar,
                            Node {
                                width: px(HULL_W),
                                height: px(13),
                                column_gap: px(2),
                                ..default()
                            },
                        ))
                        .with_children(|bar| {
                            for i in 0..max_hp {
                                bar.spawn((
                                    HullSegment(i),
                                    Node {
                                        flex_grow: 1.0,
                                        height: percent(100),
                                        border_radius: BorderRadius::all(px(3)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.3, 0.95, 0.45)),
                                ));
                            }
                        });
                        p.spawn((
                            ShieldRow,
                            Node {
                                width: px(HULL_W),
                                height: px(5),
                                display: Display::None,
                                border_radius: BorderRadius::all(px(3)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.3, 0.7, 1.0, 0.2)),
                        ))
                        .with_child((
                            ShieldFill,
                            Node {
                                width: percent(100),
                                height: percent(100),
                                border_radius: BorderRadius::all(px(3)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.45, 0.85, 1.0)),
                        ));
                        p.spawn(Node {
                            align_items: AlignItems::Center,
                            column_gap: px(5),
                            margin: UiRect::top(px(2)),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn((
                                text("BOMB", 11.0, ACCENT),
                                Node {
                                    width: px(48),
                                    ..default()
                                },
                            ));
                            for i in 0..5 {
                                row.spawn((
                                    BombIcon(i),
                                    Node {
                                        width: px(14),
                                        height: px(14),
                                        border: UiRect::all(px(2)),
                                        border_radius: BorderRadius::MAX,
                                        ..default()
                                    },
                                    BorderColor::all(Color::srgb(1.0, 0.55, 0.4)),
                                    BackgroundColor(Color::srgb(0.9, 0.25, 0.15)),
                                ));
                            }
                        });
                        p.spawn(Node {
                            align_items: AlignItems::Center,
                            column_gap: px(7),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn((
                                text("POWER", 11.0, ACCENT),
                                Node {
                                    width: px(46),
                                    ..default()
                                },
                            ));
                            for i in 0..MAX_POWER {
                                row.spawn((
                                    PowerPip(i),
                                    Node {
                                        width: px(11),
                                        height: px(11),
                                        border: UiRect::all(px(2)),
                                        ..default()
                                    },
                                    UiTransform::from_rotation(Rot2::degrees(45.0)),
                                    BorderColor::all(Color::srgb(1.0, 0.7, 0.3)),
                                    BackgroundColor(Color::NONE),
                                ));
                            }
                            row.spawn((PowerLabel, text("", 11.0, GOLD)));
                        });
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

fn vignette(alpha: f32) -> BackgroundGradient {
    BackgroundGradient::from(RadialGradient::new(
        UiPosition::CENTER,
        RadialGradientShape::FarthestCorner,
        vec![
            ColorStop::percent(Color::srgba(0.9, 0.0, 0.0, 0.0), 55),
            ColorStop::percent(Color::srgba(0.9, 0.05, 0.02, alpha), 100),
        ],
    ))
}

/// Only touch the text when it changes; every write triggers a re-layout.
fn set_text(t: &mut Mut<Text>, value: String) {
    if t.0 != value {
        t.0 = value;
    }
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
        Query<&mut Text, With<BossName>>,
        Query<(&mut Text, &mut TextColor, &mut TextShadow), With<BannerTitle>>,
        Query<(&mut Text, &mut TextColor, &mut TextShadow), With<BannerSub>>,
        Query<&mut Text, With<PowerLabel>>,
    )>,
    mut nodes: ParamSet<(
        Query<&mut Node, With<BossFill>>,
        Query<&mut Node, With<BossPanel>>,
        Query<&mut Node, With<ProgressFill>>,
        Query<&mut Node, With<BossShieldTag>>,
        Query<(&BombIcon, &mut BackgroundColor, &mut BorderColor)>,
        Query<(&PowerPip, &mut BackgroundColor)>,
    )>,
) {
    if let Ok(mut t) = texts.p0().single_mut() {
        set_text(&mut t, run.score.to_string());
    }
    if let Ok(mut t) = texts.p1().single_mut() {
        set_text(&mut t, run.stars.to_string());
    }
    if let Ok(mut t) = texts.p2().single_mut() {
        set_text(&mut t, boss.name.to_string());
    }
    let player = player.single().ok();
    let (bombs, power) = player.map(|p| (p.bombs, p.power)).unwrap_or((0, 0));
    if let Ok(mut t) = texts.p5().single_mut() {
        set_text(&mut t, if power >= MAX_POWER { "MAX".into() } else { String::new() });
    }

    // Banner fades in and out; warnings blink red. The shadow fades with it.
    let k = banner.timer;
    let alpha = (k * 2.5).min(1.0) * if k > 0.0 { 1.0 } else { 0.0 };
    let blink = if banner.warning {
        0.55 + 0.45 * (time.elapsed_secs() * 10.0).sin().abs()
    } else {
        1.0
    };
    if let Ok((mut t, mut c, mut shadow)) = texts.p3().single_mut() {
        set_text(&mut t, banner.title.clone());
        let base = if banner.warning { Color::srgb(1.0, 0.25, 0.2) } else { Color::WHITE };
        c.0 = base.with_alpha(alpha * blink);
        shadow.color.set_alpha(SHADOW_ALPHA * alpha * blink);
    }
    if let Ok((mut t, mut c, mut shadow)) = texts.p4().single_mut() {
        set_text(&mut t, banner.subtitle.clone());
        c.0 = ACCENT.with_alpha(alpha);
        shadow.color.set_alpha(SHADOW_ALPHA * alpha);
    }

    if let Ok(mut n) = nodes.p0().single_mut() {
        n.width = percent(boss.hp_frac * 100.0);
    }
    if let Ok(mut n) = nodes.p1().single_mut() {
        let want = if boss.active { Display::Flex } else { Display::None };
        if n.display != want {
            n.display = want;
        }
    }
    if let Ok(mut n) = nodes.p2().single_mut() {
        let k = if director.boss_time > 0.0 {
            (director.t / director.boss_time).clamp(0.0, 1.0)
        } else {
            0.0
        };
        n.width = percent(k * 100.0);
    }
    if let Ok(mut n) = nodes.p3().single_mut() {
        let want = if boss.shielded { Display::Flex } else { Display::None };
        if n.display != want {
            n.display = want;
        }
    }
    for (icon, mut bg, mut border) in &mut nodes.p4() {
        let full = icon.0 < bombs;
        let fill = if full { Color::srgb(0.95, 0.3, 0.15) } else { Color::NONE };
        if bg.0 != fill {
            bg.0 = fill;
        }
        border.set_all(if full { Color::srgb(1.0, 0.7, 0.5) } else { Color::srgba(1.0, 1.0, 1.0, 0.25) });
    }
    for (pip, mut bg) in &mut nodes.p5() {
        let fill = if pip.0 < power { Color::srgb(1.0, 0.62, 0.15) } else { Color::NONE };
        if bg.0 != fill {
            bg.0 = fill;
        }
    }
}

/// Segmented hull bar: green -> amber -> red, flashing when critical.
fn update_hull(
    time: Res<Time<Real>>,
    player: Query<&Player>,
    mut segments: Query<(&HullSegment, &mut BackgroundColor)>,
    mut percent_text: Query<&mut Text, With<HullPercent>>,
    mut shield_row: Query<&mut Node, (With<ShieldRow>, Without<ShieldFill>)>,
    mut shield_fill: Query<&mut Node, (With<ShieldFill>, Without<ShieldRow>)>,
) {
    let (hp, max, shield) = player.single().map(|p| (p.hp.max(0.0), p.max_hp, p.shield)).unwrap_or((0.0, 1.0, 0.0));
    let frac = (hp / max).clamp(0.0, 1.0);
    let critical = frac <= 0.34;
    let flash = critical && (time.elapsed_secs() * 6.0).sin() > 0.0;
    let on = if frac > 0.66 {
        Color::srgb(0.3, 0.95, 0.45)
    } else if frac > 0.34 {
        Color::srgb(1.0, 0.78, 0.2)
    } else if flash {
        Color::srgb(1.0, 0.45, 0.35)
    } else {
        Color::srgb(0.95, 0.15, 0.1)
    };
    let off = Color::srgba(1.0, 1.0, 1.0, 0.1);
    for (seg, mut bg) in &mut segments {
        let want = if (seg.0 as f32) < hp.ceil() { on } else { off };
        if bg.0 != want {
            bg.0 = want;
        }
    }
    if let Ok(mut t) = percent_text.single_mut() {
        set_text(&mut t, format!("{}%", (frac * 100.0).round() as u32));
    }
    if let Ok(mut row) = shield_row.single_mut() {
        let want = if shield > 0.0 { Display::Flex } else { Display::None };
        if row.display != want {
            row.display = want;
        }
    }
    if let Ok(mut fill) = shield_fill.single_mut() {
        fill.width = percent((shield / 9.0).clamp(0.0, 1.0) * 100.0);
    }
}

/// Pause mask (follows the slow-motion pause) and the low-hull vignette.
fn update_overlays(
    time: Res<Time<Real>>,
    flow: Res<PauseFlow>,
    player: Query<&Player>,
    mut mask: Query<&mut BackgroundColor, With<PauseMask>>,
    mut vignette_q: Query<&mut BackgroundGradient, With<LowHullVignette>>,
    mut last_alpha: Local<f32>,
) {
    if let Ok(mut bg) = mask.single_mut() {
        bg.0 = Color::srgba(0.0, 0.02, 0.06, flow.mask * 0.62);
    }
    let frac = player.single().map(|p| (p.hp / p.max_hp).clamp(0.0, 1.0)).unwrap_or(1.0);
    let alpha = if frac <= 0.34 && frac > 0.0 {
        0.28 + 0.22 * (time.elapsed_secs() * 4.0).sin().abs()
    } else {
        0.0
    };
    if (alpha - *last_alpha).abs() > 0.01 {
        *last_alpha = alpha;
        if let Ok(mut g) = vignette_q.single_mut() {
            *g = vignette(alpha);
        }
    }
}

/// Health bars over damaged large enemies, positioned in screen space.
fn update_enemy_bars(
    mut commands: Commands,
    ui_scale: Res<UiScale>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    enemies: Query<(Entity, &Enemy, &Health, &Transform)>,
    mut bars: Query<(Entity, &EnemyBar, &mut Node, &Children)>,
    mut fills: Query<&mut Node, (With<EnemyBarFill>, Without<EnemyBar>)>,
    mut tracked: Local<Vec<Entity>>,
) {
    let Ok((cam, cam_t)) = camera.single() else { return };
    // Start bars for newly damaged enemies.
    for (e, enemy, health, _) in &enemies {
        let big = health.max >= 14.0 && enemy.kind != crate::enemy::EnemyKind::Boss;
        if big && health.hp < health.max && health.hp > 0.0 && !tracked.contains(&e) {
            tracked.push(e);
            let width = (enemy.size * 1.1).clamp(34.0, 90.0);
            commands.spawn((
                EnemyBar { target: e, width },
                Node {
                    position_type: PositionType::Absolute,
                    width: px(width),
                    height: px(6),
                    padding: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    left: px(-500),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                Pickable::IGNORE,
                DespawnOnExit(AppState::Playing),
                children![(
                    EnemyBarFill,
                    Node {
                        width: percent(100),
                        height: percent(100),
                        border_radius: BorderRadius::all(px(2)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(1.0, 0.35, 0.25)),
                )],
            ));
        }
    }
    for (bar_e, bar, mut node, children) in &mut bars {
        let Ok((_, enemy, health, t)) = enemies.get(bar.target) else {
            commands.entity(bar_e).try_despawn();
            tracked.retain(|e| *e != bar.target);
            continue;
        };
        if health.hp <= 0.0 {
            commands.entity(bar_e).try_despawn();
            tracked.retain(|e| *e != bar.target);
            continue;
        }
        let above = t.translation + Vec3::new(0.0, enemy.size + 10.0, 30.0);
        if let Ok(vp) = cam.world_to_viewport(cam_t, above) {
            node.left = px(vp.x / ui_scale.0 - bar.width / 2.0);
            node.top = px(vp.y / ui_scale.0);
        }
        for child in children.iter() {
            if let Ok(mut fill) = fills.get_mut(child) {
                fill.width = percent(health.frac() * 100.0);
            }
        }
    }
}

fn spawn_pause(mut commands: Commands, touch: Res<TouchInput>) {
    // The dark backdrop is the pause mask, which fades in with the slowdown.
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
