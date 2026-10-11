//! Parent reuse, fresh child, full hit and capacity-driven dependency disposal.
use super::*;
use bevy_ecs::template::FromTemplate;

// Fill the current public terminal-request retention before steady-state timing.
const INSTANCE_WARMUP: usize = 64;

#[derive(Component, Clone, Default)]
struct Parent(String, u32);

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Disabled,
    Parent,
    Both,
    Eviction,
}

struct Instances {
    fixture: Fixture,
    parent_builds: Arc<AtomicUsize>,
    mode: Mode,
}

struct Census {
    parents: Vec<(Entity, bool)>,
    pages: Vec<(Entity, bool)>,
    live: u32,
}

impl Instances {
    fn new(content: usize, shape: ContentShape, mode: Mode) -> Self {
        let mut app = base_app();
        let parent_builds = Arc::new(AtomicUsize::new(0));
        let calls = parent_builds.clone();
        let parent = app
            .register_bsn_view(ViewDriverKey::new("parent").unwrap(), move |input| {
                calls.fetch_add(1, Ordering::Relaxed);
                let name = input.params().get("section").unwrap().to_owned();
                Ok(SceneFunction(move |_, root: &mut ResolvedScene| {
                    root.push_template(Node::default());
                    root.push_template(Parent(name, 0));
                    append_content(root, content, shape.levels());
                    let mut outlet = ResolvedScene::default();
                    outlet.push_template(Node::default());
                    outlet.push_template(<DefaultOutlet as FromTemplate>::Template::default());
                    root.get_or_insert_related_resolved_scenes::<ChildOf>()
                        .scenes
                        .push(outlet);
                }))
            })
            .unwrap();
        let page_builds = Arc::new(AtomicUsize::new(0));
        let calls = page_builds.clone();
        let page = app
            .register_bsn_view(ViewDriverKey::new("page").unwrap(), move |input| {
                calls.fetch_add(1, Ordering::Relaxed);
                let id = input.params().get("id").unwrap().to_owned();
                Ok(SceneFunction(move |_, root: &mut ResolvedScene| {
                    root.push_template(Node::default());
                    root.push_template(Page(id));
                    append_content(root, content, shape.levels());
                }))
            })
            .unwrap();
        let app_driver = app
            .register_bsn_view(ViewDriverKey::new("app").unwrap(), |_| {
                Ok(bsn! { Node DefaultOutlet })
            })
            .unwrap();
        let broken = app
            .register_bsn_view(ViewDriverKey::new("broken").unwrap(), |_| {
                Err::<(), _>(BsnBuildError::new("baseline factory failure"))
            })
            .unwrap();
        let fallback = app
            .register_bsn_view(ViewDriverKey::new("fallback").unwrap(), |input| {
                assert!(input.failure().is_some());
                Ok(bsn! { Node ErrorPage })
            })
            .unwrap();
        let mut views = RouteViewRegistry::new();
        for (key, driver) in [
            ("app", app_driver),
            ("parent", parent),
            ("page", page),
            ("broken", broken),
            ("fallback", fallback),
        ] {
            views
                .insert(
                    RouteViewKey::new(key).unwrap(),
                    RouteViewDescriptor::new(driver),
                )
                .unwrap();
        }
        let mut routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::pathless(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("app").unwrap()),
            )
            .unwrap();
        let root = routes.root_id().unwrap();
        let parent = routes
            .route_with_options(
                root,
                RoutePattern::path(":section").unwrap(),
                RouteNodeOptions::new()
                    .with_view(RouteViewKey::new("parent").unwrap())
                    .with_keep_alive(mode != Mode::Disabled),
            )
            .unwrap();
        routes
            .route_with_options(
                parent,
                RoutePattern::path(":id").unwrap(),
                RouteNodeOptions::new()
                    .with_view(RouteViewKey::new("page").unwrap())
                    .with_keep_alive(matches!(mode, Mode::Both | Mode::Eviction)),
            )
            .unwrap();
        routes
            .route_with_options(
                root,
                RoutePattern::path("broken").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("broken").unwrap()),
            )
            .unwrap();
        let router = app
            .add_ui_router(
                RouterConfig::new(
                    routes.build().unwrap(),
                    views,
                    RouteViewKey::new("fallback").unwrap(),
                )
                .with_keep_alive_capacity(if mode == Mode::Disabled {
                    0
                } else {
                    2
                }),
            )
            .unwrap();
        let host = app
            .world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)))
            .id();
        app.update();
        let mut fixture = Fixture {
            app,
            router,
            host,
            content,
            shape,
            issued: Vec::new(),
            factory_calls: Some(page_builds),
        };
        fixture.navigate(Action::Replace, "/p/a", false);
        Self {
            fixture,
            parent_builds,
            mode,
        }
    }

    fn census(&mut self, section: &str, id: &str) -> Census {
        let fixture = &mut self.fixture;
        let (parents, pages, active_content, inactive_content) = fixture
            .app
            .world_mut()
            .run_system_once(
                |activity: RouteActivity,
                 parents: Query<Entity, With<Parent>>,
                 pages: Query<Entity, With<Page>>,
                 content: Query<Entity, With<Content>>| {
                    let collect = |entities: Vec<Entity>| {
                        entities
                            .into_iter()
                            .map(|entity| (entity, activity.is_active(entity)))
                            .collect::<Vec<_>>()
                    };
                    let active = content
                        .iter()
                        .filter(|entity| activity.is_active(*entity))
                        .count();
                    (
                        collect(parents.iter().collect()),
                        collect(pages.iter().collect()),
                        active,
                        content.iter().count() - active,
                    )
                },
            )
            .unwrap();
        let active_parent = parents.iter().find(|(_, active)| *active).unwrap().0;
        let active_page = pages.iter().find(|(_, active)| *active).unwrap().0;
        assert_eq!(parents.iter().filter(|(_, active)| *active).count(), 1);
        assert_eq!(pages.iter().filter(|(_, active)| *active).count(), 1);
        assert_eq!(
            fixture.app.world().get::<Parent>(active_parent).unwrap().0,
            section
        );
        assert_eq!(fixture.app.world().get::<Page>(active_page).unwrap().0, id);
        assert_eq!(active_content, fixture.content * 2);
        let inactive = parents.len() + pages.len() - 2;
        assert!(inactive <= if self.mode == Mode::Disabled { 0 } else { 2 });
        assert_eq!(inactive_content, fixture.content * inactive);
        let gate = fixture.app.world().resource::<RouterInputGate>();
        for (entity, active) in parents.iter().chain(&pages) {
            assert_eq!(gate.allows_keyboard(*entity), *active);
        }
        let location = RouteLocation::new(format!("/{section}/{id}"));
        assert_eq!(fixture.snapshot().active().location(), Some(&location));
        assert_eq!(fixture.snapshot().history().current(), Some(&location));
        assert_eq!(fixture.snapshot().history().entries().len(), 1);
        Census {
            parents,
            pages,
            live: fixture.app.world().entities().count_spawned(),
        }
    }

    fn release(&mut self) {
        let old = self
            .fixture
            .app
            .world_mut()
            .query_filtered::<Entity, Or<(With<Parent>, With<Page>, With<Content>)>>()
            .iter(self.fixture.app.world())
            .collect::<Vec<_>>();
        self.fixture.navigate(Action::Push, "/broken", true);
        assert!(
            old.into_iter()
                .all(|entity| self.fixture.app.world().get_entity(entity).is_err())
        );
        assert_eq!(
            self.fixture
                .app
                .world_mut()
                .query_filtered::<Entity, Or<(With<Parent>, With<Page>, With<Content>)>>()
                .iter(self.fixture.app.world())
                .count(),
            0
        );
        assert!(
            self.fixture
                .app
                .world()
                .get_entity(self.fixture.host)
                .is_ok()
        );
    }
}

