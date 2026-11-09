use super::*;
use std::sync::atomic::AtomicUsize;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetApp, AssetPlugin, Handle};
use bevy_ecs::{
    component::Component,
    hierarchy::Children,
    template::{FnTemplate, TemplateContext},
};
use bevy_internal::image::Image;
use bevy_scene::{
    ResolveContext, ResolvedScene, SceneDependencies, SceneFunction, ScenePlugin, bsn,
};
use bevy_ui::{Node, Val};
use bevy_ui_router_bsn::DefaultOutlet;
use bevy_ui_router_core::{
    integration::{EngineAction, RouterEngine},
    prelude::{RouteLocation, RouteNodeOptions, RoutePattern, RouteTree, RouteViewKey},
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
    ));
    app
}

fn input() -> RouteViewInput {
    let view = RouteViewKey::new("native").unwrap();
    let routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(view.clone()),
        )
        .unwrap()
        .build()
        .unwrap();
    let mut engine = RouterEngine::install(routes, view).unwrap();
    let token = engine.request_host_admission().unwrap();
    let grant = engine.grant_host_admission(&token).unwrap();
    engine.record_top_slot_registered(&grant).unwrap();
    engine.confirm_host(grant);
    engine
        .push(
            RouteLocation::new("/")
                .with_search("?tab=details")
                .with_fragment("#body"),
        )
        .unwrap();
    engine.collect();
    let EngineAction::FrameTask(mut task) = engine.action().unwrap() else {
        panic!("root push must produce a real frame task")
    };
    task.take_input()
}

#[derive(Component, Default, Clone, Debug, PartialEq)]
struct ContentValue(usize);

#[derive(Component, Clone)]
struct ContentImage(Handle<Image>);

#[test]
fn each_preparation_calls_the_factory_once_and_consumes_a_fresh_scene() {
    let mut app = app();
    let builds = Arc::new(AtomicUsize::new(0));
    let resolves = Arc::new(AtomicUsize::new(0));
    let mut factory: BsnSceneFactory = Box::new({
        let builds = builds.clone();
        let resolves = resolves.clone();
        move |input| {
            assert_eq!(input.location().path(), "/");
            assert_eq!(input.raw_search(), Some("?tab=details"));
            assert_eq!(input.fragment(), Some("#body"));
            let value = builds.fetch_add(1, Ordering::SeqCst) + 1;
            let resolves = resolves.clone();
            Ok(Box::new(SceneFunction(move |_, scene| {
                resolves.fetch_add(1, Ordering::SeqCst);
                scene.push_template(ContentValue(value));
            })))
        }
    });
    let input = input();
    for value in 1..=2 {
        let before = app.world().entities().len();
        let prepared = prepare_bsn_view(app.world(), &mut factory, &input).unwrap();
        assert_eq!(
            app.world().entities().len(),
            before,
            "preparation must not allocate UI"
        );
        assert_eq!(builds.load(Ordering::SeqCst), value);
        assert_eq!(resolves.load(Ordering::SeqCst), value);
        // Payload compatibility, not Router publication: only this unit test owns
        // these entities. Production must use registered obligations + wrapper.
        let entity = prepared.root.spawn(app.world_mut()).unwrap().id();
        assert_eq!(
            app.world().get::<ContentValue>(entity),
            Some(&ContentValue(value))
        );
    }
}

#[test]
fn native_templates_and_internal_outlet_survive_graph_preparation() {
    let mut app = app();
    let scene = bsn! {
        Node { width: Val::Px(91.0) }
        ContentValue(4)
        Children [
            Node { width: Val::Px(13.0) } ContentValue(5)
            --
            Node { flex_grow: 1.0 } DefaultOutlet ContentValue(6)
        ]
    };
    let prepared = prepare_bsn_scene(app.world(), Box::new(scene)).unwrap();
    assert_eq!(
        prepared
            .declarations
            .iter()
            .map(|node| node.parent)
            .collect::<Vec<_>>(),
        [None, Some(0), Some(0)]
    );
    let root = prepared.root.spawn(app.world_mut()).unwrap().id();
    assert_eq!(app.world().get::<Node>(root).unwrap().width, Val::Px(91.0));
    let children = app.world().get::<Children>(root).unwrap();
    assert_eq!(children.len(), 2);
    assert_eq!(
        app.world().get::<ContentValue>(children[0]),
        Some(&ContentValue(5))
    );
    assert_eq!(
        app.world().get::<Node>(children[0]).unwrap().width,
        Val::Px(13.0)
    );
    assert!(app.world().get::<DefaultOutlet>(children[0]).is_none());
    assert!(app.world().get::<DefaultOutlet>(children[1]).is_some());
    assert_eq!(app.world().get::<Node>(children[1]).unwrap().flex_grow, 1.0);
    assert!(app.world().get::<Children>(children[1]).unwrap().is_empty());
}

