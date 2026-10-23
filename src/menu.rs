//! Title screen, mission select, hangar (upgrades) and results screens.

use crate::art::{player_set, Art, UPGRADE_ANCHORS};
use crate::audio::{PlaySfx, Sfx};
use crate::terrain::THEMES;
use crate::boss::BOSSES;
use crate::save::{Profile, Upgrade, STAGE_COUNT, UPGRADES, UPGRADE_COUNT};
use crate::state::{AppState, Outcome, PlayState, Run, MEDAL_NAMES};
use crate::touch::TouchInput;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::Pressed;
use bevy::ui_widgets::{Activate, Button};

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Start,
    Hangar,
    Menu,
    Launch(usize),
    /// Select an upgrade in the hangar.
    Select(usize),
    /// Buy the selected upgrade.
    Buy,
    ToggleMusic,
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
struct Title;

#[derive(Component)]
struct TitleLetter {
    index: usize,
    material: Handle<StandardMaterial>,
}

#[derive(Component)]
struct MusicLabel;

/// The jet on display in the hangar.
#[derive(Component)]
struct HangarJet;

#[derive(Component)]
struct UpgradeRow(usize);

#[derive(Component)]
struct CalloutLabel;

/// Pulsing glow marking the selected part on the hangar jet.
#[derive(Component)]
struct CalloutMarker;

/// Which upgrade is selected in the hangar.
#[derive(Resource, Default)]
struct HangarSelection(usize);

const TITLE_Y: f32 = 300.0;
const TITLE_Z: f32 = 600.0;
const JET_POS: Vec3 = Vec3::new(0.0, 175.0, 560.0);
const JET_SCALE: f32 = 3.1;

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
            .init_resource::<HangarSelection>()
            .add_systems(Startup, configure_gizmos)
            .add_systems(Update, (style_buttons, style_upgrade_rows, menu_keys, scale_ui, tap_buttons))
            .add_systems(Update, animate_title.run_if(in_state(AppState::Menu)))
            .add_systems(Update, (animate_hangar_jet, draw_callout).chain().run_if(in_state(AppState::Hangar)))
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
        (With<Action>, Without<UpgradeRow>),
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

/// Finger taps activate buttons straight from the touch position. A tap is a
/// brand-new pointer with no hover history, which UI picking can miss.
fn tap_buttons(
    mut commands: Commands,
    touches: Res<Touches>,
    windows: Query<&Window>,
    buttons: Query<(Entity, &ComputedNode, &UiGlobalTransform, &InheritedVisibility), With<Action>>,
) {
    let scale = windows.single().map(|w| w.scale_factor()).unwrap_or(1.0);
    for touch in touches.iter_just_released() {
        let (start, end) = (touch.start_position() * scale, touch.position() * scale);
        for (entity, node, transform, visible) in &buttons {
            if visible.get() && node.contains_point(*transform, start) && node.contains_point(*transform, end) {
                commands.trigger(Activate { entity });
            }
        }
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

fn spawn_main_menu(
    mut commands: Commands,
    art: Res<Art>,
    profile: Res<Profile>,
    touch: Res<TouchInput>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    spawn_title(&mut commands, &art, &mut materials);
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            DespawnOnExit(AppState::Menu),
        ))
        .with_children(|outer| {
            outer
                .spawn(Node {
                    width: px(600),
                    height: px(900),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|root| {
                    // The 3D title floats above this; the tagline sits under it.
                    root.spawn((
                        Node {
                            margin: UiRect::top(px(292)),
                            ..default()
                        },
                        text("ARCADE SKY COMBAT", 18.0, ACCENT),
                    ));
                    root.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: px(16),
                        margin: UiRect::top(px(110)),
                        ..default()
                    })
                    .with_children(|col| {
                        col.spawn(big_button("Missions", Action::Start));
                        col.spawn(big_button("Upgrades", Action::Hangar));
                    });
                    // Music toggle (sound effects stay on).
                    root.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            bottom: px(28),
                            ..default()
                        },
                        children![(
                            Button,
                            Hovered::default(),
                            Action::ToggleMusic,
                            Node {
                                width: px(170),
                                height: px(40),
                                border: UiRect::all(px(2)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border_radius: BorderRadius::all(px(20)),
                                ..default()
                            },
                            BorderColor::all(ACCENT.with_alpha(0.5)),
                            BackgroundColor(BTN),
                            children![(
                                MusicLabel,
                                Text::new(music_label(&profile)),
                                TextFont {
                                    font_size: FontSize::Px(15.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                Pickable::IGNORE,
                            )],
                        )],
                    ));
                    let hint = if touch.show_touch_hints() {
                        "Drag to fly - lift to pause - 2nd finger bombs"
                    } else {
                        "WASD / mouse to fly - Space bomb - Esc pause"
                    };
                    root.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            bottom: px(84),
                            ..default()
                        },
                        text(hint, 13.0, DIM),
                    ));
                });
        });
}

