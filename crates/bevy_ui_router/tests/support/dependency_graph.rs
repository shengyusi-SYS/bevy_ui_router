/// Classifies dependencies for renderer-neutral feature checks, without
/// constraining their version or Cargo source.
pub fn is_bevy_dependency(name: &str) -> bool {
    name == "bevy" || name.starts_with("bevy_") && !name.starts_with("bevy_ui_router")
}
