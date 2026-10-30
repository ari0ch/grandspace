//! Screen furniture: the top bar (clock, money, heat), the sidebar (system,
//! planet and orders) and the event log. Panels are rebuilt from the latest
//! server state a few times a second; there is no client-side game logic.

use bevy::prelude::*;
use grandspace_protocol::*;
use grandspace_sim::Power;

use crate::net::Net;
use crate::palette;
use crate::state::Game;

pub const TOP_BAR_H: f32 = 40.0;
pub const SIDEBAR_W: f32 = 390.0;
pub const LOG_W: f32 = 620.0;
pub const LOG_H: f32 = 170.0;
/// Seconds between UI rebuilds while the clock is running.
const REBUILD_INTERVAL: f32 = 0.2;

/// True when the cursor is over a panel rather than the map.
pub fn over_ui(cursor: Vec2, window: &Window) -> bool {
    cursor.y < TOP_BAR_H
        || cursor.x > window.width() - SIDEBAR_W
        || (cursor.y > window.height() - LOG_H && cursor.x < LOG_W)
}

#[derive(Component)]
pub struct TopBar;
#[derive(Component)]
pub struct Sidebar;
#[derive(Component)]
pub struct LogPanel;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiAction {
    SelectPlanet(u32),
    Order(WireAction),
    SetSpeed(f32),
    TogglePause,
}

#[derive(Component)]
pub struct UiButton {
    pub action: UiAction,
    pub enabled: bool,
}

pub fn spawn_panels(mut commands: Commands) {
    let panel = |node: Node| (node, BackgroundColor(palette::PANEL));
    commands.spawn((
        TopBar,
        panel(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Px(TOP_BAR_H),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            column_gap: Val::Px(18.0),
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Row,
            border: UiRect::bottom(Val::Px(1.0)),
            ..default()
        }),
        BorderColor::all(palette::PANEL_EDGE),
    ));
    commands.spawn((
        Sidebar,
        panel(Node {
            position_type: PositionType::Absolute,
            right: Val::Px(0.0),
            top: Val::Px(TOP_BAR_H),
            bottom: Val::Px(0.0),
            width: Val::Px(SIDEBAR_W),
            padding: UiRect::all(Val::Px(12.0)),
            row_gap: Val::Px(6.0),
            flex_direction: FlexDirection::Column,
            overflow: Overflow::clip(),
            border: UiRect::left(Val::Px(1.0)),
            ..default()
        }),
        BorderColor::all(palette::PANEL_EDGE),
    ));
    commands.spawn((
        LogPanel,
        panel(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            bottom: Val::Px(0.0),
            width: Val::Px(LOG_W),
            height: Val::Px(LOG_H),
            padding: UiRect::all(Val::Px(10.0)),
            row_gap: Val::Px(2.0),
            flex_direction: FlexDirection::Column,
            overflow: Overflow::clip(),
            border: UiRect::new(Val::Px(0.0), Val::Px(1.0), Val::Px(1.0), Val::Px(0.0)),
            ..default()
        }),
        BorderColor::all(palette::PANEL_EDGE),
    ));
}

fn text(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s.into()),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

fn row() -> Node {
    Node {
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        column_gap: Val::Px(6.0),
        row_gap: Val::Px(6.0),
        align_items: AlignItems::Center,
        ..default()
    }
}

fn button(
    parent: &mut ChildSpawnerCommands,
    label: impl Into<String>,
    action: UiAction,
    enabled: bool,
    active: bool,
) {
    let bg = if active {
        palette::BUTTON_ACTIVE
    } else {
        palette::BUTTON
    };
    let fg = if enabled {
        palette::TEXT
    } else {
        palette::TEXT_DIM
    };
    parent
        .spawn((
            Button,
            UiButton { action, enabled },
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BorderColor::all(palette::PANEL_EDGE),
            BackgroundColor(bg),
        ))
        .with_children(|b| {
            b.spawn(text(label, 13.0, fg));
        });
}

fn heading(parent: &mut ChildSpawnerCommands, s: impl Into<String>) {
    parent.spawn((
        text(s, 12.0, palette::TEXT_DIM),
        Node {
            margin: UiRect::top(Val::Px(8.0)),
            ..default()
        },
    ));
}

