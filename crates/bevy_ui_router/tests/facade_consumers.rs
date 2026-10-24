#[allow(dead_code)]
#[path = "support/dependency_policy.rs"]
mod dependency_policy;

use dependency_policy::{is_bevy_package, lock_violations, manifest_violations};
use serde_json::Value as Json;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};
use toml::Value;

struct Consumer {
    directory: PathBuf,
    target: PathBuf,
}

impl Drop for Consumer {
    fn drop(&mut self) {
        // Only this invocation's manifest/source/lock; retain the shared build cache.
        let _ = fs::remove_dir_all(&self.directory);
    }
}

impl Consumer {
    fn new(
        name: &str,
        defaults: bool,
        features: &[&str],
        ecs: bool,
        source: &str,
        native_ui: bool,
    ) -> Self {
        let facade = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let base = facade
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("target/facade-consumers");
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = base.join(format!("{}-{stamp}-{name}", std::process::id()));
        let path = Value::String(facade.to_str().expect("UTF-8 workspace path").to_owned());
        let bsn = features.contains(&"bsn");
        let desktop_back = features.contains(&"desktop-back");
        let features = Value::Array(
            features
                .iter()
                .map(|feature| Value::String((*feature).into()))
                .collect(),
        );
        let mut manifest = format!(
            "[package]\nname = 'facade-consumer-{name}'\nversion = '0.0.0'\nedition = '2024'\n\n[workspace]\nresolver = '3'\n\n[dependencies]\nbevy_ui_router = {{ path = {path}, default-features = {defaults}, features = {features} }}\n",
        );
        if ecs {
            for dependency in ["bevy_app", "bevy_ecs", "bevy_ui"] {
                manifest.push_str(&format!(
                    "{dependency} = {{ version = '=0.19.1', default-features = false }}\n"
                ));
            }
        }
        if bsn {
            for dependency in ["bevy_asset", "bevy_scene"] {
                manifest.push_str(&format!(
                    "{dependency} = {{ version = '=0.19.1', default-features = false }}\n"
                ));
            }
        }
        if desktop_back {
            if !ecs {
                for dependency in ["bevy_app", "bevy_ecs"] {
                    manifest.push_str(&format!(
                        "{dependency} = {{ version = '=0.19.1', default-features = false }}\n"
                    ));
                }
            }
            manifest.push_str(
                "bevy_input = { version = '=0.19.1', default-features = false, features = ['mouse'] }\n",
            );
        }
        if native_ui {
            manifest.push_str(
                "bevy_input_focus = { version = '=0.19.1', default-features = false }\n\
                 bevy_internal = { version = '=0.19.1', default-features = false, features = ['bevy_window', 'bevy_camera', 'bevy_ui', 'bevy_picking'] }\n",
            );
        }
        assert!(manifest_violations(&toml::from_str(&manifest).unwrap()).is_empty());
        fs::create_dir_all(directory.join("src")).unwrap();
        fs::write(directory.join("Cargo.toml"), manifest).unwrap();
        fs::write(directory.join("src/main.rs"), source).unwrap();
        Self {
            directory,
            target: base.join("cargo-target"),
        }
    }

    fn cargo(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO"))
            .current_dir(&self.directory)
            .env("CARGO_TARGET_DIR", &self.target)
            .args(args)
            .output()
            .expect("consumer Cargo command runs")
    }