fn report(
    name: &str,
    content: usize,
    mode: Mode,
    values: &[Sample],
    census: &Census,
    parents: usize,
    pages: usize,
) {
    println!(
        "{name},{content},{content},{},{},{},{},{},{parents},{pages}",
        if mode == Mode::Disabled { 0 } else { 2 },
        timing_columns(values),
        census.live,
        census.parents.iter().filter(|(_, active)| !active).count(),
        census.pages.iter().filter(|(_, active)| !active).count()
    );
}

pub(super) fn run(shape: ContentShape, samples: usize) {
    println!(
        "scenario,parent_content_children,child_content_children,capacity,samples,mean_us,p50_us,p95_us,max_us,min_updates,max_updates,live_entities,inactive_parents,inactive_children,parent_factory_builds,child_factory_builds"
    );
    for content in [1, 32, 128] {
        for (name, mode, paths) in [
            ("disabled", Mode::Disabled, &[("q", "b"), ("p", "a")][..]),
            (
                "parent_fresh_child",
                Mode::Parent,
                &[("q", "b"), ("p", "a")][..],
            ),
            (
                "parent_child_hit",
                Mode::Both,
                &[("q", "b"), ("p", "a")][..],
            ),
            (
                "dependency_eviction",
                Mode::Eviction,
                &[("q", "a"), ("r", "a"), ("p", "a")][..],
            ),
        ] {
            let mut instances = Instances::new(content, shape, mode);
            let mut values = Vec::with_capacity(samples);
            let mut parent_builds = 0;
            let mut page_builds = 0;
            let mut stable_live = None;
            for index in 0..INSTANCE_WARMUP + samples {
                let previous = instances.census(
                    if index == 0 {
                        "p"
                    } else {
                        paths[(index - 1) % paths.len()].0
                    },
                    if index == 0 {
                        "a"
                    } else {
                        paths[(index - 1) % paths.len()].1
                    },
                );
                for (entity, _) in &previous.parents {
                    instances
                        .fixture
                        .app
                        .world_mut()
                        .get_mut::<Parent>(*entity)
                        .unwrap()
                        .1 = 73;
                }
                let (section, id) = paths[index % paths.len()];
                let before_parents = instances.parent_builds.load(Ordering::Relaxed);
                let before_pages = instances.fixture.factory_count();
                let sample =
                    instances
                        .fixture
                        .navigate(Action::Replace, &format!("/{section}/{id}"), false);
                let built_parents =
                    instances.parent_builds.load(Ordering::Relaxed) - before_parents;
                let built_pages = instances.fixture.factory_count() - before_pages;
                let census = instances.census(section, id);
                let parent = census.parents.iter().find(|(_, active)| *active).unwrap().0;
                let page = census.pages.iter().find(|(_, active)| *active).unwrap().0;
                if index > 0 && matches!(mode, Mode::Parent | Mode::Both) {
                    assert_eq!(built_parents, 0);
                    assert!(previous.parents.iter().any(|(entity, _)| *entity == parent));
                    assert_eq!(
                        instances
                            .fixture
                            .app
                            .world()
                            .get::<Parent>(parent)
                            .unwrap()
                            .1,
                        73
                    );
                } else {
                    assert_eq!(built_parents, 1);
                    assert!(!previous.parents.iter().any(|(entity, _)| *entity == parent));
                }
                if index > 0 && mode == Mode::Both {
                    assert_eq!(built_pages, 0);
                    assert!(previous.pages.iter().any(|(entity, _)| *entity == page));
                } else {
                    assert_eq!(built_pages, 1);
                    assert!(!previous.pages.iter().any(|(entity, _)| *entity == page));
                }
                if mode == Mode::Disabled || (mode == Mode::Eviction && index > 0) {
                    let disappeared_parents = previous
                        .parents
                        .iter()
                        .filter(|(entity, _)| {
                            instances.fixture.app.world().get_entity(*entity).is_err()
                        })
                        .count();
                    let disappeared_pages = previous
                        .pages
                        .iter()
                        .filter(|(entity, _)| {
                            instances.fixture.app.world().get_entity(*entity).is_err()
                        })
                        .count();
                    assert_eq!(disappeared_parents, 1);
                    assert_eq!(disappeared_pages, 1);
                }
                if index >= INSTANCE_WARMUP {
                    assert_eq!(instances.fixture.retained(), 64);
                    assert_eq!(census.live, *stable_live.get_or_insert(census.live));
                    values.push(sample);
                    parent_builds += built_parents;
                    page_builds += built_pages;
                }
            }
            let (section, id) = paths[(INSTANCE_WARMUP + samples - 1) % paths.len()];
            let census = instances.census(section, id);
            report(
                &format!("{name}_replace"),
                content,
                mode,
                &values,
                &census,
                parent_builds,
                page_builds,
            );
            let before_parents = instances.parent_builds.load(Ordering::Relaxed);
            let before_pages = instances.fixture.factory_count();
            let values = idle(&mut instances.fixture.app, samples);
            let after = instances.census(section, id);
            assert_eq!(after.live, census.live);
            assert_eq!(
                instances.parent_builds.load(Ordering::Relaxed),
                before_parents
            );
            assert_eq!(instances.fixture.factory_count(), before_pages);
            report(
                &format!("{name}_idle"),
                content,
                mode,
                &values,
                &after,
                0,
                0,
            );
            instances.release();
        }
    }
}
