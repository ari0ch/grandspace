//! The real map (decision: the influence map is the hub). Stars carry a
//! ring in the colour of whoever leads that system; the official empires
//! are deliberately absent from this view.

use bevy::color::Alpha;
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use grandspace_protocol::{WireControl, WireSystem};

use crate::palette;
use crate::state::Game;
use crate::ui;

/// Map units are the galaxy's 0..1000 x 0..700, centred on the origin.
pub fn world_pos(s: &WireSystem) -> Vec2 {
    Vec2::new(s.x - 500.0, 350.0 - s.y)
}

#[derive(Resource)]
pub struct CameraZoom(pub f32);

#[derive(Component)]
pub struct SystemLabel;

pub fn spawn_camera(mut commands: Commands) {
    // Nudge right so the map sits in the space left of the sidebar.
    commands.spawn((
        Camera2d,
        Transform::from_xyz(ui::SIDEBAR_W * 0.5, -10.0, 0.0),
    ));
}

pub fn spawn_labels(
    mut commands: Commands,
    game: Res<Game>,
    existing: Query<(), With<SystemLabel>>,
) {
    let Some(galaxy) = &game.galaxy else { return };
    if !existing.is_empty() {
        return;
    }
    for s in &galaxy.systems {
        let p = world_pos(s);
        commands.spawn((
            SystemLabel,
            Text2d::new(s.name.clone()),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(palette::TEXT_DIM),
            Transform::from_xyz(p.x, p.y - 20.0, 1.0),
        ));
    }
}

/// The colour a system's ring should take, if any.
fn system_ring(game: &Game, system: &WireSystem) -> Option<Color> {
    let view = game.view.as_ref()?;
    let leader = view
        .system_leaders
        .get(system.id as usize)
        .copied()
        .flatten();
    let pick = |f: u32| {
        game.faction_power(f)
            .map(|p| palette::faction(p, f))
            .unwrap_or(palette::CONTESTED)
    };
    if let Some(f) = leader {
        return Some(pick(f));
    }
    // No overall leader: show whoever controls the most planets here.
    let mut counts: Vec<(u32, usize)> = Vec::new();
    let mut contested = false;
    for &pid in &system.planets {
        match view.planets.get(pid as usize).map(|p| p.control) {
            Some(WireControl::Controlled(f)) => match counts.iter_mut().find(|c| c.0 == f) {
                Some(c) => c.1 += 1,
                None => counts.push((f, 1)),
            },
            Some(WireControl::Contested) => contested = true,
            _ => {}
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    match counts.first() {
        Some(&(f, _)) => Some(pick(f).with_alpha(0.55)),
        None if contested => Some(palette::CONTESTED),
        None => None,
    }
}

pub fn draw_map(mut gizmos: Gizmos, game: Res<Game>) {
    let Some(galaxy) = &game.galaxy else { return };
    for s in &galaxy.systems {
        let a = world_pos(s);
        for &l in &s.lanes {
            if l > s.id {
                let b = world_pos(&galaxy.systems[l as usize]);
                gizmos.line_2d(a, b, palette::LANE);
            }
        }
    }
    let me = game.me;
    let my_colour = game
        .my_power()
        .zip(me)
        .map(|(p, f)| palette::faction(p, f))
        .unwrap_or(palette::SELECTED);
    for s in &galaxy.systems {
        let p = world_pos(s);
        // The star itself.
        for r in [1.0, 2.0, 3.0] {
            gizmos.circle_2d(p, r, palette::STAR);
        }
        if let Some(c) = system_ring(&game, s) {
            for r in [8.0, 8.8, 9.6] {
                gizmos.circle_2d(p, r, c);
            }
        }
        if let (Some(view), Some(me)) = (&game.view, me) {
            let present = s.planets.iter().any(|&pid| {
                view.planets
                    .get(pid as usize)
                    .is_some_and(|pv| pv.buildings.iter().any(|b| b.owner == me))
            });
            if present {
                // A small tick under the star: you have holdings here.
                gizmos.line_2d(
                    p + Vec2::new(-5.0, -13.0),
                    p + Vec2::new(5.0, -13.0),
                    my_colour,
                );
            }
            if view.relay_systems.contains(&s.id) {
                // A diamond: your ansible relay.
                let d = 4.0;
                let c = p + Vec2::new(13.0, 9.0);
                gizmos.line_2d(c + Vec2::Y * d, c + Vec2::X * d, my_colour);
                gizmos.line_2d(c + Vec2::X * d, c - Vec2::Y * d, my_colour);
                gizmos.line_2d(c - Vec2::Y * d, c - Vec2::X * d, my_colour);
                gizmos.line_2d(c - Vec2::X * d, c + Vec2::Y * d, my_colour);
            }
        }
        if game.selected_system == Some(s.id) {
            gizmos.circle_2d(p, 15.0, palette::SELECTED);
        }
    }
}

pub fn camera_controls(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    mut motion: MessageReader<MouseMotion>,
    time: Res<Time>,
    mut zoom: ResMut<CameraZoom>,
    window: Single<&Window>,
    camera: Single<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    let (mut transform, mut projection) = camera.into_inner();
    let over_ui = window
        .cursor_position()
        .is_some_and(|c| ui::over_ui(c, &window));

    let mut pan = Vec2::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        pan.y += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        pan.y -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        pan.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        pan.x += 1.0;
    }
    let speed = 500.0 * zoom.0 * time.delta_secs();
    transform.translation += (pan * speed).extend(0.0);

    // Drag with the right or middle mouse button.
    let dragging = buttons.pressed(MouseButton::Right) || buttons.pressed(MouseButton::Middle);
    for m in motion.read() {
        if dragging {
            transform.translation.x -= m.delta.x * zoom.0;
            transform.translation.y += m.delta.y * zoom.0;
        }
    }

    for w in wheel.read() {
        if over_ui {
            continue;
        }
        let step = match w.unit {
            MouseScrollUnit::Line => w.y * 0.1,
            MouseScrollUnit::Pixel => w.y * 0.002,
        };
        zoom.0 = (zoom.0 * (1.0 - step)).clamp(0.35, 2.5);
    }
    if let Projection::Orthographic(o) = &mut *projection {
        o.scale = zoom.0;
    }
}

pub fn click_select(
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    zoom: Res<CameraZoom>,
    mut game: ResMut<Game>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if ui::over_ui(cursor, &window) {
        return;
    }
    let (camera, cam_transform) = *camera;
    let Ok(world) = camera.viewport_to_world_2d(cam_transform, cursor) else {
        return;
    };
    let Some(galaxy) = &game.galaxy else { return };
    let hit = galaxy
        .systems
        .iter()
        .map(|s| (s, world_pos(s).distance(world)))
        .filter(|(_, d)| *d <= 22.0 * zoom.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(s, _)| (s.id, s.planets.clone()));
    let Some((system, planets)) = hit else { return };
    // Prefer a planet where you already have holdings.
    let me = game.me;
    let planet = game
        .view
        .as_ref()
        .and_then(|v| {
            planets.iter().copied().find(|&p| {
                v.planets
                    .get(p as usize)
                    .is_some_and(|pv| pv.buildings.iter().any(|b| Some(b.owner) == me))
            })
        })
        .or_else(|| planets.first().copied());
    game.selected_system = Some(system);
    game.selected_planet = planet;
    game.dirty = true;
}