#[test]
fn runtime_removal_rejects_before_factory_or_scene_execution() {
    let input = input();
    for missing in [0, 1, 2] {
        let mut app = app();
        if missing != 1 {
            app.world_mut().remove_resource::<AssetServer>();
        }
        if missing != 0 {
            app.world_mut().remove_resource::<Assets<ScenePatch>>();
        }
        let before = app.world().entities().len();
        let mut factory: BsnSceneFactory = Box::new(|_| panic!("runtime check must precede build"));
        assert!(matches!(
            prepare_bsn_view(app.world(), &mut factory, &input),
            Err(BsnPreparationFailure {
                error: BsnPreparationError::MissingSceneRuntime,
                ..
            })
        ));
        assert_eq!(app.world().entities().len(), before);
    }
}

#[test]
fn build_and_official_resolve_failures_keep_distinct_sources_without_ui_allocation() {
    use std::error::Error;
    let app = app();
    let before = app.world().entities().len();
    let mut factory: BsnSceneFactory =
        Box::new(|_| Err(BsnBuildError::new("application build failed")));
    let failure = prepare_bsn_view(app.world(), &mut factory, &input())
        .err()
        .unwrap();
    assert!(failure.dependencies.is_empty());
    let error = failure.error;
    assert!(matches!(error, BsnPreparationError::Build(_)));
    assert_eq!(
        error.source().unwrap().to_string(),
        "application build failed"
    );

    struct Unresolvable;
    impl Scene for Unresolvable {
        fn resolve(
            self,
            _: &mut ResolveContext,
            _: &mut ResolvedScene,
        ) -> Result<(), ResolveSceneError> {
            Err(ResolveSceneError::MissingSceneDependency(
                "required.scene".into(),
            ))
        }
    }
    let error = prepare_bsn_scene(app.world(), Box::new(Unresolvable))
        .err()
        .unwrap()
        .error;
    assert!(
        matches!(&error, BsnPreparationError::Resolve(ResolveSceneError::MissingSceneDependency(path)) if path.to_string() == "required.scene")
    );
    assert!(error.source().is_some());
    assert_eq!(app.world().entities().len(), before);
}

#[test]
fn cached_inclusion_is_rejected_even_when_loaded_and_below_the_root() {
    for nested in [false, true] {
        let mut app = app();
        let cached = app
            .world_mut()
            .resource_mut::<Assets<ScenePatch>>()
            .add(ScenePatch {
                scene: None,
                dependencies: Vec::new(),
                resolved: Some(Arc::new(ResolvedSceneRoot {
                    scene: ResolvedScene::default(),
                })),
            });
        let cached_id = cached.id().untyped();
        let before = app.world().entities().len();
        let scene = SceneFunction(move |_, scene| {
            if nested {
                let children = scene.get_or_insert_related_resolved_scenes::<ChildOf>();
                children.scenes.push(ResolvedScene::default());
                children.scenes[0].include_cached(cached).unwrap();
            } else {
                scene.include_cached(cached).unwrap();
            }
        });
        let error = prepare_bsn_scene(app.world(), Box::new(scene))
            .err()
            .unwrap()
            .error;
        assert!(
            matches!(error, BsnPreparationError::CachedInclusion(CachedSceneError::MultipleCached { id, .. }) if id == cached_id)
        );
        assert_eq!(app.world().entities().len(), before);
    }
}

#[test]
fn duplicate_references_are_rejected_within_and_across_declarations() {
    let app = app();
    for across_nodes in [false, true] {
        let reference = SceneEntityReference::new(("application", 1, 1), 0, 1);
        let scene = SceneFunction(move |_, scene| {
            scene.entity_references.push(reference);
            if across_nodes {
                let mut child = ResolvedScene::default();
                child.entity_references.push(reference);
                scene
                    .get_or_insert_related_resolved_scenes::<ChildOf>()
                    .scenes
                    .push(child);
            } else {
                scene.entity_references.push(reference);
            }
        });
        assert!(
            matches!(prepare_bsn_scene(app.world(), Box::new(scene)), Err(BsnPreparationFailure { error: BsnPreparationError::DuplicateReference(found), .. }) if found == reference)
        );
    }
}