fn control_label(game: &Game, c: WireControl) -> (String, Color) {
    match c {
        WireControl::Unclaimed => ("unclaimed".into(), palette::TEXT_DIM),
        WireControl::Contested => ("contested".into(), palette::WARNING),
        WireControl::Controlled(f) => (
            game.faction_name(f),
            game.faction_power(f)
                .map(|p| palette::faction(p, f))
                .unwrap_or(palette::TEXT),
        ),
    }
}

pub fn rebuild(
    mut commands: Commands,
    mut game: ResMut<Game>,
    time: Res<Time>,
    mut since: Local<f32>,
    top: Single<Entity, With<TopBar>>,
    side: Single<Entity, With<Sidebar>>,
    log: Single<Entity, With<LogPanel>>,
) {
    *since += time.delta_secs();
    if !game.dirty || *since < REBUILD_INTERVAL {
        return;
    }
    *since = 0.0;
    game.dirty = false;

    commands.entity(*top).despawn_children().with_children(|p| {
        build_top_bar(p, &game);
    });
    commands
        .entity(*side)
        .despawn_children()
        .with_children(|p| {
            build_sidebar(p, &game);
        });
    commands.entity(*log).despawn_children().with_children(|p| {
        if let Some(n) = &game.notice {
            p.spawn(text(n.clone(), 13.0, palette::WARNING));
        }
        for line in &game.log {
            p.spawn(text(line.clone(), 12.5, palette::TEXT));
        }
        if game.log.is_empty() && game.notice.is_none() {
            p.spawn(text(
                "News from your network will appear here.",
                12.5,
                palette::TEXT_DIM,
            ));
        }
    });
}

fn build_top_bar(p: &mut ChildSpawnerCommands, game: &Game) {
    if let Some(reason) = &game.disconnected {
        p.spawn(text(
            format!("Disconnected: {reason}"),
            14.0,
            palette::DANGER,
        ));
        return;
    }
    let (Some(view), Some(galaxy), Some(me)) = (&game.view, &game.galaxy, game.me) else {
        p.spawn(text("Connecting to the galaxy…", 14.0, palette::TEXT_DIM));
        return;
    };
    let fac = &galaxy.factions[me as usize];
    let power: Power = fac.power.into();
    p.spawn(text(fac.name.clone(), 15.0, palette::faction(power, me)));
    p.spawn(text(power.name(), 12.0, palette::TEXT_DIM));
    p.spawn(text(view.date.clone(), 14.0, palette::TEXT));
    p.spawn(text(
        format!(
            "{:.0} {}  ({:+.1}/day)",
            view.funds, view.currency, view.income_per_day
        ),
        14.0,
        palette::TEXT,
    ));
    let heat_colour = if view.heat >= 60.0 {
        palette::DANGER
    } else if view.heat >= 40.0 {
        palette::WARNING
    } else {
        palette::TEXT
    };
    p.spawn(text(
        format!(
            "Heat {:.0}{}",
            view.heat,
            if view.heat >= 60.0 { "  EXPOSED" } else { "" }
        ),
        14.0,
        heat_colour,
    ));
    let pending = view.orders.len();
    if pending > 0 {
        p.spawn(text(
            format!("{pending} order(s) in transit"),
            12.0,
            palette::TEXT_DIM,
        ));
    }
    p.spawn(Node {
        flex_grow: 1.0,
        ..default()
    });
    if game.locked {
        p.spawn(text(
            format!("Shared galaxy · {:.0} days/s", game.speed),
            12.5,
            palette::TEXT_DIM,
        ));
        return;
    }
    p.spawn(row()).with_children(|r| {
        button(
            r,
            if game.paused { "Play" } else { "Pause" },
            UiAction::TogglePause,
            true,
            game.paused,
        );
        for speed in [1.0, 4.0, 16.0, 64.0] {
            let active = !game.paused && (game.speed - speed).abs() < 0.01;
            button(
                r,
                format!("{speed:.0} d/s"),
                UiAction::SetSpeed(speed),
                true,
                active,
            );
        }
    });
}