fn music_label(profile: &Profile) -> String {
    if profile.music_on { "MUSIC: ON".into() } else { "MUSIC: OFF".into() }
}

fn big_button(label: &str, action: Action) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        action,
        Node {
            width: px(300),
            height: px(64),
            border: UiRect::all(px(2)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(14)),
            ..default()
        },
        BorderColor::all(ACCENT.with_alpha(0.5)),
        BackgroundColor(BTN),
        BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.45), px(0), px(4), px(0), px(10)),
        children![(
            Text::new(label.to_uppercase()),
            TextFont {
                font_size: FontSize::Px(26.0),
                ..default()
            },
            TextColor(Color::WHITE),
            TextShadow {
                offset: Vec2::new(0.0, 2.0),
                color: Color::srgba(0.0, 0.0, 0.0, 0.5),
            },
            Pickable::IGNORE,
        )],
    )
}

/// The game's name in floating 3D gold letters, high above the sea.
fn spawn_title(commands: &mut Commands, art: &Art, materials: &mut Assets<StandardMaterial>) {
    let title = commands
        .spawn((
            Title,
            Transform::from_xyz(0.0, TITLE_Y, TITLE_Z).with_scale(Vec3::splat(1.3)),
            Visibility::default(),
            DespawnOnExit(AppState::Menu),
        ))
        .id();
    for (index, (mesh, x)) in art.title_letters.iter().enumerate() {
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.55, 0.27, 0.02),
            emissive: LinearRgba::rgb(0.06, 0.025, 0.0),
            metallic: 0.5,
            perceptual_roughness: 0.35,
            reflectance: 0.5,
            ..default()
        });
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(*x, 0.0, 0.0),
            TitleLetter { index, material },
            NotShadowCaster,
            ChildOf(title),
        ));
    }
}

