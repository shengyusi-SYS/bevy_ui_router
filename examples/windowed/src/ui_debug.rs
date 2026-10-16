//! Example-only, opt-in BRP bridge using winit-compatible input messages.
//! Compiled only with debug assertions and the ui-debug feature.

use std::{io::Cursor, net::Ipv4Addr};

use bevy_input_focus::InputFocus;
use bevy_internal::{
    diagnostic::FrameCount,
    prelude::*,
    remote::{BrpError, BrpResult, RemotePlugin, http::RemoteHttpPlugin},
    render::view::window::screenshot::{Screenshot, ScreenshotCaptured},
    window::{PrimaryWindow, WindowEvent},
};
use bevy_ui_router::prelude::{
    RouteActivity, RouteInputs, RouteScrollRestorations, RouterInputGate,
};
use serde_json::{Value, json};

/// Exposes BRP on localhost:15703 and keeps the development window processing requests.
pub struct UiDebugPlugin;

impl Plugin for UiDebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Capture>()
            .add_plugins((
                RemotePlugin::default()
                    .with_method_main("router/ui_snapshot", snapshot)
                    .with_method_main("router/input", input)
                    .with_method_main("router/capture", capture)
                    .with_method_main("router/capture_status", capture_status),
                RemoteHttpPlugin::default()
                    .with_address(Ipv4Addr::LOCALHOST)
                    .with_port(15703),
            ))
            .add_systems(Last, keep_debug_window_running);
    }
}

fn input(params: In<Option<Value>>, world: &mut World) -> BrpResult {
    let event: WindowEvent =
        serde_json::from_value(params.0.unwrap_or(Value::Null)).map_err(|error| BrpError {
            code: -32602,
            message: error.to_string(),
            data: None,
        })?;
    // Like WinitAppRunnerState::send_events: picking consumes WindowEvent while
    // InputPlugin and text controls consume the individual typed messages.
    match &event {
        WindowEvent::CursorMoved(value) => {
            world.write_message(value.clone());
        }
        WindowEvent::MouseButtonInput(value) => {
            world.write_message(*value);
        }
        WindowEvent::MouseWheel(value) => {
            world.write_message(*value);
        }
        WindowEvent::KeyboardInput(value) => {
            world.write_message(value.clone());
        }
        WindowEvent::Ime(value) => {
            world.write_message(value.clone());
        }
        _ => {
            return Err(BrpError {
                code: -32602,
                message: "Expected cursor, mouse, keyboard or IME input".into(),
                data: None,
            });
        }
    }
    world.write_message(event);
    Ok(json!({"accepted": true}))
}

fn keep_debug_window_running(mut settings: ResMut<bevy_internal::winit::WinitSettings>) {
    // BRP's HTTP thread does not wake a reactive winit loop. Only this opt-in mode
    // overrides the production power policy; no business systems are replaced.
    if settings.focused_mode != bevy_internal::winit::UpdateMode::Continuous
        || settings.unfocused_mode != bevy_internal::winit::UpdateMode::Continuous
    {
        *settings = bevy_internal::winit::WinitSettings::continuous();
    }
}

fn snapshot(
    _: In<Option<Value>>,
    frame: Res<FrameCount>,
    activity: RouteActivity,
    inputs: RouteInputs,
    restorations: RouteScrollRestorations,
    gate: Res<RouterInputGate>,
    focus: Res<InputFocus>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    nodes: Query<(
        Entity,
        &ComputedNode,
        &UiGlobalTransform,
        Option<&Name>,
        Option<&Text>,
        Option<&ChildOf>,
        Option<&ScrollPosition>,
    )>,
) -> BrpResult {
    let windows: Vec<_> = windows
        .iter()
        .map(|(entity, window)| {
            json!({
                "entity": entity.to_bits(), "width": window.width(),
                "height": window.height(), "scale_factor": window.scale_factor()
            })
        })
        .collect();
    let nodes: Vec<_> = nodes
        .iter()
        .map(|(entity, node, transform, name, text, parent, scroll)| {
            let actual_scroll = node.scroll_position * node.inverse_scale_factor();
            let pending = restorations.pending(entity, "list");
            let input = inputs.current(entity);
            json!({
                "entity": entity.to_bits(), "name": name.map(Name::as_str),
                "text": text.map(|text| text.0.as_str()),
                "parent": parent.map(|parent| parent.parent().to_bits()),
                "center_physical": [transform.translation.x, transform.translation.y],
                "size_physical": [node.size().x, node.size().y],
                "router_active": activity.is_active(entity),
                "pointer_allowed": gate.allows_pointer(entity),
                "keyboard_allowed": gate.allows_keyboard(entity),
                "route_location": input.map(|input| input.location().path()),
                "route_search": input.and_then(|input| input.location().search()),
                "route_fragment": input.and_then(|input| input.location().fragment()),
                "route_id": input.and_then(|input| input.route()).map(|route| route.raw()),
                "pending_restore": pending.map(|intent| [intent.position().0.x, intent.position().0.y]),
                "scroll_requested": scroll.map(|scroll| [scroll.0.x, scroll.0.y]),
                "scroll_actual": [actual_scroll.x, actual_scroll.y]
            })
        })
        .collect();
    Ok(json!({"frame": frame.0, "windows": windows, "nodes": nodes,
        "focus": focus.get().map(Entity::to_bits)}))
}

#[derive(Resource, Default)]
struct Capture {
    ticket: u64,
    result: Option<Result<Vec<u8>, String>>,
}

fn capture(_: In<Option<Value>>, mut commands: Commands, mut state: ResMut<Capture>) -> BrpResult {
    state.ticket += 1;
    state.result = None;
    let ticket = state.ticket;
    commands.spawn(Screenshot::primary_window()).observe(
        move |event: On<ScreenshotCaptured>, mut state: ResMut<Capture>| {
            if state.ticket != ticket {
                return;
            }
            state.result = Some((|| {
                let image = event
                    .image
                    .clone()
                    .try_into_dynamic()
                    .map_err(|e| e.to_string())?;
                let mut bytes = Cursor::new(Vec::new());
                image::DynamicImage::ImageRgb8(image.to_rgb8())
                    .write_to(&mut bytes, image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?;
                Ok(bytes.into_inner())
            })());
        },
    );
    Ok(json!({"ticket": ticket}))
}

fn capture_status(params: In<Option<Value>>, state: Res<Capture>) -> BrpResult {
    let ticket = params
        .0
        .as_ref()
        .and_then(|value| value.get("ticket"))
        .and_then(Value::as_u64);
    if ticket != Some(state.ticket) || state.ticket == 0 {
        return Err(BrpError {
            code: -32602,
            message: "Unknown or superseded capture ticket".into(),
            data: None,
        });
    }
    match &state.result {
        None => Ok(json!({"status": "pending"})),
        Some(Ok(png)) => Ok(json!({"status": "complete", "png": png})),
        Some(Err(message)) => Err(BrpError {
            code: -32603,
            message: message.clone(),
            data: None,
        }),
    }
}