fn build_sidebar(p: &mut ChildSpawnerCommands, game: &Game) {
    let (Some(view), Some(galaxy), Some(me), Some(power)) =
        (&game.view, &game.galaxy, game.me, game.my_power())
    else {
        return;
    };
    let Some(sid) = game.selected_system else {
        p.spawn(text("Select a star system", 16.0, palette::TEXT));
        p.spawn(text(
            "Left-click a star. Drag with the right mouse button or use WASD to pan; scroll to zoom.",
            13.0,
            palette::TEXT_DIM,
        ));
        return;
    };
    let system = &galaxy.systems[sid as usize];
    let region = &galaxy.regions[system.region as usize].name;
    p.spawn(text(system.name.clone(), 18.0, palette::TEXT));
    let leader = view.system_leaders.get(sid as usize).copied().flatten();
    p.spawn(text(
        match leader {
            Some(f) => format!("{region} · led by {}", game.faction_name(f)),
            None => format!("{region} · no clear leader"),
        },
        12.5,
        palette::TEXT_DIM,
    ));

    heading(p, "PLANETS");
    for &pid in &system.planets {
        let info = &galaxy.planets[pid as usize];
        let pv = &view.planets[pid as usize];
        let (ctrl, _) = control_label(game, pv.control);
        let mine = pv.buildings.iter().any(|b| b.owner == me);
        let label = format!(
            "{}{}  ·  {:?}, {:?}  ·  {}",
            if mine { "● " } else { "" },
            info.name,
            info.kind,
            info.race,
            ctrl
        );
        button(
            p,
            label,
            UiAction::SelectPlanet(pid),
            true,
            game.selected_planet == Some(pid),
        );
    }

    let Some(pid) = game.selected_planet.filter(|p| system.planets.contains(p)) else {
        return;
    };
    let info = &galaxy.planets[pid as usize];
    let pv = &view.planets[pid as usize];
    let race: grandspace_sim::Race = info.race.into();

    heading(p, "HOLDING");
    p.spawn(text(info.name.clone(), 16.0, palette::TEXT));
    p.spawn(text(
        format!(
            "{:?} · {:.0}M {} ({}) · slots {}/{}",
            info.kind,
            info.population,
            race.name(),
            race.temperament(),
            pv.used_slots,
            info.slots
        ),
        12.5,
        palette::TEXT_DIM,
    ));
    let (ctrl, ctrl_colour) = control_label(game, pv.control);
    p.spawn(text(format!("Control: {ctrl}"), 13.5, ctrl_colour));
    p.spawn(text(
        format!(
            "{} · orders arrive {}",
            if pv.exact {
                "Exact intel"
            } else {
                "Estimates only (±35%)"
            },
            if pv.delay_days == 0 {
                "next day by relay".to_string()
            } else {
                format!("in {} days by courier", pv.delay_days)
            }
        ),
        12.5,
        palette::TEXT_DIM,
    ));

    heading(p, "INFLUENCE");
    let mut shares: Vec<(u32, f32)> = pv
        .influence
        .iter()
        .enumerate()
        .filter(|(_, v)| **v >= 0.5)
        .map(|(i, v)| (i as u32, *v))
        .collect();
    shares.sort_by(|a, b| b.1.total_cmp(&a.1));
    if shares.is_empty() {
        p.spawn(text(
            "Nobody has a foothold here yet.",
            12.5,
            palette::TEXT_DIM,
        ));
    }
    for (f, v) in shares.into_iter().take(5) {
        let c = game
            .faction_power(f)
            .map(|pw| palette::faction(pw, f))
            .unwrap_or(palette::TEXT);
        p.spawn(text(
            format!("{:>5.0}  {}", v, game.faction_name(f)),
            13.0,
            c,
        ));
    }

    if pv.exact && !pv.buildings.is_empty() {
        heading(p, "BUILDINGS");
        for b in &pv.buildings {
            let owner = game.faction_name(b.owner);
            let name = game.building_name(b.owner, b.kind);
            if b.owner == me || b.headquarters {
                p.spawn(text(
                    format!(
                        "{name} — {owner}{}",
                        if b.headquarters { " (HQ)" } else { "" }
                    ),
                    13.0,
                    palette::TEXT,
                ));
            } else {
                p.spawn(row()).with_children(|r| {
                    r.spawn(text(format!("{name} — {owner}"), 13.0, palette::TEXT));
                    button(
                        r,
                        format!("Sabotage ({:.0})", view.costs.sabotage),
                        UiAction::Order(WireAction::Sabotage {
                            planet: pid,
                            building_id: b.id,
                        }),
                        view.funds >= view.costs.sabotage,
                        false,
                    );
                });
            }
        }
    }

    heading(p, "ORDERS");
    let present = pv.buildings.iter().any(|b| b.owner == me);
    let free = (pv.used_slots as u32) < info.slots as u32;
    let afford = |c: f32| view.funds >= c;
    let build = |kind: WireBuilding| WireAction::Build {
        planet: pid,
        building: kind,
    };
    let bname = |k: grandspace_sim::BuildingKind| k.name(power);
    use grandspace_sim::BuildingKind as K;
    p.spawn(row()).with_children(|r| {
        if !present {
            button(
                r,
                format!("{} ({:.0})", bname(K::Outpost), view.costs.outpost),
                UiAction::Order(build(WireBuilding::Outpost)),
                pv.outpost_reach && free && afford(view.costs.outpost),
                false,
            );
            button(
                r,
                format!("Agent ({:.0})", view.costs.agent),
                UiAction::Order(WireAction::Agent { planet: pid }),
                afford(view.costs.agent),
                false,
            );
        } else {
            button(
                r,
                format!("{} ({:.0})", bname(K::Hall), view.costs.hall),
                UiAction::Order(build(WireBuilding::Hall)),
                free && afford(view.costs.hall),
                false,
            );
            button(
                r,
                format!("{} ({:.0})", bname(K::Income), view.costs.income),
                UiAction::Order(build(WireBuilding::Income)),
                free && afford(view.costs.income),
                false,
            );
            button(
                r,
                format!("{} ({:.0})", bname(K::Relay), view.costs.relay),
                UiAction::Order(build(WireBuilding::Relay)),
                free && afford(view.costs.relay),
                false,
            );
            button(
                r,
                format!("{} ({:.0})", power.campaign_name(), view.costs.campaign),
                UiAction::Order(WireAction::Campaign { planet: pid }),
                afford(view.costs.campaign),
                false,
            );
            button(
                r,
                format!("Bribe officials ({:.0})", view.costs.bribe),
                UiAction::Order(WireAction::Bribe { planet: pid }),
                afford(view.costs.bribe),
                false,
            );
        }
    });
    if !present && !pv.outpost_reach {
        p.spawn(text(
            "Too far from your holdings for an outpost: expand next door first.",
            12.0,
            palette::TEXT_DIM,
        ));
    }
}

