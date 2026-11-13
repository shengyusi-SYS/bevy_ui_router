//! Fixed-size visit history on one real scrollable page. Preparation and Back
//! checks are untimed; each sample includes enqueue through terminal cleanup.
use super::*;

fn visit_position(index: usize) -> Vec2 {
    // Distinct integral positions survive physical pixel rounding in UI layout.
    Vec2::new(8.0 + (index % 32) as f32, 16.0 + (index / 32) as f32)
}

fn assert_position(state: &RetainedSteady, expected: Vec2) {
    let world = state.fixture.app.world();
    let actual = world.get::<ScrollPosition>(state.active).unwrap().0;
    assert!((actual - expected).abs().max_element() < 0.01);
    let node = world.get::<ComputedNode>(state.active).unwrap();
    assert!(node.size().min_element() > 0.0);
    assert!(node.content_size.y > node.size().y);
    assert!(
        (node.scroll_position * node.inverse_scale_factor - expected)
            .abs()
            .max_element()
            < 0.01
    );
    assert!(
        world
            .get::<ComputedUiTargetCamera>(state.active)
            .unwrap()
            .get()
            .is_some()
    );
    let target = world
        .get::<ComputedUiRenderTargetInfo>(state.active)
        .unwrap();
    assert!(target.physical_size().min_element() > 0);
    assert!(target.scale_factor() > 0.0);
}

fn scroll(state: &mut RetainedSteady, expected: Vec2) {
    state
        .fixture
        .app
        .world_mut()
        .get_mut::<ScrollPosition>(state.active)
        .unwrap()
        .0 = expected;
    state.fixture.app.update();
    assert_position(state, expected);
}

fn assert_stable(state: &mut RetainedSteady, visits: usize, cursor: usize, live: u32) {
    assert_eq!(only::<Page>(&mut state.fixture.app), state.active);
    assert_eq!(only::<Layout>(&mut state.fixture.app), state.parent);
    assert_eq!(state.fixture.factory_count(), 1);
    assert_eq!(state.fixture.app.world().entities().count_spawned(), live);
    let snapshot = state.fixture.snapshot();
    assert_eq!(
        snapshot.active().location(),
        Some(&RouteLocation::new("/items/a"))
    );
    assert_eq!(snapshot.history().entries().len(), visits);
    assert_eq!(snapshot.history().cursor(), Some(cursor));
    assert!(snapshot.running_request().is_none());
    assert!(!snapshot.cleanup().foreground_pending());
    assert_eq!(snapshot.cleanup().quarantine_count(), 0);
    assert_eq!(state.fixture.retained(), 64);
}

pub(super) fn run(shape: ContentShape, samples: usize, smoke: bool) {
    let sizes: &[usize] = if smoke { &[1, 32] } else { &[1, 128, 1024] };
    println!(
        "scenario,shape,ui,scroll_markers,history_entries,parent_children,active_children,retained_pages,sample_scope,samples,mean_us,p50_us,p95_us,max_us,min_updates,max_updates,live_entities,retained_requests,factory_builds,factory_builds_during_samples,back_checks"
    );
    for &visits in sizes {
        for marked in [false, true] {
            let mut state = RetainedSteady::new(shape, true);
            if !marked {
                state
                    .fixture
                    .app
                    .world_mut()
                    .entity_mut(state.active)
                    .remove::<RouteScroll>();
            }
            // Every completed layout samples a different displayed position;
            // identical-location Push creates visits without rebuilding entities.
            scroll(&mut state, visit_position(0));
            for index in 1..visits {
                state.fixture.navigate(Action::Push, "/items/a", false);
                scroll(&mut state, visit_position(index));
            }
            // Replace keeps the history length and current content fixed while
            // warming the terminal-record capacity and replacement workload.
            for _ in 0..64 {
                state.fixture.navigate(Action::Replace, "/items/a", false);
            }
            let live = state.fixture.app.world().entities().count_spawned();
            assert_stable(&mut state, visits, visits - 1, live);
            assert_position(&state, visit_position(visits - 1));
            let values = (0..samples)
                .map(|_| state.fixture.navigate(Action::Replace, "/items/a", false))
                .collect::<Vec<_>>();
            assert_stable(&mut state, visits, visits - 1, live);
            assert_position(&state, visit_position(visits - 1));

            // Verify saved visit records via production Back, rather than
            // inferring their presence from a marker or requested position.
            // The one-visit control adds a second visit only after timing.
            let checks = (visits - 1).clamp(1, 2);
            let check_visits = if visits == 1 {
                state.fixture.navigate(Action::Push, "/items/a", false);
                scroll(&mut state, visit_position(1));
                2
            } else {
                visits
            };
            for offset in 1..=checks {
                state.fixture.navigate(Action::Back, "", false);
                let index = check_visits - 1 - offset;
                let expected = if marked {
                    visit_position(index)
                } else {
                    visit_position(check_visits - 1)
                };
                assert_position(&state, expected);
                assert_stable(&mut state, check_visits, index, live);
            }
            let shape_name = match shape {
                ContentShape::Flat => "flat",
                ContentShape::Nested => "nested",
            };
            println!(
                "history_replace,{shape_name},1,{},{visits},{PARENT_CONTENT},{ACTIVE_CONTENT},0,complete_navigation,{},{live},64,1,0,{checks}",
                usize::from(marked),
                timing_columns(&values),
            );
            state.fixture.release_parent_scope();
        }
    }
}