/// Floats and tilts the title, with a glint sweeping across the letters.
fn animate_title(
    time: Res<Time<Real>>,
    mut titles: Query<&mut Transform, (With<Title>, Without<TitleLetter>)>,
    mut letters: Query<(&TitleLetter, &mut Transform), Without<Title>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let s = time.elapsed_secs();
    for mut t in &mut titles {
        t.translation.y = TITLE_Y + (s * 0.8).sin() * 9.0;
        t.rotation = Quat::from_rotation_x((s * 0.55).sin() * 0.16) * Quat::from_rotation_y((s * 0.4).sin() * 0.14);
    }
    let sweep = (s / 3.2).fract() * 1.8 - 0.3;
    for (letter, mut t) in &mut letters {
        let k = letter.index as f32 / 8.0;
        t.translation.y = ((s * 1.8) + letter.index as f32 * 0.55).sin() * 3.0;
        t.translation.z = ((s * 1.3) + letter.index as f32 * 0.7).sin() * 4.0;
        let glint = (-((sweep - k) * (sweep - k)) / 0.006).exp();
        let shimmer = 0.15 * ((s * 3.0) + letter.index as f32).sin().abs();
        if let Some(mut m) = materials.get_mut(&letter.material) {
            m.emissive = LinearRgba::rgb(0.06 + shimmer * 0.5 + 2.4 * glint, 0.025 + shimmer * 0.25 + 1.4 * glint, 0.3 * glint);
        }
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

fn spawn_hangar_screen(
    mut commands: Commands,
    art: Res<Art>,
    profile: Res<Profile>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut selection: ResMut<HangarSelection>,
) {
    selection.0 = selection.0.min(UPGRADE_COUNT - 1);
    spawn_hangar_jet(&mut commands, &art, &mut meshes, &profile);
    spawn_hangar(&mut commands, &art, &profile, selection.0);
}

/// The player's jet, showing its current upgrades, on a slow turntable.
fn spawn_hangar_jet(commands: &mut Commands, art: &Art, meshes: &mut Assets<Mesh>, profile: &Profile) {
    let set = player_set(meshes, profile.upgrades);
    let jet = commands
        .spawn((
            HangarJet,
            Mesh3d(set.paint.clone()),
            MeshMaterial3d(art.mat_paint.clone()),
            Transform::from_translation(JET_POS).with_scale(Vec3::splat(JET_SCALE)),
            DespawnOnExit(AppState::Hangar),
        ))
        .id();
    crate::common::attach_model_parts(commands, jet, &set, art);
    commands.spawn((
        CalloutMarker,
        Mesh3d(art.quad.clone()),
        MeshMaterial3d(art.mat_star_halo.clone()),
        Transform::from_translation(JET_POS + Vec3::Z * 60.0).with_scale(Vec3::new(60.0, 60.0, 1.0)),
        NotShadowCaster,
        NotShadowReceiver,
        DespawnOnExit(AppState::Hangar),
    ));
}

fn spawn_hangar(commands: &mut Commands, art: &Art, profile: &Profile, selected: usize) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            HangarRoot,
            DespawnOnExit(AppState::Hangar),
        ))
        .with_children(|outer| {
            outer
                .spawn(Node {
                    width: px(600),
                    height: px(900),
                    ..default()
                })
                .with_children(|root| {
                    root.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            top: px(20),
                            left: px(24),
                            ..default()
                        },
                        text("UPGRADES", 34.0, Color::WHITE),
                    ));
                    root.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            top: px(24),
                            right: px(24),
                            ..default()
                        },
                        children![star_count(art, profile.stars, 26.0)],
                    ));
                    // Label at the end of the callout line, placed each frame.
                    root.spawn((
                        CalloutLabel,
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(-400),
                            ..default()
                        },
                        text("", 15.0, GOLD),
                    ));

                    // Upgrade list.
                    root.spawn(Node {
                        position_type: PositionType::Absolute,
                        top: px(426),
                        left: px(30),
                        width: px(540),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(6),
                        ..default()
                    })
                    .with_children(|list| {
                        for i in 0..UPGRADE_COUNT {
                            let u = upgrade_from_index(i);
                            let level = profile.level(u);
                            let cost = profile.next_cost(u);
                            list.spawn((
                                Button,
                                Hovered::default(),
                                Action::Select(i),
                                UpgradeRow(i),
                                Node {
                                    height: px(50),
                                    padding: UiRect::axes(px(14), px(6)),
                                    border: UiRect::all(px(2)),
                                    border_radius: BorderRadius::all(px(12)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    ..default()
                                },
                                BorderColor::all(ACCENT.with_alpha(0.2)),
                                BackgroundColor(PANEL),
                            ))
                            .with_children(|row| {
                                row.spawn((
                                    Node {
                                        flex_direction: FlexDirection::Column,
                                        row_gap: px(4),
                                        ..default()
                                    },
                                    Pickable::IGNORE,
                                ))
                                .with_children(|col| {
                                    col.spawn((text(UPGRADES[i].name, 18.0, Color::WHITE), Pickable::IGNORE));
                                    col.spawn((
                                        Node {
                                            column_gap: px(4),
                                            ..default()
                                        },
                                        Pickable::IGNORE,
                                    ))
                                    .with_children(|pips| {
                                        for k in 0..u.max_level() {
                                            pips.spawn((
                                                Node {
                                                    width: px(20),
                                                    height: px(6),
                                                    border_radius: BorderRadius::all(px(3)),
                                                    ..default()
                                                },
                                                BackgroundColor(if k < level { GOLD } else { Color::srgba(1.0, 1.0, 1.0, 0.15) }),
                                                Pickable::IGNORE,
                                            ));
                                        }
                                    });
                                });
                                match cost {
                                    Some(c) => {
                                        let color = if profile.stars >= c { GOLD } else { DIM };
                                        row.spawn((
                                            Node {
                                                align_items: AlignItems::Center,
                                                column_gap: px(5),
                                                ..default()
                                            },
                                            Pickable::IGNORE,
                                            children![
                                                (
                                                    ImageNode::new(art.tex_star_icon.clone()),
                                                    Node {
                                                        width: px(18),
                                                        height: px(18),
                                                        ..default()
                                                    },
                                                    Pickable::IGNORE,
                                                ),
                                                (text(c.to_string(), 17.0, color), Pickable::IGNORE),
                                            ],
                                        ));
                                    }
                                    None => {
                                        row.spawn((text("MAX", 17.0, ACCENT), Pickable::IGNORE));
                                    }
                                }
                            });
                        }
                    });

                    // Details of the selected upgrade and the buy button.
                    let u = upgrade_from_index(selected);
                    let cost = profile.next_cost(u);
                    let affordable = cost.is_some_and(|c| profile.stars >= c);
                    root.spawn(Node {
                        position_type: PositionType::Absolute,
                        top: px(718),
                        left: px(30),
                        width: px(540),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: px(10),
                        ..default()
                    })
                    .with_children(|d| {
                        d.spawn(text(UPGRADES[selected].blurb, 15.0, DIM));
                        d.spawn(Node {
                            column_gap: px(12),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn(button("Back", Action::Menu, 150.0));
                            let label = match cost {
                                Some(c) => format!("Upgrade  {c}"),
                                None => "Maxed out".to_string(),
                            };
                            let mut b = row.spawn(button(&label, Action::Buy, 220.0));
                            if !affordable {
                                b.insert(Disabled);
                            }
                            row.spawn(button("Missions", Action::Start, 150.0));
                        });
                    });
                });
        });
}