pub fn handle_buttons(
    query: Query<(&Interaction, &UiButton), Changed<Interaction>>,
    net: NonSend<Net>,
    mut game: ResMut<Game>,
) {
    for (interaction, button) in &query {
        if *interaction != Interaction::Pressed || !button.enabled {
            continue;
        }
        match button.action {
            UiAction::SelectPlanet(p) => {
                game.selected_planet = Some(p);
                game.notice = None;
            }
            UiAction::Order(action) => net.send(ClientMsg::Order { action }),
            UiAction::SetSpeed(s) => {
                net.send(ClientMsg::SetSpeed { days_per_second: s });
                if game.paused {
                    net.send(ClientMsg::Resume);
                }
            }
            UiAction::TogglePause => net.send(if game.paused {
                ClientMsg::Resume
            } else {
                ClientMsg::Pause
            }),
        }
        game.dirty = true;
    }
}

pub fn button_hover(
    mut query: Query<(&Interaction, &UiButton, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, button, mut bg) in &mut query {
        if !button.enabled {
            continue;
        }
        *bg = match interaction {
            Interaction::Hovered | Interaction::Pressed => palette::BUTTON_HOVER,
            Interaction::None => palette::BUTTON,
        }
        .into();
    }
}

/// Space toggles pause, as in every grand strategy game.
pub fn keyboard(keys: Res<ButtonInput<KeyCode>>, net: NonSend<Net>, game: Res<Game>) {
    if keys.just_pressed(KeyCode::Space) && !game.locked {
        net.send(if game.paused {
            ClientMsg::Resume
        } else {
            ClientMsg::Pause
        });
    }
}
