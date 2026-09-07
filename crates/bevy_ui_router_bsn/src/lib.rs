#![forbid(unsafe_code)]

//! Renderer-side contracts for the official, synchronous Bevy Scene adapter.
//!
//! Factories describe fresh scenes; ECS owns their execution and Router lifecycle.

use bevy_ecs::{component::Component, template::FromTemplate};
use bevy_scene::Scene;
use bevy_ui_router_core::view_input::RouteViewInput;

/// Produces one owned scene for one view effect, without World or Router authority.
///
/// The result is consumed, not cloned or replayed. An error presentation invokes
/// its factory separately with its own read-only input.
pub type BsnSceneFactory =
    Box<dyn FnMut(&RouteViewInput) -> Result<Box<dyn Scene>, BsnBuildError> + Send + Sync>;

/// An application factory's readable failure, before a Scene is executed.
#[derive(Debug)]
pub struct BsnBuildError {
    message: String,
}

impl BsnBuildError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for BsnBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for BsnBuildError {}

/// Declares the frame-local anchor for a layout's single child route outlet.
///
/// This is an ordinary Scene component, not a mount binding or lifecycle owner.
#[derive(Component, FromTemplate, Default, Clone, Copy)]
pub struct DefaultOutlet;

pub mod prelude {
    pub use crate::{BsnBuildError, BsnSceneFactory, DefaultOutlet};
}
