use std::sync::atomic::{AtomicU64, Ordering};

use bevy_camera::visibility::Visibility;
use bevy_ecs::{entity::Entity, hierarchy::ChildOf, prelude::World, resource::Resource};
use bevy_picking::Pickable;
use bevy_ui_router_core::{HostVersion, frame::RouteFrameVersion, ids::RouteSlotId};

use crate::{
    app_ext::RouterHandle,
    components::{ProvisionalSlotLifecycle, RouteOwnershipSeal, RouterInputBarrier},
    content::{ContentColor, ContentPrimitiveKind},
    host::validated_host_candidate_is_current,
    navigation::RouterEntry,
    presentation::{FrameLifecycle, RealizedFrame, validate_active_frame, validate_realized_frame},
    registry::{EntityLifecycle, EntityOwner, RegistryOwner, RouteEntityRegistry},
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AuditEntityId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditEntityState {
    PresentOwned,
    Quarantined,
    Removed,
    NotRetained,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityHostMapping {
    version: HostVersion,
    entity: AuditEntityId,
    top_slot: AuditEntityId,
    app_entity: Entity,
}

impl AuthorityHostMapping {
    pub fn version(&self) -> &HostVersion {
        &self.version
    }

    pub fn entity(&self) -> AuditEntityId {
        self.entity
    }

    pub fn top_slot(&self) -> AuditEntityId {
        self.top_slot
    }

    pub fn matches_app_entity(&self, entity: Entity) -> bool {
        self.app_entity == entity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoritySlotKind {
    HostTop,
    ChildOutlet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoritySlotMapping {
    id: RouteSlotId,
    entity: AuditEntityId,
    host: HostVersion,
    kind: AuthoritySlotKind,
}

impl AuthoritySlotMapping {
    pub fn id(&self) -> RouteSlotId {
        self.id
    }

    pub fn entity(&self) -> AuditEntityId {
        self.entity
    }

    pub fn host(&self) -> &HostVersion {
        &self.host
    }

    pub fn kind(&self) -> AuthoritySlotKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityFrameLifecycle {
    PreparedInert,
    Active,
    LeavingInert,
    Quarantined,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityFrameMapping {
    frame: RouteFrameVersion,
    root: AuditEntityId,
    slot_entity: AuditEntityId,
    lifecycle: AuthorityFrameLifecycle,
}

impl AuthorityFrameMapping {
    pub fn frame(&self) -> &RouteFrameVersion {
        &self.frame
    }

    pub fn root(&self) -> AuditEntityId {
        self.root
    }

    pub fn slot_entity(&self) -> AuditEntityId {
        self.slot_entity
    }

    pub fn lifecycle(&self) -> AuthorityFrameLifecycle {
        self.lifecycle
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityContentMapping {
    frame: RouteFrameVersion,
    entity: AuditEntityId,
    width_px: u16,
    height_px: u16,
    color: ContentColor,
    visibility_hidden: bool,
    pickable_active: bool,
}

impl AuthorityContentMapping {
    pub fn frame(&self) -> &RouteFrameVersion {
        &self.frame
    }

    pub fn entity(&self) -> AuditEntityId {
        self.entity
    }

    pub fn primitive(&self) -> ContentPrimitiveKind {
        ContentPrimitiveKind::SolidRect
    }

    pub fn width_px(&self) -> u16 {
        self.width_px
    }

    pub fn height_px(&self) -> u16 {
        self.height_px
    }

    pub fn color(&self) -> ContentColor {
        self.color
    }

    pub fn visibility_hidden(&self) -> bool {
        self.visibility_hidden
    }

    pub fn pickable_active(&self) -> bool {
        self.pickable_active
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouterAuthorityAuditSnapshot {
    host: Option<AuthorityHostMapping>,
    slots: Vec<AuthoritySlotMapping>,
    frames: Vec<AuthorityFrameMapping>,
    content: Vec<AuthorityContentMapping>,
    owned: Vec<AuditEntityId>,
}

impl RouterAuthorityAuditSnapshot {
    pub fn host(&self) -> Option<&AuthorityHostMapping> {
        self.host.as_ref()
    }

    pub fn slots(&self) -> &[AuthoritySlotMapping] {
        &self.slots
    }

    pub fn frames(&self) -> &[AuthorityFrameMapping] {
        &self.frames
    }

    pub fn content(&self) -> &[AuthorityContentMapping] {
        &self.content
    }

    pub fn owned_entity_state(&self, entity: AuditEntityId) -> AuditEntityState {
        if self.owned.contains(&entity) {
            AuditEntityState::PresentOwned
        } else {
            AuditEntityState::Unknown
        }
    }
}

#[derive(Resource)]
pub struct RouterAuthorityAudits {
    entity_ids: Vec<(Entity, AuditEntityId)>,
    snapshots: Vec<(RouterHandle, RouterAuthorityAuditSnapshot)>,
}

impl RouterAuthorityAudits {
    pub(crate) const fn new() -> Self {
        Self {
            entity_ids: Vec::new(),
            snapshots: Vec::new(),
        }
    }

    pub fn get(&self, router: RouterHandle) -> Option<&RouterAuthorityAuditSnapshot> {
        self.snapshots
            .iter()
            .find_map(|(stored, snapshot)| (*stored == router).then_some(snapshot))
    }

    pub(crate) fn refresh(&mut self, world: &World, entries: &[RouterEntry]) {
        let mut snapshots = Vec::with_capacity(entries.len());
        for entry in entries {
            let Some(host) = entry.host.as_ref() else {
                snapshots.push((
                    entry.handle,
                    RouterAuthorityAuditSnapshot {
                        host: None,
                        slots: Vec::new(),
                        frames: Vec::new(),
                        content: Vec::new(),
                        owned: Vec::new(),
                    },
                ));
                continue;
            };
            let host_is_current = validated_host_candidate_is_current(world, &host.candidate)
                && world
                    .get_resource::<RouteEntityRegistry>()
                    .and_then(|registry| registry.record(host.top_slot))
                    .is_some_and(|record| {
                        record.claim() == host.top_slot_claim
                            && matches!(
                                record.owner(),
                                    RegistryOwner::Committed(EntityOwner::HostTopSlot {
                                        host: version,
                                        slot,
                                    }) if version == &host.version && *slot == host.slot_id
                            )
                            && record.lifecycle() == EntityLifecycle::PreparedInert
                            && record.manifest().matches(world, host.top_slot)
                            && world.get::<RouteOwnershipSeal>(host.top_slot)
                                == Some(&RouteOwnershipSeal::new(record.seal()))
                    })
                && world.get::<ChildOf>(host.top_slot).map(ChildOf::parent) == Some(host.host)
                && world.get::<Visibility>(host.top_slot) == Some(&Visibility::Visible)
                && world.get::<Pickable>(host.top_slot) == Some(&Pickable::IGNORE)
                && world
                    .get::<RouterInputBarrier>(host.top_slot)
                    .is_some_and(RouterInputBarrier::denies_all)
                && world.get::<ProvisionalSlotLifecycle>(host.top_slot)
                    == Some(&ProvisionalSlotLifecycle::PreparedInert);
            if !host_is_current {
                snapshots.push((
                    entry.handle,
                    RouterAuthorityAuditSnapshot {
                        host: None,
                        slots: Vec::new(),
                        frames: Vec::new(),
                        content: Vec::new(),
                        owned: Vec::new(),
                    },
                ));
                continue;
            }
            let host_id = self.id_for(host.host);
            let top_slot_id = self.id_for(host.top_slot);
            let mut owned = vec![top_slot_id];
            let mut frames = Vec::new();
            let mut content = Vec::new();
            for (frame, lifecycle) in [
                (
                    entry.pending_frame(),
                    AuthorityFrameLifecycle::PreparedInert,
                ),
                (entry.active.as_ref(), AuthorityFrameLifecycle::Active),
                (entry.leaving_frame(), AuthorityFrameLifecycle::LeavingInert),
            ] {
                let Some(frame) = frame else { continue };
                let frame_is_current = match lifecycle {
                    AuthorityFrameLifecycle::PreparedInert => validate_realized_frame(world, frame),
                    AuthorityFrameLifecycle::Active => validate_active_frame(world, frame),
                    AuthorityFrameLifecycle::LeavingInert => {
                        frame_registry_is_current(world, frame, EntityLifecycle::Leaving)
                    }
                    AuthorityFrameLifecycle::Quarantined => false,
                };
                if !frame_is_current {
                    continue;
                }
                let root = self.id_for(frame.shell());
                owned.push(root);
                frames.push(AuthorityFrameMapping {
                    frame: frame.frame().clone(),
                    root,
                    slot_entity: top_slot_id,
                    lifecycle,
                });
                for (entity, rect) in frame.audit_content() {
                    let id = self.id_for(entity);
                    owned.push(id);
                    content.push(AuthorityContentMapping {
                        frame: frame.frame().clone(),
                        entity: id,
                        width_px: rect.width_px(),
                        height_px: rect.height_px(),
                        color: rect.color(),
                        visibility_hidden: world.get::<Visibility>(entity)
                            == Some(&Visibility::Hidden),
                        pickable_active: world.get::<Pickable>(entity).is_some_and(|pickable| {
                            pickable.should_block_lower && pickable.is_hoverable
                        }),
                    });
                }
            }
            snapshots.push((
                entry.handle,
                RouterAuthorityAuditSnapshot {
                    host: Some(AuthorityHostMapping {
                        version: host.version.clone(),
                        entity: host_id,
                        top_slot: top_slot_id,
                        app_entity: host.host,
                    }),
                    slots: vec![AuthoritySlotMapping {
                        id: host.slot_id,
                        entity: top_slot_id,
                        host: host.version.clone(),
                        kind: AuthoritySlotKind::HostTop,
                    }],
                    frames,
                    content,
                    owned,
                },
            ));
        }
        self.snapshots = snapshots;
    }

    fn id_for(&mut self, entity: Entity) -> AuditEntityId {
        if let Some((_, id)) = self.entity_ids.iter().find(|(stored, _)| *stored == entity) {
            return *id;
        }
        static NEXT_AUDIT_ID: AtomicU64 = AtomicU64::new(1);
        let raw = NEXT_AUDIT_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .expect("audit id exhausted");
        let id = AuditEntityId(raw);
        self.entity_ids.push((entity, id));
        id
    }
}

fn frame_registry_is_current(
    world: &World,
    frame: &RealizedFrame,
    lifecycle: EntityLifecycle,
) -> bool {
    frame.claims().all(|claim| {
        claim.lifecycle() == lifecycle
            && world
                .get_resource::<RouteEntityRegistry>()
                .and_then(|registry| registry.record(claim.entity()))
                .is_some_and(|record| {
                    record.claim() == *claim
                        && record.lifecycle() == lifecycle
                        && record.manifest().matches(world, claim.entity())
                })
            && world.get::<Visibility>(claim.entity()) == Some(&Visibility::Hidden)
            && world.get::<Pickable>(claim.entity()) == Some(&Pickable::IGNORE)
            && world.get::<RouteOwnershipSeal>(claim.entity())
                == Some(&RouteOwnershipSeal::new(claim.seal()))
            && world.get::<FrameLifecycle>(claim.entity()) == Some(&FrameLifecycle(lifecycle))
            && world
                .get::<RouterInputBarrier>(claim.entity())
                .is_some_and(RouterInputBarrier::denies_all)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_ext::{AppRouterExt, RouterConfig, RouterHostRegistration},
        content::{ContentColor, FrameContentWriter, SolidRect},
        driver::{EcsViewDriver, EcsViewReportSession, ViewBuildReport},
        navigation::{
            RouterNavigation, collect_navigation_system, commit_presentation_system,
            drive_navigation_system, observe_authority_system,
        },
        plugin::BevyUiRouterPlugin,
    };
    use bevy_app::App;
    use bevy_ui::Node;
    use bevy_ui_router_core::prelude::{
        RouteLocation, RouteNodeOptions, RoutePattern, RouteTree, RouteViewKey,
    };
    use bevy_ui_router_view::{
        RouteViewDescriptor, RouteViewRegistry, ViewBuildEffect, ViewDriverKey,
    };

    struct AuditDriver(ViewDriverKey);

    impl EcsViewDriver for AuditDriver {
        fn key(&self) -> ViewDriverKey {
            self.0.clone()
        }

        fn apply(
            &mut self,
            _effect: &ViewBuildEffect,
            content: &mut FrameContentWriter<'_>,
            report: EcsViewReportSession<'_>,
        ) -> ViewBuildReport {
            content
                .spawn_empty()
                .expect("structural output should be accepted");
            content
                .spawn_solid_rect(SolidRect::new(8, 6, ContentColor::srgba8(1, 2, 3, 255)).unwrap())
                .unwrap();
            report.applied()
        }
    }

    #[test]
    fn audit_refreshes_at_each_authority_mutation_and_omits_structural_outputs() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let driver = ViewDriverKey::new("audit-driver").unwrap();
        app.register_ui_view_driver(AuditDriver(driver.clone()))
            .unwrap();
        let view = RouteViewKey::new("root").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(view.clone(), RouteViewDescriptor::new(driver))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(routes, views, view))
            .unwrap();
        let host = app
            .world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)))
            .id();

        observe_authority_system(app.world_mut());
        let host_snapshot = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .unwrap();
        assert!(host_snapshot.host().unwrap().matches_app_entity(host));
        assert_eq!(host_snapshot.slots().len(), 1);
        assert!(host_snapshot.frames().is_empty());

        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let pending = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .unwrap();
        assert_eq!(pending.frames().len(), 1);
        assert_eq!(
            pending.frames()[0].lifecycle(),
            AuthorityFrameLifecycle::PreparedInert
        );
        assert_eq!(pending.content().len(), 1);
        assert!(pending.content()[0].visibility_hidden());
        assert!(!pending.content()[0].pickable_active());

        commit_presentation_system(app.world_mut());
        let active = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .unwrap();
        assert_eq!(
            active.frames()[0].lifecycle(),
            AuthorityFrameLifecycle::Active
        );
        assert!(!active.content()[0].visibility_hidden());
        assert!(active.content()[0].pickable_active());
        assert_eq!(active.frames()[0].slot_entity(), active.slots()[0].entity());
        assert_eq!(
            active.owned_entity_state(active.frames()[0].root()),
            AuditEntityState::PresentOwned
        );
        assert_ne!(active.frames()[0].root(), active.content()[0].entity());

        let stale_root = active.frames()[0].root();
        let shell = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .shell();
        let claim = app
            .world()
            .resource::<crate::registry::RouteEntityRegistry>()
            .record(shell)
            .unwrap()
            .claim();
        app.world_mut()
            .resource_mut::<crate::registry::RouteEntityRegistry>()
            .compare_exchange_lifecycle(&claim, crate::registry::EntityLifecycle::Quarantined)
            .unwrap();
        drive_navigation_system(app.world_mut());

        let drifted = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .unwrap();
        assert!(drifted.frames().is_empty());
        assert!(drifted.content().is_empty());
        assert_eq!(
            drifted.owned_entity_state(stale_root),
            AuditEntityState::Unknown
        );
    }

    #[test]
    fn audit_entity_ids_are_process_unique_even_for_equal_entity_bits_in_distinct_worlds() {
        let mut first_world = World::new();
        let mut second_world = World::new();
        let first_entity = first_world.spawn_empty().id();
        let second_entity = second_world.spawn_empty().id();
        assert_eq!(first_entity.to_bits(), second_entity.to_bits());
        let mut first = RouterAuthorityAudits::new();
        let mut second = RouterAuthorityAudits::new();

        assert_ne!(first.id_for(first_entity), second.id_for(second_entity));
    }
}
