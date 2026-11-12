use bevy_ecs::{prelude::Entity, system::SystemParam};
use bevy_ui_router_core::view_input::RouteInputRef;

use crate::{
    active_scope::{ActiveRouteScope, ActiveScopeRead},
    scroll::{RouteScrollRestoration, pending_restoration},
};

/// Reads activity, current inputs, and pending scroll restoration for routed content.
///
/// Call [`Self::read`] once before an application loop. It shares ownership and
/// hierarchy observations across targets while reading only Router metadata and
/// hierarchy, so mutable queries of application components can run alongside it.
#[derive(SystemParam)]
pub struct RouteQueries<'w, 's> {
    scope: ActiveRouteScope<'w, 's>,
}

impl<'w, 's> RouteQueries<'w, 's> {
    /// Starts an immutable read scope with fresh facts, even within the same tick.
    ///
    /// Order the system after [`crate::RouterScheduleSet::PublishSnapshots`] to
    /// see this update's publication. The reader and borrowed inputs borrow this
    /// parameter; a later read after World mutation observes the new structure.
    pub fn read(&self) -> RouteQueryReader<'_, 'w, 's> {
        RouteQueryReader {
            scope: self.scope.read(),
        }
    }
}

/// A borrowed view of current routed content, sharing work across queries.
///
/// Native dynamic descendants use their live containment. Retained, pending,
/// leaving, invalid, and unrelated content cannot inherit an active ancestor's
/// state. Activity remains independent of visibility and input eligibility.
pub struct RouteQueryReader<'a, 'w, 's> {
    scope: ActiveScopeRead<'a, 'w, 's>,
}

impl RouteQueryReader<'_, '_, '_> {
    /// Whether this target belongs to exact, currently active route content.
    pub fn is_active(&self, target: Entity) -> bool {
        self.scope.resolve(target).is_some()
    }

    /// Borrows route-local parameters and the published presentation location.
    ///
    /// `None` also indicates inactive content, so callers needing inputs do not
    /// need a separate activity query. Error pages use their failed location.
    pub fn current(&self, target: Entity) -> Option<RouteInputRef<'_>> {
        let (entry, frame) = self.scope.resolve(target)?;
        entry.engine.current_input(frame.frame())
    }

    /// Looks up an unfinished restoration for this target's active frame.
    ///
    /// The scroll container need not exist yet. The returned restoration follows
    /// the current history entry, frame, and container key.
    pub fn pending(&self, target: Entity, key: &str) -> Option<RouteScrollRestoration> {
        let (entry, frame) = self.scope.resolve(target)?;
        pending_restoration(entry, frame, key)
    }
}
