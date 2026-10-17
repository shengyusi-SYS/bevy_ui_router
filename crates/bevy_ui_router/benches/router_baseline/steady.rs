//! One initial navigation, with ordinary content added after publication.
use super::*;
use bevy_ui::ScrollPosition;

pub(super) fn run(shape: ContentShape, samples: usize) {
    println!(
        "scenario,content_children,batch,samples,mean_us,p50_us,p95_us,max_us,min_updates,max_updates,live_entities,retained_requests"
    );
    for content in [128, 1024] {
        let mut plain = control(content, shape);
        let values = idle(&mut plain, samples);
        report(
            "idle_control",
            content,
            0,
            &values,
            plain.world().entities().count_spawned(),
            0,
        );

        // The existing fixture publishes a native layout and leaf. Populate
        // ordinary descendants afterwards, through the same public World path
        // available to application code; no retained pages or extra requests.
        let mut fixture = Fixture::new(0, shape);
        let page = only::<Page>(&mut fixture.app);
        spawn_control_content(fixture.app.world_mut(), page, content, shape.levels());
        fixture.content = content;
        fixture.app.update();
        fixture.assert_page("a", 1, 0);
        let live = fixture.app.world().entities().count_spawned();
        let values = idle(&mut fixture.app, samples);
        fixture.assert_page("a", 1, 0);
        assert_eq!(only::<Page>(&mut fixture.app), page);
        assert_eq!(fixture.retained(), 1);
        assert_eq!(fixture.app.world().entities().count_spawned(), live);
        report("idle_initial", content, 0, &values, live, 1);

        // A normal marker makes PostUpdate observe and sample the active visit
        // even without a restoration. Geometry is intentionally absent: this
        // mode measures routing observation, not UI layout or rendering.
        fixture
            .app
            .world_mut()
            .entity_mut(page)
            .insert((RouteScroll::new("body"), ScrollPosition::default()));
        fixture.app.update();
        let values = idle(&mut fixture.app, samples);
        fixture.assert_page("a", 1, 0);
        assert_eq!(only::<Page>(&mut fixture.app), page);
        assert_eq!(fixture.retained(), 1);
        assert_eq!(fixture.app.world().entities().count_spawned(), live);
        assert_eq!(
            fixture.app.world().get::<ScrollPosition>(page).unwrap().0,
            Default::default()
        );
        report("idle_initial_scroll", content, 0, &values, live, 1);
    }
}
