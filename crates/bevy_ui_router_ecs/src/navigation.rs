#![allow(dead_code)]

use bevy_ecs::resource::Resource;

struct PendingNavigationCommand;

#[derive(Resource)]
pub struct RouterNavigation {
    commands: Vec<PendingNavigationCommand>,
}

impl RouterNavigation {
    pub(crate) const fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationEnqueueErrorKind {
    UnknownRouter,
    HostUnavailable,
    AuthorityRecovering,
    NoBackEntry,
}

#[derive(Debug)]
pub struct NavigationEnqueueError {
    kind: NavigationEnqueueErrorKind,
}

impl NavigationEnqueueError {
    const fn new(kind: NavigationEnqueueErrorKind) -> Self {
        Self { kind }
    }

    pub fn kind(&self) -> NavigationEnqueueErrorKind {
        self.kind
    }
}

impl std::fmt::Display for NavigationEnqueueError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self.kind {
            NavigationEnqueueErrorKind::UnknownRouter => "router is not installed",
            NavigationEnqueueErrorKind::HostUnavailable => "router host is unavailable",
            NavigationEnqueueErrorKind::AuthorityRecovering => "router authority is recovering",
            NavigationEnqueueErrorKind::NoBackEntry => "router history has no back entry",
        })
    }
}

impl std::error::Error for NavigationEnqueueError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_error_traits<
        T: std::fmt::Debug + std::fmt::Display + std::error::Error + Send + Sync + 'static,
    >() {
    }

    fn public_methods(type_name: &str) -> Vec<(String, bool)> {
        let file = syn::parse_file(include_str!("navigation.rs")).unwrap();
        file.items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Impl(item) if item.trait_.is_none() => Some(item),
                _ => None,
            })
            .filter(|item| match item.self_ty.as_ref() {
                syn::Type::Path(path) => path.path.segments.last().unwrap().ident == type_name,
                _ => false,
            })
            .flat_map(|item| item.items.iter())
            .filter_map(|item| match item {
                syn::ImplItem::Fn(method) if matches!(method.vis, syn::Visibility::Public(_)) => {
                    Some((method.sig.ident.to_string(), method.sig.constness.is_some()))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn associated_api_defers_navigation_behavior_and_matches_the_error_golden() {
        assert!(public_methods("RouterNavigation").is_empty());
        assert_eq!(
            public_methods("NavigationEnqueueError"),
            vec![("kind".into(), false)]
        );
    }

    #[test]
    fn navigation_resource_starts_with_empty_private_command_storage() {
        fn assert_resource<T: bevy_ecs::resource::Resource>() {}
        fn assert_component<T: bevy_ecs::component::Component>() {}

        assert_resource::<RouterNavigation>();
        assert_component::<RouterNavigation>();

        let navigation = RouterNavigation::new();
        assert!(navigation.commands.is_empty());
    }

    #[test]
    fn enqueue_error_has_the_frozen_kinds_and_error_traits() {
        assert_error_traits::<NavigationEnqueueError>();

        for kind in [
            NavigationEnqueueErrorKind::UnknownRouter,
            NavigationEnqueueErrorKind::HostUnavailable,
            NavigationEnqueueErrorKind::AuthorityRecovering,
            NavigationEnqueueErrorKind::NoBackEntry,
        ] {
            assert_eq!(NavigationEnqueueError::new(kind).kind(), kind);
        }
    }
}
