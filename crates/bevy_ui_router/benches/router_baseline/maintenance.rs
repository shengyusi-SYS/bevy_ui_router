//! Steady observation after completed cleanup, and warmed retained navigation.
//! Timed work uses the same public App/update/navigation paths as the other modes.
use super::*;

const TERMINAL_RECORDS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Census {
    live: u32,
    requests: usize,
    trace_events: usize,
    diagnostics: usize,
}

fn census(fixture: &Fixture) -> Census {
    let snapshots = fixture.app.world().resource::<RouterSnapshots>();
    let mut result = Census {
        live: fixture.app.world().entities().count_spawned(),
        requests: 0,
        trace_events: 0,
        diagnostics: 0,
    };
    for &id in &fixture.issued {
        match snapshots.request(fixture.router, id) {
            RequestLookup::Found(request) => {
                assert!(matches!(request.status(), RequestStatus::Succeeded));
                result.requests += 1;
                result.trace_events += request.trace().len();
                result.diagnostics += request.diagnostics().len();
            }
            RequestLookup::NotRetained => {}
            RequestLookup::Unknown => panic!("a request from this fixture must be known"),
        }
    }
    assert_eq!(result.requests, TERMINAL_RECORDS);
    let snapshot = fixture.snapshot();
    assert_eq!(snapshot.history().entries().len(), 1);
    assert_eq!(snapshot.history().cursor(), Some(0));
    assert!(snapshot.running_request().is_none());
    assert!(!snapshot.cleanup().foreground_pending());
    assert_eq!(snapshot.cleanup().quarantine_count(), 0);
    result
}

fn report(
    scenario: &str,
    shape: ContentShape,
    active_children: usize,
    inactive_children: usize,
    retired_children: usize,
    batch: usize,
    samples: &[Sample],
    census: Census,
) {
    let shape = match shape {
        ContentShape::Flat => "flat",
        ContentShape::Nested => "nested",
    };
    let scope = if batch == 0 {
        "complete_navigation"
    } else {
        "mean_update"
    };
    println!(
        "{scenario},{shape},{scope},{active_children},{inactive_children},{retired_children},{},{},{},{batch},0,{},{},0",
        census.requests,
        census.trace_events,
        census.diagnostics,
        timing_columns(samples),
        census.live,
    );
}

fn terminal_history_idle(shape: ContentShape, samples: usize, smoke: bool) {
    let sizes: &[usize] = if smoke { &[0, 32] } else { &[0, 32, 128] };
    let batch = if smoke { 2 } else { IDLE_BATCH };
    let mut live = None;
    for &content in sizes {
        let mut fixture = Fixture::new(0, shape);
        for index in 0..TERMINAL_RECORDS {
            let old_page = only::<Page>(&mut fixture.app);
            spawn_control_content(fixture.app.world_mut(), old_page, content, shape.levels());
            fixture.navigate(Action::Replace, &format!("/items/retired-{index}"), false);
        }
        fixture.assert_page(&format!("retired-{}", TERMINAL_RECORDS - 1), 1, 0);
        let before = census(&fixture);
        assert_eq!(*live.get_or_insert(before.live), before.live);
        for _ in 0..64 {
            fixture.app.update();
        }
        let values = (0..samples)
            .map(|_| {
                let start = Instant::now();
                for _ in 0..batch {
                    fixture.app.update();
                }
                Sample {
                    elapsed: start.elapsed() / batch as u32,
                    updates: batch,
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(census(&fixture), before);
        report(
            "terminal_history_idle",
            shape,
            0,
            0,
            content,
            batch,
            &values,
            before,
        );
    }
}

fn retained_hit(shape: ContentShape, samples: usize, smoke: bool) {
    let sizes: &[usize] = if smoke { &[32] } else { &[32, 1024] };
    for &content in sizes {
        let mut fixture = Fixture::configured(0, shape, Some(2));
        let a = only::<Page>(&mut fixture.app);
        spawn_control_content(fixture.app.world_mut(), a, content, shape.levels());
        fixture.navigate(Action::Replace, "/items/b", false);
        let b = fixture
            .app
            .world_mut()
            .query::<(Entity, &Page)>()
            .iter(fixture.app.world())
            .find_map(|(entity, page)| (page.0 == "b").then_some(entity))
            .unwrap();
        spawn_control_content(fixture.app.world_mut(), b, content, shape.levels());
        fixture.navigate(Action::Replace, "/items/a", false);
        for index in 0..TERMINAL_RECORDS {
            let target = if index % 2 == 0 { "b" } else { "a" };
            fixture.navigate(Action::Replace, &format!("/items/{target}"), false);
        }
        assert_eq!(fixture.factory_count(), 2);
        // The factories remain empty; this is the expected dynamic budget used
        // by the existing census, not a change to their creation-time input.
        fixture.content = content;
        let pages = fixture.census("a", 1);
        assert_eq!(pages.active_page(), a);
        let before = census(&fixture);
        let values = (0..samples)
            .map(|index| {
                let target = if index % 2 == 0 { "b" } else { "a" };
                fixture.navigate(Action::Replace, &format!("/items/{target}"), false)
            })
            .collect::<Vec<_>>();
        let target = if samples % 2 == 0 { "a" } else { "b" };
        let after = fixture.census(target, 1);
        assert_eq!(after.active_page(), if samples % 2 == 0 { a } else { b });
        assert_eq!(fixture.factory_count(), 2);
        assert_eq!(census(&fixture), before);
        report(
            "retained_hit",
            shape,
            content,
            content,
            0,
            0,
            &values,
            before,
        );
        fixture.release_parent_scope();
    }
}

pub(super) fn run(shape: ContentShape, samples: usize, smoke: bool) {
    println!(
        "scenario,shape,sample_scope,active_children,inactive_children,retired_children_per_request,retained_requests,trace_events,diagnostics,updates_per_sample,content_queries_during_samples,samples,mean_us,p50_us,p95_us,max_us,min_updates,max_updates,live_entities,factory_builds_during_samples"
    );
    terminal_history_idle(shape, samples, smoke);
    retained_hit(shape, samples, smoke);
}
