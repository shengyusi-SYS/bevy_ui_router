use super::*;
use crate::components::RouteOwnershipSeal;

#[test]
fn fresh_cleanup_freezes_invalid_selected_stock_as_protected_observation() {
    use crate::{navigation::RouterNavigation, plugin::BevyUiRouterPlugin};
    use bevy_ui_router_core::{prelude::RouteLocation, request::RequestStatus};

    #[derive(bevy_ecs::component::Component)]
    struct LaterStockDrift;

    for absent in [false, true] {
        let mut app = bevy_app::App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = crate::navigation::tests::install_retained_slice_router(&mut app, 2);
        app.update();
        for path in ["/items/1", "/items/2"] {
            let request = app
                .world_mut()
                .resource_mut::<RouterNavigation>()
                .push(router, RouteLocation::new(path))
                .unwrap();
            for _ in 0..12 {
                app.update();
                if app.world().resource::<RouterNavigation>().entries[0]
                    .engine
                    .request(request)
                    .is_some_and(|record| record.status() == &RequestStatus::Succeeded)
                {
                    break;
                }
            }
            assert_eq!(
                app.world().resource::<RouterNavigation>().entries[0]
                    .engine
                    .request(request)
                    .unwrap()
                    .status(),
                &RequestStatus::Succeeded
            );
        }
        let (projection, affected, shell) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let projection = entry.retained.inventory().unwrap()[0].projection().clone();
            let payload = entry.retained.payload_for(&projection).unwrap();
            let shell = payload.frame().shell();
            let affected = if absent {
                payload
                    .claims()
                    .find(|claim| claim.entity() != shell)
                    .unwrap()
                    .entity()
            } else {
                shell
            };
            (projection, affected, shell)
        };
        if absent {
            app.world_mut().despawn(affected);
        } else {
            app.world_mut()
                .entity_mut(affected)
                .remove::<RouteOwnershipSeal>();
        }
        let (partition, source, manifest, top_slot) = {
            let registry = app.world().resource::<RouteEntityRegistry>();
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let payload = entry.retained.payload_for(&projection).unwrap();
            assert!(matches!(
                registry.classify_retained_payload(
                    app.world(),
                    &projection,
                    payload.frame(),
                    payload.child_slots()
                ),
                Some(crate::presentation::RetainedPayloadClassification::OwnInvalid(_))
            ));
            let selected = [(&projection, payload)];
            let host = entry.host.as_ref().unwrap();
            let manifest = FrozenCleanupManifest::empty();
            let partition = manifest
                .freeze_failure_cleanup_partition_with_selected(
                    app.world(),
                    registry,
                    entry.active.as_ref(),
                    None,
                    host,
                    &selected,
                )
                .expect("invalid stock is protected by observation, not an activation proof");
            let observed = partition
                .protected_observed
                .iter()
                .find(|item| item.entity == affected)
                .unwrap();
            assert_eq!(observed.present, !absent);
            assert!(partition.allowed_writes.is_empty());
            let source = FailureCleanupSourceProjection::observe_with_selected(
                Some(host),
                entry.active.as_ref(),
                None,
                &selected,
            );
            let active_target_manifest = FrozenCleanupManifest::from_current_frame(
                app.world(),
                registry,
                entry.active.as_ref().unwrap().frames().last().unwrap(),
            )
            .unwrap();
            assert!(
                active_target_manifest
                    .freeze_failure_cleanup_partition_with_selected(
                        app.world(),
                        registry,
                        entry.active.as_ref(),
                        None,
                        host,
                        &selected,
                    )
                    .is_none(),
                "current cannot become a fresh cleanup target"
            );
            (partition, source, manifest, host.top_slot)
        };
        assert!(
            partition
                .refreeze(
                    app.world(),
                    Some(&source),
                    &manifest.obligations,
                    &manifest.mount_parents,
                    &[],
                    manifest.digest(),
                    None,
                    FailureCleanupTransitionStage::MountParentsCommitted
                )
                .is_ok()
        );
        let no_stock = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            FailureCleanupSourceProjection::observe_with_prepared(
                entry.host.as_ref(),
                entry.active.as_ref(),
                None,
            )
        };
        assert!(matches!(
            partition.refreeze(
                app.world(),
                Some(&no_stock),
                &manifest.obligations,
                &manifest.mount_parents,
                &[],
                manifest.digest(),
                None,
                FailureCleanupTransitionStage::MountParentsCommitted
            ),
            Err(FailureCleanupSettlementRefreezeEvidence {
                failure: FailureCleanupPartitionRefreezeFailure::Source { .. },
                ..
            })
        ));
        app.world_mut().entity_mut(shell).insert(LaterStockDrift);
        assert!(matches!(
            partition.refreeze(
                app.world(),
                Some(&source),
                &manifest.obligations,
                &manifest.mount_parents,
                &[],
                manifest.digest(),
                None,
                FailureCleanupTransitionStage::MountParentsCommitted
            ),
            Err(FailureCleanupSettlementRefreezeEvidence {
                failure: FailureCleanupPartitionRefreezeFailure::Protected { .. },
                ..
            })
        ));
        app.world_mut()
            .entity_mut(top_slot)
            .remove::<RouteOwnershipSeal>();
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let payload = entry.retained.payload_for(&projection).unwrap();
        assert!(
            manifest
                .freeze_failure_cleanup_partition_with_selected(
                    app.world(),
                    app.world().resource::<RouteEntityRegistry>(),
                    entry.active.as_ref(),
                    None,
                    entry.host.as_ref().unwrap(),
                    &[(&projection, payload)]
                )
                .is_none(),
            "the host authority predicate stays required"
        );
    }
}