    fn success(&self, args: &[&str]) -> Output {
        let output = self.cargo(args);
        assert!(
            output.status.success(),
            "{} {args:?}:\n{}\n{}",
            self.directory.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn prepare(&self) {
        self.success(&["generate-lockfile", "--offline"]);
        let lock = fs::read_to_string(self.directory.join("Cargo.lock")).unwrap();
        assert_eq!(
            lock_violations(&toml::from_str(&lock).unwrap()),
            Vec::<String>::new()
        );
    }

    fn run(&self, renderer_neutral: bool) {
        self.prepare();
        let mut tree_args = vec![
            "tree",
            "--offline",
            "--locked",
            "--edges",
            "normal,build",
            "--no-dedupe",
            "--prefix",
            "none",
            "--format",
            "{p}|{f}",
        ];
        // Cross-target neutrality matters for Core/View. An ECS run only needs
        // its host graph; its lock is still checked for every target's Bevy packages.
        if renderer_neutral {
            tree_args.extend(["--target", "all"]);
        }
        let output = self.success(&tree_args);
        let tree = String::from_utf8(output.stdout).unwrap();
        assert!(!tree.trim().is_empty());
        for line in tree.lines() {
            let name = line.split_whitespace().next().unwrap();
            if renderer_neutral {
                assert!(
                    !is_bevy_package(name)
                        && !matches!(name, "bevy_ui_router_ecs" | "bevy_ui_router_bsn"),
                    "unexpected renderer dependency: {line}"
                );
            }
            let (_, features) = line.split_once('|').expect("Cargo feature graph");
            assert!(
                !features
                    .split(',')
                    .any(|feature| matches!(feature, "test-support" | "ecs-test-support")),
                "test support leaked into the independent consumer: {line}"
            );
        }
        self.success(&["run", "--offline", "--locked", "--quiet"]);
    }

    fn rejects(&self, code: &str, symbol: &str) {
        self.prepare();
        let output = self.cargo(&["check", "--offline", "--locked", "--message-format=json"]);
        assert!(
            !output.status.success(),
            "{} unexpectedly compiled",
            self.directory.display()
        );
        let messages = String::from_utf8(output.stdout).unwrap();
        let errors = messages
            .lines()
            .filter_map(|line| serde_json::from_str::<Json>(line).ok())
            .filter(|message| {
                message["reason"] == "compiler-message" && message["message"]["level"] == "error"
            })
            .collect::<Vec<_>>();
        assert_eq!(
            errors.len(),
            1,
            "expected one consumer rejection, not an infrastructure failure:\n{messages}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let error = &errors[0]["message"];
        assert_eq!(error["code"]["code"].as_str(), Some(code), "{error}");
        assert!(
            error["rendered"].as_str().unwrap().contains(symbol),
            "{error}"
        );
        let source = self.directory.join("src/main.rs");
        assert!(
            error["spans"].as_array().unwrap().iter().any(|span| {
                span["is_primary"] == true
                    && span["file_name"]
                        .as_str()
                        .is_some_and(|file| self.directory.join(file) == source)
            }),
            "rejection must come from this consumer's source: {error}"
        );
    }
}

#[test]
fn independent_facade_consumers_run_and_cannot_forge_authority() {
    // Each feature configuration exercises real production paths in an independent workspace.
    for (name, defaults, features, ecs, source) in [
        (
            "core",
            false,
            &[][..],
            false,
            include_str!("consumers/core.rs"),
        ),
        (
            "default",
            true,
            &[][..],
            false,
            include_str!("consumers/view.rs"),
        ),
        (
            "view",
            false,
            &["view"][..],
            false,
            include_str!("consumers/view.rs"),
        ),
        (
            "platform",
            false,
            &["platform"][..],
            false,
            "use bevy_ui_router::platform::prelude::BackIntent; fn main() { let _ = BackIntent::NavigateBack; }",
        ),
        (
            "ecs",
            false,
            &["ecs"][..],
            true,
            include_str!("consumers/ecs.rs"),
        ),
        (
            "bsn-native",
            false,
            &["bsn"][..],
            true,
            include_str!("consumers/bsn.rs"),
        ),
    ] {
        Consumer::new(name, defaults, features, ecs, source, false).run(!ecs);
    }

    Consumer::new(
        "desktop-back",
        false,
        &["desktop-back"],
        false,
        include_str!("consumers/desktop_back.rs"),
        false,
    )
    .run(false);

    // Compile and execute the actual recommended examples, outside the workspace
    // dev-feature graph. Keep explicit dependencies aligned with the README.
    for (name, source, native_ui) in [
        (
            "quick-start",
            include_str!("../examples/bsn_quick_start.rs"),
            false,
        ),
        ("nested", include_str!("../examples/bsn_nested.rs"), true),
    ] {
        Consumer::new(name, false, &["bsn"], true, source, native_ui).run(false);
    }

    // One rejection per consumer: another compiler error cannot mask a removed boundary.
    for (name, source, code, symbol) in [
        (
            "frame-forgery",
            "use bevy_ui_router::prelude::RouteFrameId; fn main() { let _ = RouteFrameId::new(1); }",
            "E0624",
            "new",
        ),
        (
            "generation-forgery",
            "use bevy_ui_router::prelude::RouteGeneration; fn main() { let _ = RouteGeneration::initial(); }",
            "E0624",
            "initial",
        ),
        (
            "handle-forgery",
            "use bevy_ui_router::prelude::RouterHandle; fn main() { let _ = RouterHandle { domain: 1, router: 1 }; }",
            "E0451",
            "RouterHandle",
        ),
        (
            "navigation-authority",
            "use bevy_ui_router::prelude::RouterNavigation; fn probe(value: &mut RouterNavigation) { value.entries.clear(); } fn main() {}",
            "E0616",
            "entries",
        ),
        (
            "direct-publication",
            "use bevy_ui_router::prelude::RouterSnapshots; fn main() { let _ = RouterSnapshots::publish; }",
            "E0624",
            "publish",
        ),
    ] {
        Consumer::new(name, false, &["ecs"], false, source, false).rejects(code, symbol);
    }

    // Disabled modules are compiler-enforced boundaries, not just absent Cargo targets.
    // The positive consumers above use each module with its owning feature enabled.
    for (name, defaults, features, source, symbol) in [
        (
            "core-without-view",
            false,
            &[][..],
            "use bevy_ui_router::view::RouteViewRegistry; fn main() {}",
            "view",
        ),
        (
            "default-without-ecs",
            true,
            &[][..],
            "use bevy_ui_router::ecs::RouterNavigation; fn main() {}",
            "ecs",
        ),
        (
            "ecs-without-bsn",
            false,
            &["ecs"][..],
            "use bevy_ui_router::bsn::AppBsnExt; fn main() {}",
            "bsn",
        ),
        (
            "core-without-engine-authority",
            false,
            &[][..],
            "use bevy_ui_router::core::integration::RouterEngine; fn main() {}",
            "integration",
        ),
        (
            "ecs-without-registry-authority",
            false,
            &["ecs"][..],
            "use bevy_ui_router::ecs::RouteEntityRegistry; fn main() {}",
            "RouteEntityRegistry",
        ),
    ] {
        Consumer::new(name, defaults, features, false, source, false).rejects("E0432", symbol);
    }
}