/// Rows highlight on hover; hovering (or tapping) a row selects it.
fn style_upgrade_rows(
    mut commands: Commands,
    art: Option<Res<Art>>,
    profile: Res<Profile>,
    mut selection: ResMut<HangarSelection>,
    roots: Query<Entity, With<HangarRoot>>,
    mut rows: Query<(&UpgradeRow, &Hovered, &mut BackgroundColor, &mut BorderColor)>,
) {
    let mut changed = None;
    for (row, hovered, mut bg, mut border) in &mut rows {
        if hovered.get() && selection.0 != row.0 {
            changed = Some(row.0);
        }
        let selected = selection.0 == row.0;
        let want = if selected {
            Color::srgba(0.12, 0.22, 0.34, 0.92)
        } else if hovered.get() {
            Color::srgba(0.08, 0.16, 0.26, 0.9)
        } else {
            PANEL
        };
        if bg.0 != want {
            bg.0 = want;
        }
        border.set_all(if selected { GOLD } else { ACCENT.with_alpha(0.2) });
    }
    if let (Some(i), Some(art)) = (changed, art) {
        selection.0 = i;
        for e in &roots {
            commands.entity(e).despawn();
        }
        spawn_hangar(&mut commands, &art, &profile, i);
    }
}

fn animate_hangar_jet(time: Res<Time<Real>>, mut q: Query<&mut Transform, With<HangarJet>>) {
    let s = time.elapsed_secs();
    for mut t in &mut q {
        t.translation = JET_POS + Vec3::new(0.0, (s * 0.9).sin() * 6.0, 0.0);
        t.rotation = Quat::from_rotation_z((s * 0.35).sin() * 0.18) * Quat::from_rotation_y((s * 0.6).sin() * 0.32);
    }
}