#[test]
fn references_preserve_aliases_and_anonymous_nodes_are_attempt_unique() {
    let app = app();
    let aliases = [
        SceneEntityReference::new(("application", 1, 1), 0, 1),
        SceneEntityReference::new(("application", 1, 1), 1, 1),
    ];
    let mut all_anonymous = HashSet::new();
    for _ in 0..2 {
        let scene = SceneFunction(move |_, scene| {
            // Scene implementations may replace the entire resolve target.
            *scene = ResolvedScene::default();
            scene.entity_references.extend(aliases);
            let mut child = ResolvedScene::default();
            child
                .get_or_insert_related_resolved_scenes::<ChildOf>()
                .scenes
                .push(ResolvedScene::default());
            scene
                .get_or_insert_related_resolved_scenes::<ChildOf>()
                .scenes
                .extend([child, ResolvedScene::default()]);
        });
        let prepared = prepare_bsn_scene(app.world(), Box::new(scene)).unwrap();
        assert_eq!(
            prepared
                .declarations
                .iter()
                .map(|node| node.parent)
                .collect::<Vec<_>>(),
            [None, Some(0), Some(1), Some(0)]
        );
        assert_eq!(prepared.declarations[0].references, aliases);
        assert_eq!(prepared.root.scene.entity_references, aliases);
        for node in &prepared.declarations[1..] {
            assert_eq!(node.references.len(), 1);
            assert!(all_anonymous.insert(node.references[0]));
        }
    }
    assert_eq!(all_anonymous.len(), 6);
}

#[test]
fn dependency_handles_survive_resolve_without_blanket_loaded_asset_rejection() {
    struct WithDependency;
    impl Scene for WithDependency {
        fn register_dependencies(&self, dependencies: &mut SceneDependencies) {
            dependencies.register::<Image>("unused.png".into());
        }
        fn resolve(
            self,
            context: &mut ResolveContext,
            scene: &mut ResolvedScene,
        ) -> Result<(), ResolveSceneError> {
            let image = context.assets.load("unused.png");
            scene.push_template(FnTemplate(move |_: &mut TemplateContext| {
                Ok(ContentImage(image.clone()))
            }));
            Ok(())
        }
    }
    let mut app = app();
    app.init_asset::<Image>();
    let prepared = prepare_bsn_scene(app.world(), Box::new(WithDependency)).unwrap();
    assert_eq!(prepared.dependencies.len(), 1);
    assert_eq!(
        prepared.dependencies[0].path().unwrap().to_string(),
        "unused.png"
    );
    assert!(matches!(prepared.dependencies[0], UntypedHandle::Strong(_)));
    let dependency = prepared.dependencies[0].clone().typed::<Image>();
    // This preparation-level consumer checks the authored handle survives the
    // adapter boundary; Router allocation/adoption belongs to native creation.
    let root = prepared.root.spawn(app.world_mut()).unwrap().id();
    assert_eq!(
        app.world().get::<ContentImage>(root).unwrap().0.id(),
        dependency.id()
    );
}

#[test]
fn resolve_and_graph_rejections_return_dependency_lifetimes_for_settlement() {
    struct FailsWithDependency {
        during_resolve: bool,
    }
    impl Scene for FailsWithDependency {
        fn register_dependencies(&self, dependencies: &mut SceneDependencies) {
            dependencies.register::<Image>("failed-attempt.png".into());
        }

        fn resolve(
            self,
            _: &mut ResolveContext,
            scene: &mut ResolvedScene,
        ) -> Result<(), ResolveSceneError> {
            if self.during_resolve {
                return Err(ResolveSceneError::MissingSceneDependency(
                    "failed-attempt.png".into(),
                ));
            }
            let duplicate = SceneEntityReference::new(("application", 1, 1), 0, 1);
            scene.entity_references.extend([duplicate, duplicate]);
            Ok(())
        }
    }
    let mut app = app();
    app.init_asset::<Image>();
    for during_resolve in [true, false] {
        let failure = prepare_bsn_scene(
            app.world(),
            Box::new(FailsWithDependency { during_resolve }),
        )
        .err()
        .unwrap();
        if during_resolve {
            assert!(matches!(failure.error, BsnPreparationError::Resolve(_)));
        } else {
            assert!(matches!(
                failure.error,
                BsnPreparationError::DuplicateReference(_)
            ));
        }
        assert_eq!(failure.dependencies.len(), 1);
        assert!(matches!(failure.dependencies[0], UntypedHandle::Strong(_)));
        assert_eq!(
            failure.dependencies[0].type_id(),
            std::any::TypeId::of::<Image>()
        );
        assert_eq!(
            failure.dependencies[0].path().unwrap().to_string(),
            "failed-attempt.png"
        );
    }
}

#[test]
fn exhausted_attempt_identity_never_wraps_or_reuses_a_reference_namespace() {
    let counter = AtomicU64::new(u64::MAX - 1);
    assert_eq!(next_scene_attempt(&counter).unwrap(), u64::MAX);
    for _ in 0..2 {
        assert!(matches!(
            next_scene_attempt(&counter),
            Err(BsnPreparationError::AttemptExhausted)
        ));
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }
}
