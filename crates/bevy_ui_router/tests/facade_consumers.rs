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

impl Consumer {
    fn new(
        name: &str,
        defaults: bool,
        features: &[&str],
        ecs: bool,
        source: &str,
        crlf: bool,
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
        assert!(manifest_violations(&toml::from_str(&manifest).unwrap()).is_empty());
        fs::create_dir_all(directory.join("src")).unwrap();
        let line_endings = |text: &str| {
            let lf = text.replace("\r\n", "\n");
            if crlf { lf.replace('\n', "\r\n") } else { lf }
        };
        fs::write(directory.join("Cargo.toml"), line_endings(&manifest)).unwrap();
        fs::write(directory.join("src/main.rs"), line_endings(source)).unwrap();
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
    // Each positive consumer uses real production paths. The default consumer runs
    // unchanged with both line endings, including its manifest and Rust source.
    for (name, defaults, features, ecs, source, crlf) in [
        (
            "core",
            false,
            &[][..],
            false,
            include_str!("consumers/core.rs"),
            false,
        ),
        (
            "default-lf",
            true,
            &[][..],
            false,
            include_str!("consumers/view.rs"),
            false,
        ),
        (
            "default-crlf",
            true,
            &[][..],
            false,
            include_str!("consumers/view.rs"),
            true,
        ),
        (
            "view",
            false,
            &["view"][..],
            false,
            include_str!("consumers/view.rs"),
            false,
        ),
        (
            "ecs",
            false,
            &["ecs"][..],
            true,
            include_str!("consumers/ecs.rs"),
            false,
        ),
        (
            "bsn-native-lf",
            false,
            &["bsn"][..],
            true,
            include_str!("consumers/bsn.rs"),
            false,
        ),
        (
            "bsn-native-crlf",
            false,
            &["bsn"][..],
            true,
            include_str!("consumers/bsn.rs"),
            true,
        ),
    ] {
        Consumer::new(name, defaults, features, ecs, source, crlf).run(!ecs);
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
        // Temporary lifecycle API: remove this probe when the old session exits in phase C/E.
        (
            "session-double-consumption",
            "use bevy_ui_router::prelude::EcsViewReportSession; fn probe(session: EcsViewReportSession<'_>) { let _ = session.applied(); let _ = session.applied(); } fn main() {}",
            "E0382",
            "session",
        ),
    ] {
        Consumer::new(name, false, &["ecs"], false, source, false).rejects(code, symbol);
    }
}