/// Draws a line from the selected part of the jet out to its label.
fn draw_callout(
    mut gizmos: Gizmos,
    time: Res<Time<Real>>,
    selection: Res<HangarSelection>,
    profile: Res<Profile>,
    ui_scale: Res<UiScale>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::common::MainCamera>>,
    jets: Query<&Transform, (With<HangarJet>, Without<CalloutMarker>)>,
    mut markers: Query<&mut Transform, (With<CalloutMarker>, Without<HangarJet>)>,
    mut labels: Query<(&mut Node, &mut Text), With<CalloutLabel>>,
) {
    let (Ok(jet), Ok((cam, cam_t))) = (jets.single(), camera.single()) else { return };
    let i = selection.0.min(UPGRADE_COUNT - 1);
    let local = UPGRADE_ANCHORS[i];
    let part = jet.transform_point(local) + Vec3::Z * 40.0;
    let right = local.x > 1.0;
    let side = if right { 1.0 } else { -1.0 };
    let elbow = Vec3::new(part.x + side * 60.0, part.y + 46.0, part.z);
    let end = Vec3::new(side * 300.0, elbow.y, part.z);
    let gold = Color::srgb(1.0, 0.82, 0.35);
    gizmos.line(part, elbow, gold);
    gizmos.line(elbow, end, gold);
    let pulse = 9.0 + (time.elapsed_secs() * 5.0).sin() * 2.0;
    gizmos.circle(Isometry3d::from_translation(part), pulse, gold);
    if let Ok(mut m) = markers.single_mut() {
        m.translation = part - Vec3::Z * 2.0;
        let k = 44.0 + pulse * 2.0;
        m.scale = Vec3::new(k, k, 1.0);
    }
    if let Ok((mut node, mut label)) = labels.single_mut() {
        let u = upgrade_from_index(i);
        let value = format!("{}  {}/{}", UPGRADES[i].name.to_uppercase(), profile.level(u), u.max_level());
        if label.0 != value {
            label.0 = value;
        }
        if let Ok(vp) = cam.world_to_viewport(cam_t, end + Vec3::new(-side * 4.0, 6.0, 0.0)) {
            // Convert to the 600x900 frame (centred in the window).
            let window = cam.logical_viewport_size().unwrap_or(Vec2::new(600.0, 900.0)) / ui_scale.0;
            let origin = (window - Vec2::new(600.0, 900.0)) / 2.0;
            let p = vp / ui_scale.0 - origin;
            node.top = px(p.y - 22.0);
            if right {
                node.left = Val::Auto;
                node.right = px(600.0 - p.x);
            } else {
                node.right = Val::Auto;
                node.left = px(p.x);
            }
        }
    }
}

fn configure_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
    config.line.width = 3.0;
    config.depth_bias = -1.0;
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
    jets: Query<Entity, Or<(With<HangarJet>, With<CalloutMarker>)>>,
    mut music_labels: Query<&mut Text, With<MusicLabel>>,
    mut selection: ResMut<HangarSelection>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut sfx: MessageWriter<PlaySfx>,
    time: Res<Time<Real>>,
    mut last: Local<Option<(Entity, f64)>>,
) {
    let Ok((&action, disabled)) = actions.get(ev.entity) else {
        return;
    };
    // A tap can be reported by both picking and `tap_buttons`; act once.
    let now = time.elapsed_secs_f64();
    if last.is_some_and(|(e, t)| e == ev.entity && now - t < 0.35) {
        return;
    }
    *last = Some((ev.entity, now));
    if disabled {
        sfx.write(PlaySfx::new(Sfx::Denied));
        return;
    }
    sfx.write(PlaySfx::new(Sfx::Click));
    match action {
        Action::Start => next.set(AppState::StageSelect),
        Action::Hangar => next.set(AppState::Hangar),
        Action::Menu => next.set(AppState::Menu),
        Action::Launch(i) => launch(&mut run, &mut next, &profile, i),
        Action::Retry => {
            let stage = run.stage;
            launch(&mut run, &mut next, &profile, stage);
        }
        Action::Select(i) => {
            if selection.0 != i {
                selection.0 = i;
                for e in &hangar {
                    commands.entity(e).despawn();
                }
                spawn_hangar(&mut commands, &art, &profile, i);
            }
        }
        Action::Buy => {
            if profile.try_buy(upgrade_from_index(selection.0)) {
                profile.save();
                sfx.write(PlaySfx::new(Sfx::PowerUp));
                for e in hangar.iter().chain(jets.iter()) {
                    commands.entity(e).despawn();
                }
                spawn_hangar_jet(&mut commands, &art, &mut meshes, &profile);
                spawn_hangar(&mut commands, &art, &profile, selection.0);
            }
        }
        Action::ToggleMusic => {
            profile.music_on = !profile.music_on;
            profile.save();
            let label = music_label(&profile);
            for mut t in &mut music_labels {
                t.0 = label.clone();
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
