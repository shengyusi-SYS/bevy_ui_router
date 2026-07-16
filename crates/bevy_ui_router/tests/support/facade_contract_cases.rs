#![allow(dead_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TableRow(pub Vec<String>);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SymbolRow {
    pub symbol: String,
    pub module: String,
    pub prelude: bool,
    pub feature: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct GoldenRow {
    pub feature: String,
    pub path: String,
    pub owner: String,
    pub kind: String,
    pub api: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FacadeExport {
    module: String,
    symbol: String,
    feature: String,
    source: String,
}

pub fn facade_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn workspace_dir() -> PathBuf {
    facade_dir()
        .parent()
        .and_then(Path::parent)
        .expect("facade crate is nested under the workspace crates directory")
        .to_path_buf()
}

pub fn read_workspace(path: &str) -> String {
    fs::read_to_string(workspace_dir().join(path))
        .unwrap_or_else(|error| panic!("failed to read workspace file `{path}`: {error}"))
}

pub fn markdown_table(input: &str, heading: &str, expected_header: &[&str]) -> Vec<TableRow> {
    let lines = input.lines().collect::<Vec<_>>();
    let heading_index = lines
        .iter()
        .position(|line| line.trim() == heading)
        .unwrap_or_else(|| panic!("missing heading `{heading}`"));
    let header_index = (heading_index + 1..lines.len())
        .find(|index| lines[*index].trim_start().starts_with('|'))
        .unwrap_or_else(|| panic!("missing table after `{heading}`"));
    let header = parse_pipe_row(lines[header_index]);
    assert_eq!(
        header,
        expected_header
            .iter()
            .map(|cell| (*cell).to_owned())
            .collect::<Vec<_>>(),
        "table `{heading}` header drifted"
    );
    let separator = parse_pipe_row(lines[header_index + 1]);
    assert_eq!(
        separator.len(),
        header.len(),
        "table separator width drifted"
    );
    assert!(
        separator.iter().all(|cell| cell
            .chars()
            .all(|character| character == '-' || character == ':')),
        "table `{heading}` has an invalid separator"
    );

    let mut rows = Vec::new();
    for line in &lines[header_index + 2..] {
        if !line.trim_start().starts_with('|') {
            break;
        }
        let cells = parse_pipe_row(line);
        assert_eq!(
            cells.len(),
            header.len(),
            "table `{heading}` row width drifted"
        );
        rows.push(TableRow(cells));
    }
    assert!(!rows.is_empty(), "table `{heading}` must not be empty");
    let unique = rows.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(unique.len(), rows.len(), "table `{heading}` has duplicates");
    rows
}

pub fn markdown_table_by_header(
    input: &str,
    expected_header: &[&str],
    occurrence: usize,
) -> Vec<TableRow> {
    let expected = expected_header
        .iter()
        .map(|cell| (*cell).to_owned())
        .collect::<Vec<_>>();
    let lines = input.lines().collect::<Vec<_>>();
    let header_indices = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (parse_pipe_row(line) == expected).then_some(index))
        .collect::<Vec<_>>();
    let header_index = *header_indices
        .get(occurrence)
        .unwrap_or_else(|| panic!("missing table header occurrence {occurrence}: {expected:?}"));
    let separator = parse_pipe_row(lines[header_index + 1]);
    assert_eq!(separator.len(), expected.len());
    let mut rows = Vec::new();
    for line in &lines[header_index + 2..] {
        if !line.trim_start().starts_with('|') {
            break;
        }
        let cells = parse_pipe_row(line);
        assert_eq!(cells.len(), expected.len());
        rows.push(TableRow(cells));
    }
    assert!(!rows.is_empty(), "controlled table must not be empty");
    assert_eq!(
        rows.iter().cloned().collect::<BTreeSet<_>>().len(),
        rows.len(),
        "controlled table must not contain duplicates"
    );
    rows
}

fn parse_pipe_row(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().trim_matches('`').to_owned())
        .collect()
}

pub fn root_symbols(symbols: &[TableRow], exclusions: &[TableRow]) -> Vec<SymbolRow> {
    validate_exclusion_vocabulary(exclusions);
    let excluded = exclusions
        .iter()
        .filter(|row| row.0[0] == "symbol")
        .map(|row| row.0[1].clone())
        .collect::<BTreeSet<_>>();
    for path in &excluded {
        assert_eq!(
            symbols
                .iter()
                .filter(|row| format!("bevy_ui_router::{}::{}", row.0[1], row.0[0]) == *path)
                .count(),
            1,
            "symbol exclusion `{path}` must match exactly one allowlist row"
        );
    }
    let rows = symbols
        .iter()
        .map(|row| {
            assert!(matches!(row.0[2].as_str(), "yes" | "no"));
            SymbolRow {
                symbol: row.0[0].clone(),
                module: row.0[1].clone(),
                prelude: row.0[2] == "yes",
                feature: row.0[3].trim_matches('`').to_owned(),
            }
        })
        .filter(|row| {
            !excluded.contains(&format!("bevy_ui_router::{}::{}", row.module, row.symbol))
        })
        .collect::<Vec<_>>();
    let unique = rows.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        unique.len(),
        rows.len(),
        "allowed symbol rows must be unique"
    );
    rows
}

pub fn parse_golden(input: &str, exclusions: &[TableRow]) -> Vec<GoldenRow> {
    validate_exclusion_vocabulary(exclusions);
    let header = input.lines().take(4).collect::<Vec<_>>();
    assert_eq!(
        header,
        vec![
            "# bevy-ui-router-facade-associated-api-v1",
            "# columns: feature | facade canonical path | owner crate | item kind | normalized public API",
            "# sort: feature-rank(default,view,ecs,ecs-test-support,platform), canonical-path ASCII, source/fixture declaration order; compile-assert entries follow their associated source contract",
            "# normalization: Self names the canonical-path type; lifetimes are normalized to 'a; impl Trait syntax is preserved; omitted field records mean zero public fields; compile-assert records are compiler-verified requirements rather than source-enumerated impls; the sole allowed public unit/zero-field struct is bevy_ui_router::ecs::BevyUiRouterPlugin",
        ],
        "associated API golden metadata drifted"
    );
    let excluded_symbols = exclusions
        .iter()
        .filter(|row| row.0[0] == "symbol")
        .map(|row| row.0[1].clone())
        .collect::<BTreeSet<_>>();
    let excluded_associated = exclusions
        .iter()
        .filter(|row| row.0[0] == "associated")
        .map(|row| row.0[1].clone())
        .collect::<BTreeSet<_>>();
    assert!(
        exclusions
            .iter()
            .all(|row| matches!(row.0[0].as_str(), "symbol" | "associated"))
    );

    let mut raw_rows = Vec::new();
    for (line_number, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split(" | ").collect::<Vec<_>>();
        assert_eq!(
            fields.len(),
            5,
            "golden line {} must have exactly five columns",
            line_number + 1
        );
        assert!(matches!(
            fields[0],
            "default" | "view" | "ecs" | "ecs-test-support" | "platform"
        ));
        assert!(matches!(
            fields[2],
            "bevy_ui_router_core" | "bevy_ui_router_view" | "bevy_ui_router_ecs"
        ));
        assert!(matches!(
            fields[3],
            "struct"
                | "enum"
                | "trait"
                | "variant"
                | "method"
                | "trait-method"
                | "trait-impl"
                | "compile-assert"
        ));
        let row = GoldenRow {
            feature: fields[0].to_owned(),
            path: fields[1].to_owned(),
            owner: fields[2].to_owned(),
            kind: fields[3].to_owned(),
            api: fields[4].to_owned(),
        };
        let associated_path = method_name(&row).map(|method| format!("{}::{method}", row.path));
        raw_rows.push((row, associated_path));
    }
    let feature_rank = |feature: &str| match feature {
        "default" => 0,
        "view" => 1,
        "ecs" => 2,
        "ecs-test-support" => 3,
        "platform" => 4,
        _ => unreachable!(),
    };
    for pair in raw_rows.windows(2) {
        let before = &pair[0].0;
        let after = &pair[1].0;
        assert!(
            (feature_rank(&before.feature), &before.path)
                <= (feature_rank(&after.feature), &after.path),
            "associated API golden sort drifted between '{}' and '{}'",
            before.path,
            after.path
        );
    }
    let kind_rank = |kind: &str| match kind {
        "struct" | "enum" | "trait" => 0,
        "variant" => 1,
        "method" | "trait-method" => 2,
        "trait-impl" => 3,
        "compile-assert" => 4,
        other => panic!("unknown golden order kind: {other}"),
    };
    let mut offset = 0;
    while offset < raw_rows.len() {
        let feature = &raw_rows[offset].0.feature;
        let path = &raw_rows[offset].0.path;
        let end = (offset + 1..raw_rows.len())
            .find(|index| {
                raw_rows[*index].0.feature != *feature || raw_rows[*index].0.path != *path
            })
            .unwrap_or(raw_rows.len());
        let group = &raw_rows[offset..end];
        for pair in group.windows(2) {
            assert!(
                kind_rank(&pair[0].0.kind) <= kind_rank(&pair[1].0.kind),
                "golden declaration/fixture order drifted within {path}: {} before {}",
                pair[0].0.kind,
                pair[1].0.kind
            );
        }
        let trait_impls = group
            .iter()
            .filter(|(row, _)| row.kind == "trait-impl")
            .map(|(row, _)| row.api.as_str())
            .collect::<Vec<_>>();
        assert!(
            trait_impls.windows(2).all(|pair| pair[0] <= pair[1]),
            "trait-impl fixture order drifted within {path}"
        );
        let compile_asserts = group
            .iter()
            .filter(|(row, _)| row.kind == "compile-assert")
            .map(|(row, _)| row.api.as_str())
            .collect::<Vec<_>>();
        assert!(
            compile_asserts.is_empty()
                || compile_asserts == ["Self: Send", "Self: Sync", "Self: 'static"],
            "compile-assert placement/order drifted within {path}: {compile_asserts:?}"
        );
        offset = end;
    }
    let required_error_bounds = BTreeSet::from([
        "bevy_ui_router::core::RouteError",
        "bevy_ui_router::core::RouteViewKeyError",
        "bevy_ui_router::view::RouteViewRegistryError",
        "bevy_ui_router::view::ViewDriverKeyError",
        "bevy_ui_router::ecs::ContentPrimitiveError",
        "bevy_ui_router::ecs::ContentWriteError",
        "bevy_ui_router::ecs::NavigationEnqueueError",
        "bevy_ui_router::ecs::RouterInstallError",
        "bevy_ui_router::ecs::ViewDriverRegistrationError",
    ]);
    let bounded_paths = raw_rows
        .iter()
        .filter(|(row, _)| row.kind == "compile-assert")
        .map(|(row, _)| row.path.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        bounded_paths, required_error_bounds,
        "required error compile-assert type set drifted"
    );
    for path in &excluded_associated {
        assert_eq!(
            raw_rows
                .iter()
                .filter(|(_, associated)| associated.as_deref() == Some(path.as_str()))
                .count(),
            1,
            "associated exclusion `{path}` must match exactly one golden method"
        );
    }
    let rows = raw_rows
        .into_iter()
        .filter_map(|(row, associated_path)| {
            (!excluded_symbols.contains(&row.path)
                && !associated_path
                    .as_ref()
                    .is_some_and(|path| excluded_associated.contains(path)))
            .then_some(row)
        })
        .collect::<Vec<_>>();
    assert!(!rows.is_empty(), "golden must contain records");
    assert_eq!(
        rows.iter().cloned().collect::<BTreeSet<_>>().len(),
        rows.len(),
        "golden must not contain duplicate records"
    );
    rows
}

fn validate_exclusion_vocabulary(exclusions: &[TableRow]) {
    let slices = BTreeSet::from(["Authority closure", "History", "Nested push", "Recovery"]);
    for row in exclusions {
        assert_eq!(row.0.len(), 3, "exclusion row width drifted");
        assert!(matches!(row.0[0].as_str(), "symbol" | "associated"));
        assert!(
            row.0[1].starts_with("bevy_ui_router::"),
            "exclusion path is not canonical: {}",
            row.0[1]
        );
        assert!(
            slices.contains(row.0[2].as_str()),
            "unknown First slice vocabulary: {}",
            row.0[2]
        );
    }
    let actual = exclusions
        .iter()
        .map(|row| (row.0[0].as_str(), row.0[1].as_str(), row.0[2].as_str()))
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([
        (
            "symbol",
            "bevy_ui_router::ecs::AuthorityOutletMapping",
            "Nested push",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::InputProbeResult",
            "Authority closure",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::QuarantineLeaseId",
            "Authority closure",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::QuarantineOwnerKind",
            "Authority closure",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::QuarantineOwnerSnapshot",
            "Authority closure",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::QuarantineSnapshot",
            "Authority closure",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::RouterInputModality",
            "Authority closure",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::RouterInputProbes",
            "Authority closure",
        ),
        (
            "associated",
            "bevy_ui_router::ecs::FrameContentWriter::declare_outlet",
            "Nested push",
        ),
        (
            "associated",
            "bevy_ui_router::ecs::RouterNavigation::replace",
            "History",
        ),
        (
            "associated",
            "bevy_ui_router::ecs::RouterNavigation::back",
            "History",
        ),
        (
            "associated",
            "bevy_ui_router::core::RouteTreeBuilder::route_with_options",
            "Nested push",
        ),
        (
            "associated",
            "bevy_ui_router::core::RouteTreeBuilder::route",
            "Nested push",
        ),
        (
            "associated",
            "bevy_ui_router::core::RouteNodeOptions::with_error_boundary",
            "Recovery",
        ),
        (
            "associated",
            "bevy_ui_router::core::RouteNodeOptions::with_error_view",
            "Recovery",
        ),
        (
            "associated",
            "bevy_ui_router::ecs::RouterAuthorityAuditSnapshot::outlets",
            "Nested push",
        ),
        (
            "associated",
            "bevy_ui_router::ecs::RouterAuthorityAuditSnapshot::quarantines",
            "Authority closure",
        ),
    ]);
    assert_eq!(
        actual, expected,
        "Root profile exclusion path/slice mapping drifted"
    );
}

fn method_name(row: &GoldenRow) -> Option<String> {
    if !matches!(row.kind.as_str(), "method" | "trait-method") {
        return None;
    }
    let before_parameters = row.api.split('(').next()?;
    before_parameters
        .split_whitespace()
        .last()
        .map(|name| name.trim_start_matches("r#").to_owned())
}

pub fn assert_golden_covers_symbols(symbols: &[SymbolRow], golden: &[GoldenRow]) {
    let expected = symbols
        .iter()
        .map(|row| format!("bevy_ui_router::{}::{}", row.module, row.symbol))
        .collect::<BTreeSet<_>>();
    let actual = golden
        .iter()
        .map(|row| row.path.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected, "golden canonical type paths drifted");
    for row in golden {
        let symbol = symbols
            .iter()
            .find(|symbol| {
                row.path == format!("bevy_ui_router::{}::{}", symbol.module, symbol.symbol)
            })
            .expect("golden path was checked above");
        assert_eq!(
            row.feature, symbol.feature,
            "golden feature drifted for {}",
            row.path
        );
    }
}

pub fn assert_facade_exports(source: &str, symbols: &[SymbolRow]) {
    let file = syn::parse_file(source).expect("facade source must parse");
    let mut actual = BTreeSet::new();
    for item in &file.items {
        if let syn::Item::Mod(module) = item
            && matches!(module.vis, syn::Visibility::Public(_))
        {
            collect_module_exports(module, None, &module.ident.to_string(), &mut actual);
        }
    }

    let source_scopes = project_source_scopes();
    let public_modules = owner_public_modules();
    let mut expected = BTreeSet::new();
    for row in symbols {
        expected.insert(FacadeExport {
            module: row.module.clone(),
            symbol: row.symbol.clone(),
            feature: row.feature.clone(),
            source: owner_source_path(&row.module, &row.symbol, &source_scopes, &public_modules),
        });
        if row.prelude {
            expected.insert(FacadeExport {
                module: "prelude".to_owned(),
                symbol: row.symbol.clone(),
                feature: row.feature.clone(),
                source: format!("crate::{}::{}", row.module, row.symbol),
            });
        }
    }
    assert_eq!(actual, expected, "facade public re-export set drifted");
}

fn collect_module_exports(
    module: &syn::ItemMod,
    inherited_feature: Option<String>,
    module_path: &str,
    exports: &mut BTreeSet<FacadeExport>,
) {
    let module_feature = cfg_feature(&module.attrs).or(inherited_feature);
    let Some((_, items)) = &module.content else {
        return;
    };
    for item in items {
        match item {
            syn::Item::Use(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                let feature = cfg_feature(&item.attrs)
                    .or_else(|| module_feature.clone())
                    .unwrap_or_else(|| "default".to_owned());
                let mut leaves = Vec::new();
                collect_use_leaves(&item.tree, &mut Vec::new(), &mut leaves);
                for (source, symbol) in leaves {
                    let source = normalized_facade_source(module_path, &source, &symbol);
                    exports.insert(FacadeExport {
                        module: module_path.to_owned(),
                        symbol,
                        feature: feature.clone(),
                        source,
                    });
                }
            }
            syn::Item::Mod(child) if matches!(child.vis, syn::Visibility::Public(_)) => {
                collect_module_exports(
                    child,
                    module_feature.clone(),
                    &format!("{module_path}::{}", child.ident),
                    exports,
                );
            }
            _ => {}
        }
    }
}

fn normalized_facade_source(module: &str, source: &str, symbol: &str) -> String {
    let expected_root = match module {
        "core" => "bevy_ui_router_core",
        "view" => "bevy_ui_router_view",
        "ecs" => "bevy_ui_router_ecs",
        "prelude" => "crate",
        "platform::prelude" => "crate",
        other => panic!("unknown facade export module: {other}"),
    };
    let segments = source.split("::").collect::<Vec<_>>();
    assert_eq!(
        segments.first().copied(),
        Some(expected_root),
        "facade `{module}::{symbol}` uses unexpected source `{source}`"
    );
    assert_eq!(
        segments.last().copied(),
        Some(symbol),
        "facade `{module}::{symbol}` source leaf is `{source}`"
    );
    if module == "prelude" {
        assert!(
            matches!(segments.get(1).copied(), Some("core" | "view" | "ecs")),
            "prelude source must route through a curated facade module: {source}"
        );
        format!("crate::{}::{symbol}", segments[1])
    } else if module == "platform::prelude" {
        assert_eq!(segments.get(1).copied(), Some("core"));
        format!("crate::core::{symbol}")
    } else {
        source.to_owned()
    }
}

fn owner_source_path(
    module: &str,
    symbol: &str,
    scopes: &ProjectSourceScopes,
    public_modules: &BTreeMap<String, BTreeSet<String>>,
) -> String {
    if module == "platform::prelude" {
        return format!("crate::core::{symbol}");
    }
    let owner = match module {
        "core" => "bevy_ui_router_core",
        "view" => "bevy_ui_router_view",
        "ecs" => "bevy_ui_router_ecs",
        other => panic!("unknown facade symbol module: {other}"),
    };
    let scope = &scopes.declarations[&(owner.to_owned(), symbol.to_owned())];
    if !scope.module.is_empty()
        && public_modules
            .get(owner)
            .is_some_and(|modules| modules.contains(&scope.module))
    {
        format!("{owner}::{}::{symbol}", scope.module)
    } else {
        format!("{owner}::{symbol}")
    }
}

fn owner_public_modules() -> BTreeMap<String, BTreeSet<String>> {
    let owners = [
        (
            "bevy_ui_router_core",
            "crates/bevy_ui_router_core/src/lib.rs",
        ),
        (
            "bevy_ui_router_view",
            "crates/bevy_ui_router_view/src/lib.rs",
        ),
        ("bevy_ui_router_ecs", "crates/bevy_ui_router_ecs/src/lib.rs"),
    ];
    owners
        .into_iter()
        .map(|(owner, path)| {
            let parsed = syn::parse_file(&read_workspace(path)).unwrap();
            let modules = parsed
                .items
                .iter()
                .filter_map(|item| match item {
                    syn::Item::Mod(module) if is_public(&module.vis) => {
                        Some(module.ident.to_string())
                    }
                    _ => None,
                })
                .collect::<BTreeSet<_>>();
            (owner.to_owned(), modules)
        })
        .collect()
}

fn collect_use_leaves(
    tree: &syn::UseTree,
    prefix: &mut Vec<String>,
    leaves: &mut Vec<(String, String)>,
) {
    match tree {
        syn::UseTree::Name(name) => {
            let symbol = name.ident.to_string();
            let mut source = prefix.clone();
            source.push(symbol.clone());
            leaves.push((source.join("::"), symbol));
        }
        syn::UseTree::Rename(rename) => {
            let mut source = prefix.clone();
            source.push(rename.ident.to_string());
            leaves.push((source.join("::"), rename.rename.to_string()));
        }
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_use_leaves(&path.tree, prefix, leaves);
            prefix.pop();
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                collect_use_leaves(item, prefix, leaves);
            }
        }
        syn::UseTree::Glob(_) => panic!("public glob was rejected before export collection"),
    }
}

fn cfg_feature(attributes: &[syn::Attribute]) -> Option<String> {
    let mut feature = None;
    for attribute in attributes {
        assert!(
            !attribute.path().is_ident("cfg_attr"),
            "facade public API cannot use cfg_attr"
        );
        if !attribute.path().is_ident("cfg") {
            continue;
        }
        let predicate = attribute
            .parse_args::<syn::Meta>()
            .expect("facade cfg predicate must parse");
        let syn::Meta::NameValue(predicate) = predicate else {
            panic!("facade cfg must be an exact single feature predicate");
        };
        assert!(
            predicate.path.is_ident("feature"),
            "facade cfg must be an exact single feature predicate"
        );
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(value),
            ..
        }) = predicate.value
        else {
            panic!("facade feature cfg must use a string literal");
        };
        assert!(
            feature.replace(value.value()).is_none(),
            "facade public API cannot combine cfg predicates"
        );
    }
    feature
}

pub fn assert_devtools_adds_no_public_symbols(source: &str) {
    let file = syn::parse_file(source).expect("facade source must parse");
    for item in &file.items {
        if let syn::Item::Mod(module) = item
            && is_public(&module.vis)
        {
            assert_public_item_is_not_devtools(&module.attrs, &module.ident.to_string());
            assert_module_has_no_devtools_public_items(module, &module.ident.to_string());
        }
    }
}

fn assert_module_has_no_devtools_public_items(module: &syn::ItemMod, module_path: &str) {
    let Some((_, items)) = &module.content else {
        return;
    };
    for item in items {
        match item {
            syn::Item::Use(item) if is_public(&item.vis) => {
                assert_public_item_is_not_devtools(&item.attrs, module_path);
            }
            syn::Item::Mod(child) if is_public(&child.vis) => {
                let child_path = format!("{module_path}::{}", child.ident);
                assert_public_item_is_not_devtools(&child.attrs, &child_path);
                assert_module_has_no_devtools_public_items(child, &child_path);
            }
            item if public_definition_kind(item).is_some() => {
                assert_public_item_is_not_devtools(item_attributes(item), module_path);
            }
            _ => {}
        }
    }
}

fn assert_public_item_is_not_devtools(attributes: &[syn::Attribute], path: &str) {
    assert_ne!(
        cfg_feature(attributes).as_deref(),
        Some("devtools"),
        "devtools adds public API at `{path}`"
    );
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SourceRecord {
    owner: String,
    symbol: String,
    kind: String,
    api: String,
}

#[derive(Clone, Debug)]
struct SourceScope {
    module: String,
    imports: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default)]
struct ProjectSourceScopes {
    declarations: BTreeMap<(String, String), SourceScope>,
    items: BTreeMap<SourceRecord, SourceScope>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublicStructShape {
    Unit,
    Tuple { fields: usize, public_fields: usize },
    Named { fields: usize, public_fields: usize },
}

pub fn assert_owner_source_matches_golden(_symbols: &[SymbolRow], golden: &[GoldenRow]) {
    let canonical_by_owner_symbol = golden
        .iter()
        .map(|row| {
            (
                (row.owner.clone(), path_symbol(&row.path).to_owned()),
                row.path.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let owners = BTreeMap::from([
        ("bevy_ui_router_core", "crates/bevy_ui_router_core/src"),
        ("bevy_ui_router_view", "crates/bevy_ui_router_view/src"),
        ("bevy_ui_router_ecs", "crates/bevy_ui_router_ecs/src"),
    ]);
    let mut actual = BTreeSet::new();
    let mut all_public_names = BTreeSet::new();
    for (owner, source_dir) in owners {
        let mut files = Vec::new();
        rust_files(&workspace_dir().join(source_dir), &mut files);
        files.sort();
        for file in files {
            let source = fs::read_to_string(&file)
                .unwrap_or_else(|error| panic!("failed to read '{}': {error}", file.display()));
            let parsed = syn::parse_file(&source)
                .unwrap_or_else(|error| panic!("failed to parse '{}': {error}", file.display()));
            collect_owner_items(
                owner,
                &parsed.items,
                &canonical_by_owner_symbol,
                &mut actual,
                &mut all_public_names,
            );
        }
    }

    let expected = golden
        .iter()
        .filter(|row| row.kind != "compile-assert")
        .map(|row| SourceRecord {
            owner: row.owner.clone(),
            symbol: path_symbol(&row.path).to_owned(),
            kind: row.kind.clone(),
            api: compact(&row.api),
        })
        .collect::<BTreeSet<_>>();
    let actual = actual
        .into_iter()
        .filter(|row| {
            canonical_by_owner_symbol.contains_key(&(row.owner.clone(), row.symbol.clone()))
        })
        .collect::<BTreeSet<_>>();
    let missing = expected.difference(&actual).cloned().collect::<Vec<_>>();
    let extra = actual.difference(&expected).cloned().collect::<Vec<_>>();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "owner source differs from the filtered associated API golden\nmissing: {missing:#?}\nextra: {extra:#?}"
    );

    let project_types = project_type_catalogue();
    let source_scopes = project_source_scopes();
    assert_public_signature_closure(golden, &project_types, &source_scopes);
    assert_source_declaration_order(golden);
    let shapes = public_struct_shapes(golden);
    for row in golden.iter().filter(|row| row.kind == "struct") {
        let key = (row.owner.clone(), path_symbol(&row.path).to_owned());
        let shape = shapes
            .get(&key)
            .unwrap_or_else(|| panic!("missing public struct declaration for {}", row.path));
        if row.path == "bevy_ui_router::ecs::BevyUiRouterPlugin" {
            assert_eq!(*shape, PublicStructShape::Unit);
            continue;
        }
        match shape {
            PublicStructShape::Unit => panic!("unexpected public unit struct: {}", row.path),
            PublicStructShape::Tuple {
                fields,
                public_fields,
            }
            | PublicStructShape::Named {
                fields,
                public_fields,
            } => {
                assert!(*fields > 0, "opaque struct has no fields: {}", row.path);
                assert_eq!(
                    *public_fields, 0,
                    "allowed struct exposes public fields: {}",
                    row.path
                );
            }
        }
    }
}

fn assert_public_signature_closure(
    golden: &[GoldenRow],
    project_types: &BTreeMap<String, BTreeSet<(String, String, bool)>>,
    source_scopes: &ProjectSourceScopes,
) {
    let allowed_features = golden
        .iter()
        .map(|row| {
            (
                (row.owner.clone(), path_symbol(&row.path).to_owned()),
                row.feature.clone(),
            )
        })
        .fold(
            BTreeMap::<_, BTreeSet<_>>::new(),
            |mut map, (key, feature)| {
                map.entry(key).or_default().insert(feature);
                map
            },
        );
    for row in golden {
        for identifier in signature_identifiers(row)
            .into_iter()
            .filter(|identifier| identifier != "Self")
        {
            let source_record = SourceRecord {
                owner: row.owner.clone(),
                symbol: path_symbol(&row.path).to_owned(),
                kind: row.kind.clone(),
                api: compact(&row.api),
            };
            let scope = source_scopes.items.get(&source_record).unwrap_or_else(|| {
                panic!(
                    "missing unique source scope for {} {} '{}'",
                    row.path, row.kind, row.api
                )
            });
            let Some((resolved_identifier, resolved)) =
                resolve_project_type(&row.owner, scope, &row.api, &identifier, project_types)
            else {
                continue;
            };
            assert_eq!(
                resolved.len(),
                1,
                "public {} signature '{}' has ambiguous or noncanonical project type `{identifier}` candidates {resolved:?}",
                row.path,
                row.api
            );
            assert!(
                resolved.iter().all(|(owner, _, public)| {
                    *public
                        && allowed_features
                            .get(&(owner.clone(), resolved_identifier.clone()))
                            .is_some_and(|features| {
                                features
                                    .iter()
                                    .any(|feature| feature_is_visible_from(feature, &row.feature))
                            })
                }),
                "public {} signature '{}' reaches private, noncanonical, or stronger-feature project type `{identifier}` resolved as {resolved:?}",
                row.path,
                row.api
            );
        }
    }
}

fn assert_source_declaration_order(golden: &[GoldenRow]) {
    let canonical = golden
        .iter()
        .map(|row| (row.owner.clone(), path_symbol(&row.path).to_owned()))
        .collect::<BTreeSet<_>>();
    let owners = BTreeMap::from([
        ("bevy_ui_router_core", "crates/bevy_ui_router_core/src"),
        ("bevy_ui_router_view", "crates/bevy_ui_router_view/src"),
        ("bevy_ui_router_ecs", "crates/bevy_ui_router_ecs/src"),
    ]);
    let mut actual = BTreeMap::<(String, String, String), Vec<String>>::new();
    for (owner, source_dir) in owners {
        let mut files = Vec::new();
        rust_files(&workspace_dir().join(source_dir), &mut files);
        files.sort();
        for file in files {
            let parsed = syn::parse_file(&fs::read_to_string(file).unwrap()).unwrap();
            collect_ordered_source_api(owner, &parsed.items, &canonical, &mut actual);
        }
    }
    let mut checked = BTreeSet::new();
    for row in golden
        .iter()
        .filter(|row| matches!(row.kind.as_str(), "variant" | "method" | "trait-method"))
    {
        let key = (
            row.owner.clone(),
            path_symbol(&row.path).to_owned(),
            row.kind.clone(),
        );
        if !checked.insert((row.path.clone(), row.kind.clone())) {
            continue;
        }
        let expected = golden
            .iter()
            .filter(|candidate| {
                candidate.path == row.path
                    && candidate.owner == row.owner
                    && candidate.kind == row.kind
            })
            .map(|candidate| compact(&candidate.api))
            .collect::<Vec<_>>();
        assert_eq!(
            actual.get(&key).cloned().unwrap_or_default(),
            expected,
            "source declaration order drifted for {} {}",
            row.path,
            row.kind
        );
    }
}

fn collect_ordered_source_api(
    owner: &str,
    items: &[syn::Item],
    canonical: &BTreeSet<(String, String)>,
    records: &mut BTreeMap<(String, String, String), Vec<String>>,
) {
    use syn::__private::ToTokens;
    for item in items {
        if item_is_test(item) {
            continue;
        }
        match item {
            syn::Item::Enum(item) if is_public(&item.vis) => {
                let symbol = item.ident.to_string();
                if canonical.contains(&(owner.to_owned(), symbol.clone())) {
                    let key = (owner.to_owned(), symbol, "variant".to_owned());
                    records.entry(key).or_default().extend(
                        item.variants
                            .iter()
                            .map(|variant| compact(&render_variant(variant))),
                    );
                }
            }
            syn::Item::Trait(item) if is_public(&item.vis) => {
                let symbol = item.ident.to_string();
                if canonical.contains(&(owner.to_owned(), symbol.clone())) {
                    let key = (owner.to_owned(), symbol, "trait-method".to_owned());
                    records
                        .entry(key)
                        .or_default()
                        .extend(item.items.iter().filter_map(|item| match item {
                            syn::TraitItem::Fn(method) => {
                                Some(compact(&method.sig.to_token_stream().to_string()))
                            }
                            _ => None,
                        }));
                }
            }
            syn::Item::Impl(item) if item.trait_.is_none() => {
                let Some(symbol) = type_last_ident(&item.self_ty) else {
                    continue;
                };
                if canonical.contains(&(owner.to_owned(), symbol.clone())) {
                    let key = (owner.to_owned(), symbol, "method".to_owned());
                    records
                        .entry(key)
                        .or_default()
                        .extend(item.items.iter().filter_map(|item| match item {
                            syn::ImplItem::Fn(method) if is_public(&method.vis) => {
                                Some(compact(&format!(
                                    "{}{}",
                                    method.vis.to_token_stream(),
                                    method.sig.to_token_stream()
                                )))
                            }
                            _ => None,
                        }));
                }
            }
            syn::Item::Mod(module) => {
                if let Some((_, nested)) = &module.content {
                    collect_ordered_source_api(owner, nested, canonical, records);
                }
            }
            _ => {}
        }
    }
}

fn render_variant(variant: &syn::Variant) -> String {
    use syn::__private::ToTokens;
    let mut rendered = variant.ident.to_string();
    match &variant.fields {
        syn::Fields::Unit => {}
        syn::Fields::Unnamed(fields) => {
            rendered.push('(');
            rendered.push_str(
                &fields
                    .unnamed
                    .iter()
                    .map(|field| compact(&field.ty.to_token_stream().to_string()))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            rendered.push(')');
        }
        syn::Fields::Named(fields) => {
            rendered.push('{');
            rendered.push_str(
                &fields
                    .named
                    .iter()
                    .map(|field| {
                        format!(
                            "{}:{}",
                            field.ident.as_ref().unwrap(),
                            compact(&field.ty.to_token_stream().to_string())
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(","),
            );
            rendered.push('}');
        }
    }
    rendered
}

fn signature_identifiers(row: &GoldenRow) -> BTreeSet<String> {
    let signature = match row.kind.as_str() {
        "method" | "trait-method" => row.api.as_str(),
        "variant" => row
            .api
            .find(|character| matches!(character, '(' | '{'))
            .map(|index| &row.api[index + 1..row.api.len() - 1])
            .unwrap_or(""),
        "trait" => row
            .api
            .split_once(':')
            .map(|(_, bounds)| bounds)
            .unwrap_or(""),
        "trait-impl" => row.api.as_str(),
        _ => "",
    };
    identifiers(signature).map(str::to_owned).collect()
}

fn feature_is_visible_from(referenced: &str, consumer: &str) -> bool {
    match consumer {
        "default" => referenced == "default",
        "view" => matches!(referenced, "default" | "view"),
        "ecs" => matches!(referenced, "default" | "view" | "ecs"),
        "ecs-test-support" => {
            matches!(referenced, "default" | "view" | "ecs" | "ecs-test-support")
        }
        "platform" => matches!(referenced, "default" | "platform"),
        other => panic!("unknown feature strength: {other}"),
    }
}

fn project_type_catalogue() -> BTreeMap<String, BTreeSet<(String, String, bool)>> {
    let owners = BTreeMap::from([
        ("bevy_ui_router_core", "crates/bevy_ui_router_core/src"),
        ("bevy_ui_router_view", "crates/bevy_ui_router_view/src"),
        ("bevy_ui_router_ecs", "crates/bevy_ui_router_ecs/src"),
    ]);
    let mut catalogue = BTreeMap::<String, BTreeSet<(String, String, bool)>>::new();
    for (owner, source_dir) in owners {
        let mut files = Vec::new();
        rust_files(&workspace_dir().join(source_dir), &mut files);
        for file in files {
            let module = source_module(&workspace_dir().join(source_dir), &file);
            let parsed = syn::parse_file(&fs::read_to_string(file).unwrap()).unwrap();
            collect_project_types(owner, &module, &parsed.items, &mut catalogue);
        }
    }
    catalogue
}

fn collect_project_types(
    owner: &str,
    module: &str,
    items: &[syn::Item],
    catalogue: &mut BTreeMap<String, BTreeSet<(String, String, bool)>>,
) {
    for item in items {
        if item_is_test(item) {
            continue;
        }
        let definition = match item {
            syn::Item::Enum(item) => Some((&item.ident, &item.vis)),
            syn::Item::Struct(item) => Some((&item.ident, &item.vis)),
            syn::Item::Trait(item) => Some((&item.ident, &item.vis)),
            syn::Item::Type(item) => Some((&item.ident, &item.vis)),
            syn::Item::Union(item) => Some((&item.ident, &item.vis)),
            _ => None,
        };
        if let Some((identifier, visibility)) = definition {
            catalogue
                .entry(identifier.to_string())
                .or_default()
                .insert((owner.to_owned(), module.to_owned(), is_public(visibility)));
        }
        if let syn::Item::Mod(child) = item
            && let Some((_, nested)) = &child.content
        {
            let nested_module = module_ident(module, &child.ident.to_string());
            collect_project_types(owner, &nested_module, nested, catalogue);
        }
    }
}

fn source_module(source_root: &Path, file: &Path) -> String {
    let relative = file.strip_prefix(source_root).unwrap();
    let mut parts = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let file_name = parts.pop().unwrap();
    let stem = file_name.trim_end_matches(".rs");
    if !matches!(stem, "lib" | "mod") {
        parts.push(stem.to_owned());
    }
    parts.join("::")
}

fn module_ident(module: &str, child: &str) -> String {
    if module.is_empty() {
        child.to_owned()
    } else {
        format!("{module}::{child}")
    }
}

fn project_source_scopes() -> ProjectSourceScopes {
    let owners = BTreeMap::from([
        ("bevy_ui_router_core", "crates/bevy_ui_router_core/src"),
        ("bevy_ui_router_view", "crates/bevy_ui_router_view/src"),
        ("bevy_ui_router_ecs", "crates/bevy_ui_router_ecs/src"),
    ]);
    let mut sources = Vec::new();
    for (owner, source_dir) in owners {
        let source_root = workspace_dir().join(source_dir);
        let mut files = Vec::new();
        rust_files(&source_root, &mut files);
        files.sort();
        for file in files {
            let module = source_module(&source_root, &file);
            sources.push((owner, module, file));
        }
    }
    let mut scopes = ProjectSourceScopes::default();
    for (owner, module, file) in &sources {
        let parsed = syn::parse_file(&fs::read_to_string(file).unwrap()).unwrap();
        collect_source_declarations(owner, module, &parsed.items, &mut scopes);
    }
    for (owner, module, file) in &sources {
        let parsed = syn::parse_file(&fs::read_to_string(file).unwrap()).unwrap();
        collect_source_item_scopes(owner, module, &parsed.items, &mut scopes);
    }
    scopes
}

fn collect_source_scopes(
    owner: &str,
    module: &str,
    items: &[syn::Item],
    scopes: &mut ProjectSourceScopes,
) {
    collect_source_declarations(owner, module, items, scopes);
    collect_source_item_scopes(owner, module, items, scopes);
}

fn collect_source_declarations(
    owner: &str,
    module: &str,
    items: &[syn::Item],
    scopes: &mut ProjectSourceScopes,
) {
    let imports = module_imports(items);
    for item in items {
        if item_is_test(item) {
            continue;
        }
        let definition = match item {
            syn::Item::Enum(item) if is_public(&item.vis) => Some(item.ident.to_string()),
            syn::Item::Struct(item) if is_public(&item.vis) => Some(item.ident.to_string()),
            syn::Item::Trait(item) if is_public(&item.vis) => Some(item.ident.to_string()),
            syn::Item::Type(item) if is_public(&item.vis) => Some(item.ident.to_string()),
            syn::Item::Union(item) if is_public(&item.vis) => Some(item.ident.to_string()),
            _ => None,
        };
        if let Some(symbol) = definition {
            let key = (owner.to_owned(), symbol);
            assert!(
                scopes
                    .declarations
                    .insert(
                        key.clone(),
                        SourceScope {
                            module: module.to_owned(),
                            imports: imports.clone(),
                        },
                    )
                    .is_none(),
                "duplicate public type declaration scope: {key:?}"
            );
        }
        if let syn::Item::Mod(child) = item
            && let Some((_, nested)) = &child.content
        {
            collect_source_declarations(
                owner,
                &module_ident(module, &child.ident.to_string()),
                nested,
                scopes,
            );
        }
    }
}

fn collect_source_item_scopes(
    owner: &str,
    module: &str,
    items: &[syn::Item],
    scopes: &mut ProjectSourceScopes,
) {
    use syn::__private::ToTokens;
    let scope = SourceScope {
        module: module.to_owned(),
        imports: module_imports(items),
    };
    for item in items {
        if item_is_test(item) {
            continue;
        }
        match item {
            syn::Item::Struct(item) if is_public(&item.vis) => {
                let symbol = item.ident.to_string();
                insert_source_item_scope(
                    scopes,
                    source_record(
                        owner,
                        &symbol,
                        "struct",
                        &format!("pubstruct{symbol}{}", generic_parameters(&item.generics)),
                    ),
                    &scope,
                );
                collect_derive_source_scopes(owner, &symbol, &item.attrs, &scope, scopes);
                for field in &item.fields {
                    if is_public(&field.vis) {
                        insert_source_item_scope(
                            scopes,
                            source_record(
                                owner,
                                &symbol,
                                "field",
                                &field.to_token_stream().to_string(),
                            ),
                            &scope,
                        );
                    }
                }
            }
            syn::Item::Enum(item) if is_public(&item.vis) => {
                let symbol = item.ident.to_string();
                insert_source_item_scope(
                    scopes,
                    source_record(
                        owner,
                        &symbol,
                        "enum",
                        &format!("pubenum{symbol}{}", generic_parameters(&item.generics)),
                    ),
                    &scope,
                );
                collect_derive_source_scopes(owner, &symbol, &item.attrs, &scope, scopes);
                for variant in &item.variants {
                    insert_source_item_scope(
                        scopes,
                        source_record(owner, &symbol, "variant", &render_variant(variant)),
                        &scope,
                    );
                }
            }
            syn::Item::Trait(item) if is_public(&item.vis) => {
                let symbol = item.ident.to_string();
                let bounds = if item.supertraits.is_empty() {
                    String::new()
                } else {
                    format!(":{}", item.supertraits.to_token_stream())
                };
                insert_source_item_scope(
                    scopes,
                    source_record(
                        owner,
                        &symbol,
                        "trait",
                        &format!("pubtrait{symbol}{bounds}"),
                    ),
                    &scope,
                );
                for trait_item in &item.items {
                    let record = match trait_item {
                        syn::TraitItem::Fn(method) => Some(source_record(
                            owner,
                            &symbol,
                            "trait-method",
                            &method.sig.to_token_stream().to_string(),
                        )),
                        syn::TraitItem::Const(value) => Some(source_record(
                            owner,
                            &symbol,
                            "associated-const",
                            &value.to_token_stream().to_string(),
                        )),
                        syn::TraitItem::Type(value) => Some(source_record(
                            owner,
                            &symbol,
                            "associated-type",
                            &value.to_token_stream().to_string(),
                        )),
                        _ => None,
                    };
                    if let Some(record) = record {
                        insert_source_item_scope(scopes, record, &scope);
                    }
                }
            }
            syn::Item::Impl(item) => collect_impl_source_scopes(owner, item, &scope, scopes),
            syn::Item::Mod(child) => {
                if let Some((_, nested)) = &child.content {
                    collect_source_item_scopes(
                        owner,
                        &module_ident(module, &child.ident.to_string()),
                        nested,
                        scopes,
                    );
                }
            }
            _ => {}
        }
    }
}

fn collect_impl_source_scopes(
    owner: &str,
    item: &syn::ItemImpl,
    scope: &SourceScope,
    scopes: &mut ProjectSourceScopes,
) {
    use syn::__private::ToTokens;
    let self_symbol = type_last_ident(&item.self_ty);
    if let Some((_, trait_path, _)) = &item.trait_ {
        let trait_symbol = trait_path
            .segments
            .last()
            .map(|segment| segment.ident.to_string());
        let record_symbol = self_symbol
            .as_ref()
            .filter(|symbol| {
                scopes
                    .declarations
                    .contains_key(&(owner.to_owned(), (*symbol).clone()))
            })
            .cloned()
            .or_else(|| {
                trait_symbol
                    .as_ref()
                    .filter(|symbol| {
                        scopes
                            .declarations
                            .contains_key(&(owner.to_owned(), (*symbol).clone()))
                    })
                    .cloned()
            });
        if let Some(symbol) = record_symbol {
            let trait_name = normalized_trait_name(trait_path);
            let self_name = if self_symbol.as_deref() == Some(&symbol) {
                "Self".to_owned()
            } else {
                normalize_self_type(&item.self_ty.to_token_stream().to_string())
            };
            insert_source_item_scope(
                scopes,
                source_record(
                    owner,
                    &symbol,
                    "trait-impl",
                    &format!("impl{trait_name}for{self_name}"),
                ),
                scope,
            );
        }
        return;
    }
    let Some(symbol) = self_symbol else {
        return;
    };
    if !scopes
        .declarations
        .contains_key(&(owner.to_owned(), symbol.clone()))
    {
        return;
    }
    for impl_item in &item.items {
        let record = match impl_item {
            syn::ImplItem::Fn(method) if is_public(&method.vis) => Some(source_record(
                owner,
                &symbol,
                "method",
                &format!(
                    "{}{}",
                    method.vis.to_token_stream(),
                    method.sig.to_token_stream()
                ),
            )),
            syn::ImplItem::Const(value) if is_public(&value.vis) => Some(source_record(
                owner,
                &symbol,
                "associated-const",
                &value.to_token_stream().to_string(),
            )),
            syn::ImplItem::Type(value) if is_public(&value.vis) => Some(source_record(
                owner,
                &symbol,
                "associated-type",
                &value.to_token_stream().to_string(),
            )),
            _ => None,
        };
        if let Some(record) = record {
            insert_source_item_scope(scopes, record, scope);
        }
    }
}

fn collect_derive_source_scopes(
    owner: &str,
    symbol: &str,
    attributes: &[syn::Attribute],
    scope: &SourceScope,
    scopes: &mut ProjectSourceScopes,
) {
    let mut records = BTreeSet::new();
    collect_derives(owner, symbol, attributes, &mut records);
    for record in records {
        insert_source_item_scope(scopes, record, scope);
    }
}

fn insert_source_item_scope(
    scopes: &mut ProjectSourceScopes,
    record: SourceRecord,
    scope: &SourceScope,
) {
    assert!(
        scopes.items.insert(record.clone(), scope.clone()).is_none(),
        "duplicate source scope for {record:?}"
    );
}

fn module_imports(items: &[syn::Item]) -> BTreeMap<String, String> {
    use std::collections::btree_map::Entry;

    let mut imports = BTreeMap::new();
    for item in items {
        if let syn::Item::Use(item) = item {
            let mut leaves = Vec::new();
            collect_internal_use_leaves(&item.tree, &mut Vec::new(), &mut leaves);
            for (source, name) in leaves {
                match imports.entry(name) {
                    Entry::Vacant(entry) => {
                        entry.insert(source);
                    }
                    Entry::Occupied(entry) => panic!(
                        "duplicate import alias `{}` in one source scope: `{}` and `{source}`",
                        entry.key(),
                        entry.get()
                    ),
                }
            }
        }
    }
    imports
}

fn collect_internal_use_leaves(
    tree: &syn::UseTree,
    prefix: &mut Vec<String>,
    leaves: &mut Vec<(String, String)>,
) {
    match tree {
        syn::UseTree::Name(name) => {
            let mut source = prefix.clone();
            source.push(name.ident.to_string());
            leaves.push((source.join("::"), name.ident.to_string()));
        }
        syn::UseTree::Rename(rename) => {
            let mut source = prefix.clone();
            source.push(rename.ident.to_string());
            leaves.push((source.join("::"), rename.rename.to_string()));
        }
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_internal_use_leaves(&path.tree, prefix, leaves);
            prefix.pop();
        }
        syn::UseTree::Group(group) => {
            for tree in &group.items {
                collect_internal_use_leaves(tree, prefix, leaves);
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}

fn resolve_project_type(
    owner: &str,
    scope: &SourceScope,
    api: &str,
    identifier: &str,
    catalogue: &BTreeMap<String, BTreeSet<(String, String, bool)>>,
) -> Option<(String, Vec<(String, String, bool)>)> {
    for explicit_owner in [
        "bevy_ui_router_core",
        "bevy_ui_router_view",
        "bevy_ui_router_ecs",
    ] {
        if api.contains(&format!("{explicit_owner}::{identifier}"))
            && let Some(original_candidates) = catalogue.get(identifier)
        {
            return Some((
                identifier.to_owned(),
                original_candidates
                    .iter()
                    .filter(|(candidate_owner, _, _)| candidate_owner == explicit_owner)
                    .cloned()
                    .collect(),
            ));
        }
    }
    if let Some(import) = scope.imports.get(identifier) {
        let imported_identifier = import.rsplit("::").next().unwrap();
        let candidates = catalogue.get(imported_identifier)?;
        for imported_owner in [
            "bevy_ui_router_core",
            "bevy_ui_router_view",
            "bevy_ui_router_ecs",
        ] {
            if import.starts_with(imported_owner) {
                return Some((
                    imported_identifier.to_owned(),
                    candidates
                        .iter()
                        .filter(|(candidate_owner, _, _)| candidate_owner == imported_owner)
                        .cloned()
                        .collect(),
                ));
            }
        }
        let (module, root_reexport) =
            relative_import_module(&scope.module, import, imported_identifier);
        let exact = candidates
            .iter()
            .filter(|(candidate_owner, candidate_module, _)| {
                candidate_owner == owner && candidate_module == &module
            })
            .cloned()
            .collect::<Vec<_>>();
        if exact.is_empty()
            && !root_reexport
            && !matches!(import.split("::").next(), Some("crate" | "self" | "super"))
        {
            return None;
        }
        let resolved = if exact.is_empty() && root_reexport {
            candidates
                .iter()
                .filter(|(candidate_owner, _, public)| candidate_owner == owner && *public)
                .cloned()
                .collect()
        } else {
            exact
        };
        return Some((imported_identifier.to_owned(), resolved));
    }
    let original_candidates = catalogue.get(identifier)?;
    let local = original_candidates
        .iter()
        .filter(|(candidate_owner, candidate_module, _)| {
            candidate_owner == owner && candidate_module == &scope.module
        })
        .cloned()
        .collect::<Vec<_>>();
    if local.is_empty() {
        Some((
            identifier.to_owned(),
            original_candidates
                .iter()
                .filter(|(_, _, public)| *public)
                .cloned()
                .collect(),
        ))
    } else {
        Some((identifier.to_owned(), local))
    }
}

fn relative_import_module(
    current_module: &str,
    import: &str,
    imported_identifier: &str,
) -> (String, bool) {
    let module_path = import
        .strip_suffix(imported_identifier)
        .and_then(|path| path.strip_suffix("::"))
        .unwrap_or("");
    let mut relative = module_path.split("::").filter(|part| !part.is_empty());
    let first = relative.next();
    let mut module = current_module
        .split("::")
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut remainder = Vec::new();
    let crate_absolute = first == Some("crate");
    match first {
        Some("crate") => module.clear(),
        Some("self") => {}
        Some("super") => {
            assert!(
                module.pop().is_some(),
                "import `{import}` traverses above the crate root"
            );
            while let Some(part) = relative.next() {
                if part != "super" {
                    remainder.push(part.to_owned());
                    break;
                }
                assert!(
                    module.pop().is_some(),
                    "import `{import}` traverses above the crate root"
                );
            }
        }
        Some(part) => remainder.push(part.to_owned()),
        None => {}
    }
    remainder.extend(relative.map(str::to_owned));
    module.extend(remainder);
    let module = module.join("::");
    let root_reexport = crate_absolute && (module.is_empty() || module == "prelude");
    (module, root_reexport)
}

fn public_struct_shapes(golden: &[GoldenRow]) -> BTreeMap<(String, String), PublicStructShape> {
    let wanted = golden
        .iter()
        .filter(|row| row.kind == "struct")
        .map(|row| (row.owner.clone(), path_symbol(&row.path).to_owned()))
        .collect::<BTreeSet<_>>();
    let owners = BTreeMap::from([
        ("bevy_ui_router_core", "crates/bevy_ui_router_core/src"),
        ("bevy_ui_router_view", "crates/bevy_ui_router_view/src"),
        ("bevy_ui_router_ecs", "crates/bevy_ui_router_ecs/src"),
    ]);
    let mut shapes = BTreeMap::new();
    for (owner, source_dir) in owners {
        let mut files = Vec::new();
        rust_files(&workspace_dir().join(source_dir), &mut files);
        for file in files {
            let source = fs::read_to_string(&file).unwrap();
            let parsed = syn::parse_file(&source).unwrap();
            collect_public_struct_shapes(owner, &parsed.items, &wanted, &mut shapes);
        }
    }
    assert_eq!(
        shapes.len(),
        wanted.len(),
        "allowed struct declaration set drifted"
    );
    shapes
}

fn collect_public_struct_shapes(
    owner: &str,
    items: &[syn::Item],
    wanted: &BTreeSet<(String, String)>,
    shapes: &mut BTreeMap<(String, String), PublicStructShape>,
) {
    for item in items {
        if item_is_test(item) {
            continue;
        }
        match item {
            syn::Item::Struct(item) if is_public(&item.vis) => {
                let key = (owner.to_owned(), item.ident.to_string());
                if wanted.contains(&key) {
                    let shape = match &item.fields {
                        syn::Fields::Unit => PublicStructShape::Unit,
                        syn::Fields::Unnamed(fields) => PublicStructShape::Tuple {
                            fields: fields.unnamed.len(),
                            public_fields: fields
                                .unnamed
                                .iter()
                                .filter(|field| is_public(&field.vis))
                                .count(),
                        },
                        syn::Fields::Named(fields) => PublicStructShape::Named {
                            fields: fields.named.len(),
                            public_fields: fields
                                .named
                                .iter()
                                .filter(|field| is_public(&field.vis))
                                .count(),
                        },
                    };
                    assert!(
                        shapes.insert(key.clone(), shape).is_none(),
                        "duplicate public struct declaration: {key:?}"
                    );
                }
            }
            syn::Item::Mod(module) => {
                if let Some((_, nested)) = &module.content {
                    collect_public_struct_shapes(owner, nested, wanted, shapes);
                }
            }
            _ => {}
        }
    }
}

fn rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read '{}': {error}", directory.display()))
    {
        let path = entry
            .expect("source directory entry must be readable")
            .path();
        if path.is_dir() {
            rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("_tests.rs"))
        {
            files.push(path);
        }
    }
}

fn collect_owner_items(
    owner: &str,
    items: &[syn::Item],
    canonical: &BTreeMap<(String, String), String>,
    records: &mut BTreeSet<SourceRecord>,
    public_names: &mut BTreeSet<String>,
) {
    use syn::__private::ToTokens;
    for item in items {
        if item_is_test(item) {
            continue;
        }
        match item {
            syn::Item::Struct(item) if is_public(&item.vis) => {
                let symbol = item.ident.to_string();
                public_names.insert(symbol.clone());
                if canonical.contains_key(&(owner.to_owned(), symbol.clone())) {
                    records.insert(source_record(
                        owner,
                        &symbol,
                        "struct",
                        &format!("pubstruct{symbol}{}", generic_parameters(&item.generics)),
                    ));
                    collect_derives(owner, &symbol, &item.attrs, records);
                    for field in &item.fields {
                        if is_public(&field.vis) {
                            records.insert(source_record(
                                owner,
                                &symbol,
                                "field",
                                &field.to_token_stream().to_string(),
                            ));
                        }
                    }
                }
            }
            syn::Item::Enum(item) if is_public(&item.vis) => {
                let symbol = item.ident.to_string();
                public_names.insert(symbol.clone());
                if canonical.contains_key(&(owner.to_owned(), symbol.clone())) {
                    records.insert(source_record(
                        owner,
                        &symbol,
                        "enum",
                        &format!("pubenum{symbol}{}", generic_parameters(&item.generics)),
                    ));
                    collect_derives(owner, &symbol, &item.attrs, records);
                    for variant in &item.variants {
                        let mut rendered = variant.ident.to_string();
                        match &variant.fields {
                            syn::Fields::Unit => {}
                            syn::Fields::Unnamed(fields) => {
                                rendered.push('(');
                                rendered.push_str(
                                    &fields
                                        .unnamed
                                        .iter()
                                        .map(|field| {
                                            compact(&field.ty.to_token_stream().to_string())
                                        })
                                        .collect::<Vec<_>>()
                                        .join(","),
                                );
                                rendered.push(')');
                            }
                            syn::Fields::Named(fields) => {
                                rendered.push('{');
                                rendered.push_str(
                                    &fields
                                        .named
                                        .iter()
                                        .map(|field| {
                                            format!(
                                                "{}:{}",
                                                field.ident.as_ref().unwrap(),
                                                compact(&field.ty.to_token_stream().to_string())
                                            )
                                        })
                                        .collect::<Vec<_>>()
                                        .join(","),
                                );
                                rendered.push('}');
                            }
                        }
                        records.insert(source_record(owner, &symbol, "variant", &rendered));
                    }
                }
            }
            syn::Item::Trait(item) if is_public(&item.vis) => {
                let symbol = item.ident.to_string();
                public_names.insert(symbol.clone());
                if canonical.contains_key(&(owner.to_owned(), symbol.clone())) {
                    let bounds = if item.supertraits.is_empty() {
                        String::new()
                    } else {
                        format!(":{}", item.supertraits.to_token_stream())
                    };
                    records.insert(source_record(
                        owner,
                        &symbol,
                        "trait",
                        &format!("pubtrait{symbol}{bounds}"),
                    ));
                    for trait_item in &item.items {
                        match trait_item {
                            syn::TraitItem::Fn(method) => {
                                records.insert(source_record(
                                    owner,
                                    &symbol,
                                    "trait-method",
                                    &method.sig.to_token_stream().to_string(),
                                ));
                            }
                            syn::TraitItem::Const(value) => {
                                records.insert(source_record(
                                    owner,
                                    &symbol,
                                    "associated-const",
                                    &value.to_token_stream().to_string(),
                                ));
                            }
                            syn::TraitItem::Type(value) => {
                                records.insert(source_record(
                                    owner,
                                    &symbol,
                                    "associated-type",
                                    &value.to_token_stream().to_string(),
                                ));
                            }
                            _ => {}
                        }
                    }
                }
            }
            syn::Item::Impl(item) => collect_impl(owner, item, canonical, records),
            syn::Item::Mod(module) => {
                if let Some((_, nested)) = &module.content {
                    collect_owner_items(owner, nested, canonical, records, public_names);
                }
            }
            _ => {}
        }
    }
}

fn collect_impl(
    owner: &str,
    item: &syn::ItemImpl,
    canonical: &BTreeMap<(String, String), String>,
    records: &mut BTreeSet<SourceRecord>,
) {
    use syn::__private::ToTokens;
    let self_symbol = type_last_ident(&item.self_ty);
    if let Some((_, trait_path, _)) = &item.trait_ {
        let trait_symbol = trait_path
            .segments
            .last()
            .map(|segment| segment.ident.to_string());
        let record_symbol = self_symbol
            .as_ref()
            .filter(|symbol| canonical.contains_key(&(owner.to_owned(), (*symbol).clone())))
            .cloned()
            .or_else(|| {
                trait_symbol
                    .as_ref()
                    .filter(|symbol| canonical.contains_key(&(owner.to_owned(), (*symbol).clone())))
                    .cloned()
            });
        if let Some(symbol) = record_symbol {
            let trait_name = normalized_trait_name(trait_path);
            let self_name = if self_symbol.as_deref() == Some(&symbol) {
                "Self".to_owned()
            } else {
                normalize_self_type(&item.self_ty.to_token_stream().to_string())
            };
            records.insert(source_record(
                owner,
                &symbol,
                "trait-impl",
                &format!("impl{trait_name}for{self_name}"),
            ));
        }
        return;
    }
    let Some(symbol) = self_symbol else {
        return;
    };
    if !canonical.contains_key(&(owner.to_owned(), symbol.clone())) {
        return;
    }
    for impl_item in &item.items {
        match impl_item {
            syn::ImplItem::Fn(method) if is_public(&method.vis) => {
                let api = format!(
                    "{}{}",
                    method.vis.to_token_stream(),
                    method.sig.to_token_stream()
                );
                records.insert(source_record(owner, &symbol, "method", &api));
            }
            syn::ImplItem::Const(value) if is_public(&value.vis) => {
                records.insert(source_record(
                    owner,
                    &symbol,
                    "associated-const",
                    &value.to_token_stream().to_string(),
                ));
            }
            syn::ImplItem::Type(value) if is_public(&value.vis) => {
                records.insert(source_record(
                    owner,
                    &symbol,
                    "associated-type",
                    &value.to_token_stream().to_string(),
                ));
            }
            _ => {}
        }
    }
}

fn collect_derives(
    owner: &str,
    symbol: &str,
    attributes: &[syn::Attribute],
    records: &mut BTreeSet<SourceRecord>,
) {
    use syn::punctuated::Punctuated;
    use syn::token::Comma;
    for attribute in attributes {
        if !attribute.path().is_ident("derive") {
            continue;
        }
        let derives = attribute
            .parse_args_with(Punctuated::<syn::Path, Comma>::parse_terminated)
            .expect("derive list must parse");
        for derive in derives {
            let name = derive.segments.last().unwrap().ident.to_string();
            let normalized = match name.as_str() {
                "Resource" => "bevy_ecs::resource::Resource",
                "Component" => "bevy_ecs::component::Component",
                "SystemSet" => "bevy_ecs::schedule::SystemSet",
                other => other,
            };
            records.insert(source_record(
                owner,
                symbol,
                "trait-impl",
                &format!("impl{normalized}forSelf"),
            ));
            if name == "Resource" {
                records.insert(source_record(
                    owner,
                    symbol,
                    "trait-impl",
                    "implbevy_ecs::component::ComponentforSelf",
                ));
            }
        }
    }
}

fn normalized_trait_name(path: &syn::Path) -> String {
    let last = path.segments.last().unwrap().ident.to_string();
    match last.as_str() {
        "Error" => "std::error::Error".to_owned(),
        "Plugin" => "bevy_app::Plugin".to_owned(),
        "Resource" => "bevy_ecs::resource::Resource".to_owned(),
        "Component" => "bevy_ecs::component::Component".to_owned(),
        "SystemSet" => "bevy_ecs::schedule::SystemSet".to_owned(),
        _ => last,
    }
}

fn normalize_self_type(source: &str) -> String {
    let compact = compact(source);
    if compact == "App" {
        "bevy_app::App".to_owned()
    } else {
        compact
    }
}

fn item_is_test(item: &syn::Item) -> bool {
    use syn::__private::ToTokens;
    item_attributes(item).iter().any(|attribute| {
        attribute.path().is_ident("cfg")
            && attribute
                .meta
                .to_token_stream()
                .to_string()
                .contains("test")
    })
}

fn item_attributes(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(item) => &item.attrs,
        syn::Item::Enum(item) => &item.attrs,
        syn::Item::ExternCrate(item) => &item.attrs,
        syn::Item::Fn(item) => &item.attrs,
        syn::Item::ForeignMod(item) => &item.attrs,
        syn::Item::Impl(item) => &item.attrs,
        syn::Item::Macro(item) => &item.attrs,
        syn::Item::Mod(item) => &item.attrs,
        syn::Item::Static(item) => &item.attrs,
        syn::Item::Struct(item) => &item.attrs,
        syn::Item::Trait(item) => &item.attrs,
        syn::Item::TraitAlias(item) => &item.attrs,
        syn::Item::Type(item) => &item.attrs,
        syn::Item::Union(item) => &item.attrs,
        syn::Item::Use(item) => &item.attrs,
        _ => &[],
    }
}

fn source_record(owner: &str, symbol: &str, kind: &str, api: &str) -> SourceRecord {
    SourceRecord {
        owner: owner.to_owned(),
        symbol: symbol.to_owned(),
        kind: kind.to_owned(),
        api: compact(api),
    }
}

fn is_public(visibility: &syn::Visibility) -> bool {
    matches!(visibility, syn::Visibility::Public(_))
}

fn type_last_ident(ty: &syn::Type) -> Option<String> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn generic_parameters(generics: &syn::Generics) -> String {
    use syn::__private::ToTokens;
    if generics.params.is_empty() {
        String::new()
    } else {
        format!(
            "<{}>",
            compact(&generics.params.to_token_stream().to_string())
        )
    }
}

fn path_symbol(path: &str) -> &str {
    path.rsplit("::")
        .next()
        .expect("canonical path has a symbol")
}

fn compact(input: &str) -> String {
    let mut compact = input
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    while compact.contains(",)") || compact.contains(",>") {
        compact = compact.replace(",)", ")").replace(",>", ">");
    }
    compact = compact
        .replace("(mutself", "(self")
        .replace(",mutself", ",self");
    if compact.contains("bevy_ecs::entity::Entity") {
        compact
    } else {
        replace_identifier(&compact, "Entity", "bevy_ecs::entity::Entity")
    }
}

fn replace_identifier(input: &str, from: &str, to: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut offset = 0;
    while let Some(relative) = input[offset..].find(from) {
        let start = offset + relative;
        let end = start + from.len();
        let before = input[..start].chars().next_back();
        let after = input[end..].chars().next();
        let boundary = |character: Option<char>| {
            character
                .is_none_or(|character| !(character.is_ascii_alphanumeric() || character == '_'))
        };
        output.push_str(&input[offset..start]);
        if boundary(before) && boundary(after) {
            output.push_str(to);
        } else {
            output.push_str(from);
        }
        offset = end;
    }
    output.push_str(&input[offset..]);
    output
}

fn identifiers(input: &str) -> impl Iterator<Item = &str> {
    input
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .filter(|token| token.chars().next().is_some_and(char::is_uppercase))
}

#[derive(Clone, Debug)]
pub struct ConsumerCase {
    pub kind: &'static str,
    pub feature_set: &'static str,
    pub slug: String,
    pub default_features: bool,
    pub features: Vec<String>,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct NegativeCase {
    pub consumer: ConsumerCase,
    pub expected_codes: Vec<&'static str>,
}

pub fn positive_consumer_cases(
    module_rows: &[TableRow],
    symbols: &[SymbolRow],
    golden: &[GoldenRow],
) -> Vec<ConsumerCase> {
    let configurations = [
        ("core-only", false, Vec::<String>::new(), vec!["default"]),
        (
            "default-enabled",
            true,
            Vec::<String>::new(),
            vec!["default", "view"],
        ),
        (
            "view-explicit",
            false,
            vec!["view".to_owned()],
            vec!["default", "view"],
        ),
        (
            "ecs",
            false,
            vec!["ecs".to_owned()],
            vec!["default", "view", "ecs"],
        ),
        (
            "test-support",
            false,
            vec!["test-support".to_owned()],
            vec!["default", "view", "ecs", "ecs-test-support"],
        ),
        (
            "bsn",
            false,
            vec!["bsn".to_owned()],
            vec!["default", "view", "ecs"],
        ),
        (
            "platform",
            false,
            vec!["platform".to_owned()],
            vec!["default", "platform"],
        ),
        (
            "devtools",
            false,
            vec!["devtools".to_owned()],
            vec!["default"],
        ),
    ];
    configurations
        .into_iter()
        .map(|(feature_set, default_features, features, visible)| {
            let visible = visible.into_iter().collect::<BTreeSet<_>>();
            let mut lines = vec!["#![allow(unused_imports)]".to_owned()];
            lines.push("use bevy_ui_router::core::*;".to_owned());
            if visible.contains("view") {
                lines.push("use bevy_ui_router::view::*;".to_owned());
            }
            if visible.contains("ecs") || visible.contains("ecs-test-support") {
                lines.push("use bevy_ui_router::ecs::*;".to_owned());
            }
            let mut index = 0usize;
            for row in module_rows {
                let module_feature = row.0[1].trim_matches(char::from(96));
                let path = &row.0[0];
                if visible.contains(module_feature)
                    || (feature_set == "bsn" && path == "bevy_ui_router::bsn")
                {
                    lines.push(format!("pub use {path} as UsedModule{index};"));
                    index += 1;
                }
            }
            for row in symbols {
                if visible.contains(row.feature.as_str()) {
                    let path = format!("bevy_ui_router::{}::{}", row.module, row.symbol);
                    lines.push(format!("pub use {path} as UsedSymbol{index};"));
                    index += 1;
                    if row.prelude {
                        lines.push(format!(
                            "pub use bevy_ui_router::prelude::{} as UsedPrelude{index};",
                            row.symbol
                        ));
                        index += 1;
                    }
                }
            }
            lines.push("fn contract_value<T>() -> T { panic!(\"compile-only value\") }".to_owned());
            for (golden_index, row) in golden.iter().enumerate() {
                if visible.contains(row.feature.as_str()) {
                    if matches!(row.kind.as_str(), "method" | "trait-method") {
                        lines.push(method_compile_probe(row, golden_index));
                    } else if row.kind == "compile-assert" {
                        lines.push(compile_assert_probe(row, golden_index));
                    }
                }
            }
            assert!(index > 0, "positive case must reference public paths");
            ConsumerCase {
                kind: "pass",
                feature_set,
                slug: "all-public-paths".to_owned(),
                default_features,
                features,
                source: lines.join("\n"),
            }
        })
        .collect()
}

fn method_compile_probe(row: &GoldenRow, index: usize) -> String {
    use syn::__private::ToTokens;

    let parsed = syn::parse_file(&format!(
        "struct ContractProbe; impl ContractProbe {{ {} {{ panic!(\"compile only\") }} }}",
        row.api
    ))
    .unwrap_or_else(|error| panic!("failed to parse golden method '{}': {error}", row.api));
    let method = match &parsed.items[1] {
        syn::Item::Impl(item) => match &item.items[0] {
            syn::ImplItem::Fn(method) => method,
            _ => panic!("golden associated record did not parse as a method"),
        },
        _ => panic!("golden associated record did not parse as an impl"),
    };
    let method_name = &method.sig.ident;
    let mut wrapper_generics = Vec::new();
    let target = if matches!(
        path_symbol(&row.path),
        "EcsViewReportSession" | "FrameContentWriter"
    ) {
        wrapper_generics.push("'a".to_owned());
        format!("{}<'a>", row.path)
    } else {
        row.path.clone()
    };
    for parameter in &method.sig.generics.params {
        wrapper_generics.push(parameter.to_token_stream().to_string());
    }
    let method_type_parameters = method
        .sig
        .generics
        .type_params()
        .map(|parameter| parameter.ident.to_string())
        .collect::<BTreeSet<_>>();

    let receiver = method.sig.inputs.iter().find_map(|input| match input {
        syn::FnArg::Receiver(receiver) => Some(receiver),
        syn::FnArg::Typed(_) => None,
    });
    if row.kind == "trait-method" {
        wrapper_generics.insert(0, format!("ContractReceiver: {}", row.path));
    }
    let generics = if wrapper_generics.is_empty() {
        String::new()
    } else {
        format!("<{}>", wrapper_generics.join(", "))
    };
    let where_clause = method
        .sig
        .generics
        .where_clause
        .as_ref()
        .map(ToTokens::to_token_stream)
        .map(|tokens| tokens.to_string())
        .unwrap_or_default();
    let arguments = method
        .sig
        .inputs
        .iter()
        .filter_map(|input| match input {
            syn::FnArg::Receiver(_) => None,
            syn::FnArg::Typed(argument) => {
                let ty = compact(&argument.ty.to_token_stream().to_string());
                Some(if ty.starts_with("implInto<String>") {
                    "String::new()".to_owned()
                } else if method_type_parameters.contains(&ty) {
                    format!("contract_value::<{ty}>()")
                } else {
                    "contract_value()".to_owned()
                })
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let call = if receiver.is_some() {
        format!("contract_receiver.{method_name}({arguments})")
    } else {
        format!("{target}::{method_name}({arguments})")
    };
    let receiver_parameter = if row.kind == "trait-method" {
        receiver.map(|_| "mut contract_receiver: ContractReceiver, ".to_owned())
    } else {
        receiver.map(|_| format!("mut contract_receiver: {target}, "))
    }
    .unwrap_or_default();
    format!(
        "#[allow(dead_code, unused_mut)] fn contract_method_{index}{generics}({receiver_parameter}) {where_clause} {{ let _ = {call}; }}"
    )
}

fn compile_assert_probe(row: &GoldenRow, index: usize) -> String {
    let bound = row.api.replacen("Self", &row.path, 1);
    format!(
        "#[allow(dead_code)] fn contract_bound_{index}() {{ fn require<T>() where {bound} {{}} require::<{}>(); }}",
        row.path
    )
}

pub fn forbidden_name_cases(rows: &[TableRow]) -> Vec<NegativeCase> {
    rows.iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let symbol = &row.0[0];
            let module = &row.0[1];
            if symbol.contains("::")
                || symbol.contains('{')
                || symbol.contains('/')
                || symbol.contains("->")
                || symbol.contains(':')
            {
                return None;
            }
            let feature = match module.as_str() {
                "view" => Some("view"),
                "ecs" => Some("ecs"),
                "core" => None,
                _ => return None,
            };
            Some(unresolved_case(
                &format!("forbidden-{index:03}-{}", slug(symbol)),
                feature.into_iter().map(str::to_owned).collect(),
                &format!("pub use bevy_ui_router::{module}::{symbol} as Probe;"),
            ))
        })
        .collect()
}

pub fn forbidden_capability_cases(rows: &[TableRow]) -> Vec<NegativeCase> {
    rows.iter()
        .enumerate()
        .filter(|(_, row)| is_forbidden_capability(row))
        .flat_map(|(index, row)| {
            let name = row.0[0].as_str();
            capability_fixtures(name).into_iter().enumerate().map(
                move |(fixture_index, (features, source, codes))| NegativeCase {
                    consumer: ConsumerCase {
                        kind: "fail",
                        feature_set: feature_set_name(&features),
                        slug: format!(
                            "forbidden-capability-{index:03}-{fixture_index:02}-{}",
                            slug(name)
                        ),
                        default_features: false,
                        features,
                        source,
                    },
                    expected_codes: codes,
                },
            )
        })
        .collect()
}

pub fn is_forbidden_capability(row: &TableRow) -> bool {
    let symbol = &row.0[0];
    symbol.contains("::")
        || symbol.contains('{')
        || symbol.contains('/')
        || symbol.contains("->")
        || symbol.contains(':')
}

type CapabilityFixture = (Vec<String>, String, Vec<&'static str>);

fn capability_fixtures(name: &str) -> Vec<CapabilityFixture> {
    let ecs = || vec!["ecs".to_owned()];
    let audit = || vec!["test-support".to_owned()];
    let one = |features: Vec<String>, source: &str, codes: Vec<&'static str>| {
        vec![(features, source.to_owned(), codes)]
    };
    let methods = |features: Vec<String>,
                   receiver: &str,
                   names: &[&str],
                   codes: Vec<&'static str>| {
        names
            .iter()
            .map(|method| {
                (
                    features.clone(),
                    format!(
                        "use bevy_ui_router::prelude::*; fn probe({receiver}) {{ let _ = value.{method}(); }}"
                    ),
                    codes.clone(),
                )
            })
            .collect()
    };
    match name {
        "EcsViewReportSession::new" => one(
            ecs(),
            "use bevy_ui_router::prelude::*; fn probe() { let _ = EcsViewReportSession::new; }",
            vec!["E0624"],
        ),
        "FrameContentWriter::spawn" => one(
            ecs(),
            "use bevy_ui_router::prelude::*; fn probe(value: &mut FrameContentWriter<'_>) { let _ = value.spawn(()); }",
            vec!["E0599", "E0624"],
        ),
        "FrameContentWriter::insert" => one(
            ecs(),
            "use bevy_ui_router::prelude::*; fn probe(value: &mut FrameContentWriter<'_>, entity: ContentEntity) { let _ = value.insert(entity, ()); }",
            vec!["E0599", "E0624"],
        ),
        "FrameContentWriter::world_mut" => methods(
            ecs(),
            "value: &mut FrameContentWriter<'_>",
            &["world_mut"],
            vec!["E0599"],
        ),
        "FrameContentWriter::commands" => methods(
            ecs(),
            "value: &mut FrameContentWriter<'_>",
            &["commands"],
            vec!["E0599"],
        ),
        "ContentEntity::{raw,entity,from_raw}" => id_method_fixtures("ContentEntity", ecs(), &["raw", "entity", "from_raw"]),
        "AuditEntityId::{raw,entity,from_raw}" => id_method_fixtures("AuditEntityId", audit(), &["raw", "entity", "from_raw"]),
        "QuarantineLeaseId::{raw,from_raw}" => id_method_fixtures("QuarantineLeaseId", audit(), &["raw", "from_raw"]),
        "RouterHandle::{raw,from_raw}" => id_method_fixtures("RouterHandle", ecs(), &["raw", "from_raw"]),
        "NavigationRequestId::{raw,from_raw}" => id_method_fixtures("NavigationRequestId", ecs(), &["raw", "from_raw"]),
        "ContentEntity/AuditEntityId -> Entity" => [
            ("ContentEntity", ecs()),
            ("AuditEntityId", audit()),
        ]
        .into_iter()
        .map(|(ty, features)| (
            features,
            format!("use bevy_ui_router::prelude::*; fn probe(value: {ty}, gate: &RouterInputGate) {{ let _ = gate.allows_pointer(value.into()); }}"),
            vec!["E0277"],
        ))
        .collect(),
        "EcsViewReportSession: Clone" => one(
            ecs(),
            "use bevy_ui_router::prelude::*; fn require<T: Clone>() {} fn probe() { require::<EcsViewReportSession<'static>>(); }",
            vec!["E0277"],
        ),
        "ViewBuildReport::{new,success,failure}" => ["new", "success", "failure"]
            .into_iter()
            .map(|method| (
                ecs(),
                format!("use bevy_ui_router::prelude::*; fn probe() {{ let _ = ViewBuildReport::{method}; }}"),
                vec!["E0599"],
            ))
            .collect(),
        "ViewBuildReport: Clone/Default" => ["Clone", "Default"]
            .into_iter()
            .map(|bound| (
                ecs(),
                format!("use bevy_ui_router::prelude::*; fn require<T: {bound}>() {{}} fn probe() {{ require::<ViewBuildReport>(); }}"),
                vec!["E0277"],
            ))
            .collect(),
        "AppRouterExt::{set_active,register_authority}" => ["set_active", "register_authority"]
            .into_iter()
            .map(|method| (
                ecs(),
                format!("use bevy_ui_router::prelude::*; fn probe<T: AppRouterExt>(value: &mut T) {{ value.{method}(); }}"),
                vec!["E0599"],
            ))
            .collect(),
        "RouterNavigation::{commit,set_active,apply_report}" => methods(
            ecs(),
            "value: &mut RouterNavigation",
            &["commit", "set_active", "apply_report"],
            vec!["E0599"],
        ),
        "RouterAuthorityAuditSnapshot::{world,entity,registry}" => methods(
            audit(),
            "value: &RouterAuthorityAuditSnapshot",
            &["world", "entity", "registry"],
            vec!["E0599"],
        ),
        other => panic!("missing forbidden capability fixture: {other}"),
    }
}

fn id_method_fixtures(
    type_name: &str,
    features: Vec<String>,
    methods: &[&str],
) -> Vec<CapabilityFixture> {
    methods
        .iter()
        .map(|method| {
            let module = if type_name == "NavigationRequestId" {
                "core"
            } else {
                "ecs"
            };
            let source = if *method == "from_raw" {
                format!("use bevy_ui_router::{module}::{type_name}; fn probe() {{ let _ = {type_name}::from_raw(0); }}")
            } else {
                format!("use bevy_ui_router::{module}::{type_name}; fn probe(value: {type_name}) {{ let _ = value.{method}(); }}")
            };
            (features.clone(), source, vec!["E0432", "E0599"])
        })
        .collect()
}

pub fn forbidden_module_cases(rows: &[TableRow]) -> Vec<NegativeCase> {
    let mut cases = Vec::new();
    for row in rows {
        let expression = &row.0[0];
        let (module, names) = expression
            .split_once("::{")
            .map(|(module, names)| (module, names.trim_end_matches('}')))
            .or_else(|| expression.split_once("::"))
            .unwrap_or_else(|| panic!("invalid forbidden module row: {expression}"));
        for name in names.split(',') {
            cases.push(unresolved_case(
                &format!("forbidden-module-{module}-{}", slug(name)),
                match module {
                    "view" => vec!["view".to_owned()],
                    "ecs" => vec!["ecs".to_owned()],
                    _ => Vec::new(),
                },
                &format!("pub use bevy_ui_router::{module}::{name} as Probe;"),
            ));
        }
    }
    cases
}

pub fn profile_exclusion_cases(rows: &[TableRow]) -> Vec<NegativeCase> {
    rows.iter()
        .enumerate()
        .map(|(index, row)| match row.0[0].as_str() {
            "symbol" => unresolved_case(
                &format!("profile-symbol-{index:02}-{}", slug(&row.0[1])),
                vec!["test-support".to_owned()],
                &format!("pub use {} as Probe;", row.0[1]),
            ),
            "associated" => associated_exclusion_case(index, &row.0[1]),
            kind => panic!("unknown profile exclusion kind: {kind}"),
        })
        .collect()
}

pub fn feature_disabled_cases(
    module_rows: &[TableRow],
    symbols: &[SymbolRow],
) -> Vec<NegativeCase> {
    let mut cases = Vec::new();
    for (index, row) in module_rows.iter().enumerate() {
        let feature = row.0[1].trim_matches(char::from(96));
        if feature == "default" {
            continue;
        }
        cases.push(unresolved_case(
            &format!("disabled-module-{index:02}-{}", slug(&row.0[0])),
            weaker_features(feature),
            &format!("pub use {} as Probe;", row.0[0]),
        ));
    }
    for (index, row) in symbols.iter().enumerate() {
        if row.feature == "default" {
            continue;
        }
        cases.push(unresolved_case(
            &format!("disabled-symbol-{index:03}-{}", slug(&row.symbol)),
            weaker_features(&row.feature),
            &format!(
                "pub use bevy_ui_router::{}::{} as Probe;",
                row.module, row.symbol
            ),
        ));
        if row.prelude {
            cases.push(unresolved_case(
                &format!("disabled-prelude-{index:03}-{}", slug(&row.symbol)),
                weaker_features(&row.feature),
                &format!("pub use bevy_ui_router::prelude::{} as Probe;", row.symbol),
            ));
        }
    }
    cases
}

pub fn opaque_struct_cases(golden: &[GoldenRow]) -> Vec<NegativeCase> {
    let shapes = public_struct_shapes(golden);
    let mut cases = Vec::new();
    for (index, row) in golden.iter().filter(|row| row.kind == "struct").enumerate() {
        if row.path == "bevy_ui_router::ecs::BevyUiRouterPlugin" {
            continue;
        }
        let key = (row.owner.clone(), path_symbol(&row.path).to_owned());
        let shape = shapes[&key];
        let features = consumer_features(&row.feature);
        cases.push(NegativeCase {
            consumer: ConsumerCase {
                kind: "fail",
                feature_set: feature_set_name(&features),
                slug: format!("opaque-value-{index:03}-{}", slug(&row.path)),
                default_features: false,
                features: features.clone(),
                source: format!("fn probe() {{ let _ = {}; }}", row.path),
            },
            expected_codes: vec!["E0423", "E0603"],
        });
        if matches!(shape, PublicStructShape::Named { .. }) {
            cases.push(NegativeCase {
                consumer: ConsumerCase {
                    kind: "fail",
                    feature_set: feature_set_name(&features),
                    slug: format!("opaque-literal-{index:03}-{}", slug(&row.path)),
                    default_features: false,
                    features,
                    source: format!("fn probe() {{ let _ = {} {{}}; }}", row.path),
                },
                expected_codes: vec!["E0063", "E0451"],
            });
        }
    }
    assert!(
        !cases.is_empty(),
        "opaque struct consumer catalogue is empty"
    );
    cases
}

fn consumer_features(feature: &str) -> Vec<String> {
    match feature {
        "default" => Vec::new(),
        "view" => vec!["view".to_owned()],
        "ecs" => vec!["ecs".to_owned()],
        "ecs-test-support" => vec!["test-support".to_owned()],
        "platform" => vec!["platform".to_owned()],
        other => panic!("unknown consumer feature: {other}"),
    }
}

fn unresolved_case(slug_value: &str, features: Vec<String>, source: &str) -> NegativeCase {
    NegativeCase {
        consumer: ConsumerCase {
            kind: "fail",
            feature_set: feature_set_name(&features),
            slug: slug_value.to_owned(),
            default_features: false,
            features,
            source: source.to_owned(),
        },
        expected_codes: vec!["E0432", "E0433", "E0603"],
    }
}

fn weaker_features(feature: &str) -> Vec<String> {
    match feature {
        "view" | "platform" => Vec::new(),
        "ecs" | "bsn" => vec!["view".to_owned()],
        "ecs-test-support" | "test-support" => vec!["ecs".to_owned()],
        other => panic!("unknown feature predecessor: {other}"),
    }
}

fn feature_set_name(features: &[String]) -> &'static str {
    match features {
        [] => "core-only",
        [feature] if feature == "view" => "view",
        [feature] if feature == "ecs" => "ecs",
        [feature] if feature == "test-support" => "test-support",
        _ => "custom",
    }
}

fn slug(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
        } else if !output.ends_with('-') {
            output.push('-');
        }
    }
    output.trim_matches('-').to_owned()
}

fn associated_exclusion_case(index: usize, path: &str) -> NegativeCase {
    let (feature, source) = if path.ends_with("FrameContentWriter::declare_outlet") {
        (
            "ecs",
            "use bevy_ui_router::prelude::*; fn probe(writer: &mut FrameContentWriter<'_>, entity: ContentEntity, slot: RouteSlotKey) { let _ = writer.declare_outlet(entity, slot); }".to_owned(),
        )
    } else if path.ends_with("RouterNavigation::replace") {
        (
            "ecs",
            "use bevy_ui_router::prelude::*; fn probe(nav: &mut RouterNavigation, router: RouterHandle) { let _ = nav.replace(router, RouteLocation::new(\"/\")); }".to_owned(),
        )
    } else if path.ends_with("RouterNavigation::back") {
        (
            "ecs",
            "use bevy_ui_router::prelude::*; fn probe(nav: &mut RouterNavigation, router: RouterHandle) { let _ = nav.back(router); }".to_owned(),
        )
    } else if path.ends_with("RouteTreeBuilder::route_with_options") {
        (
            "core-only",
            "use bevy_ui_router::prelude::*; fn probe(mut builder: RouteTreeBuilder, parent: RouteId) { let _ = builder.route_with_options(parent, RoutePattern::path(\"child\").unwrap(), RouteNodeOptions::new()); }".to_owned(),
        )
    } else if path.ends_with("RouteTreeBuilder::route") {
        (
            "core-only",
            "use bevy_ui_router::prelude::*; fn probe(mut builder: RouteTreeBuilder, parent: RouteId) { let _ = builder.route(parent, RoutePattern::path(\"child\").unwrap()); }".to_owned(),
        )
    } else if path.ends_with("RouteNodeOptions::with_error_boundary") {
        (
            "core-only",
            "use bevy_ui_router::prelude::*; fn probe(options: RouteNodeOptions, key: ErrorBoundaryKey) { let _ = options.with_error_boundary(key); }".to_owned(),
        )
    } else if path.ends_with("RouteNodeOptions::with_error_view") {
        (
            "core-only",
            "use bevy_ui_router::prelude::*; fn probe(options: RouteNodeOptions, view: RouteViewKey) { let _ = options.with_error_view(view); }".to_owned(),
        )
    } else if path.ends_with("RouterAuthorityAuditSnapshot::outlets") {
        (
            "test-support",
            "use bevy_ui_router::prelude::*; fn probe(snapshot: &RouterAuthorityAuditSnapshot) { let _ = snapshot.outlets(); }".to_owned(),
        )
    } else if path.ends_with("RouterAuthorityAuditSnapshot::quarantines") {
        (
            "test-support",
            "use bevy_ui_router::prelude::*; fn probe(snapshot: &RouterAuthorityAuditSnapshot) { let _ = snapshot.quarantines(); }".to_owned(),
        )
    } else {
        panic!("missing associated exclusion fixture: {path}");
    };
    let features = match feature {
        "ecs" => vec!["ecs".to_owned()],
        "test-support" => vec!["test-support".to_owned()],
        _ => Vec::new(),
    };
    NegativeCase {
        consumer: ConsumerCase {
            kind: "fail",
            feature_set: feature,
            slug: format!("profile-associated-{index:02}-{}", slug(path)),
            default_features: false,
            features,
            source,
        },
        expected_codes: vec!["E0599", "E0624"],
    }
}

pub fn run_positive_consumer(case: &ConsumerCase) -> String {
    let case_dir = write_consumer(case);
    let manifest = case_dir.join("Cargo.toml");
    let target = consumer_root().join("cargo-target");
    run_success(
        cargo(
            &[
                "generate-lockfile",
                "--offline",
                "--config",
                "build.rustc-wrapper = ''",
                "--manifest-path",
            ],
            &manifest,
            None,
        ),
        case,
        "generate-lockfile",
    );
    let tree = run_success(
        cargo(
            &[
                "tree",
                "--locked",
                "--offline",
                "--config",
                "build.rustc-wrapper = ''",
                "-e",
                "normal",
                "--manifest-path",
            ],
            &manifest,
            None,
        ),
        case,
        "tree",
    );
    run_success(
        cargo(
            &[
                "check",
                "--locked",
                "--offline",
                "--color",
                "never",
                "--config",
                "build.rustc-wrapper = ''",
                "--manifest-path",
            ],
            &manifest,
            Some(&target),
        ),
        case,
        "check",
    );
    assert_lock_is_pinned(&case_dir.join("Cargo.lock"));
    String::from_utf8(tree.stdout).expect("cargo tree output must be UTF-8")
}

pub fn prepare_consumer_cases(kind: &str) {
    assert!(matches!(kind, "pass" | "fail"));
    let root = consumer_root().join(kind);
    assert!(
        root.starts_with(workspace_dir().join("target/facade-contract-cases")),
        "consumer cleanup escaped the fixed target root"
    );
    if root.exists() {
        fs::remove_dir_all(&root)
            .unwrap_or_else(|error| panic!("failed to clear '{}': {error}", root.display()));
    }
    fs::create_dir_all(&root)
        .unwrap_or_else(|error| panic!("failed to recreate '{}': {error}", root.display()));
}

pub fn assert_consumer_case_files(kind: &str, cases: &[ConsumerCase]) {
    assert!(matches!(kind, "pass" | "fail"));
    let root = consumer_root().join(kind);
    let expected = cases
        .iter()
        .flat_map(|case| {
            let case_root = PathBuf::from(case.feature_set).join(&case.slug);
            [
                case_root.join("Cargo.toml"),
                case_root.join("Cargo.lock"),
                case_root.join("src/lib.rs"),
            ]
        })
        .collect::<BTreeSet<_>>();
    let mut files = Vec::new();
    consumer_files(&root, &root, &mut files);
    let actual = files.into_iter().collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "generated `{kind}` consumer file set differs from the current catalogue"
    );
}

fn consumer_files(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read '{}': {error}", directory.display()))
    {
        let path = entry.unwrap().path();
        if path.is_dir() {
            consumer_files(root, &path, files);
        } else {
            files.push(path.strip_prefix(root).unwrap().to_path_buf());
        }
    }
}

pub fn run_negative_consumer(case: &NegativeCase) {
    let consumer = &case.consumer;
    let case_dir = write_consumer(consumer);
    let manifest = case_dir.join("Cargo.toml");
    let target = consumer_root().join("cargo-target");
    run_success(
        cargo(
            &[
                "generate-lockfile",
                "--offline",
                "--config",
                "build.rustc-wrapper = ''",
                "--manifest-path",
            ],
            &manifest,
            None,
        ),
        consumer,
        "generate-lockfile",
    );
    run_success(
        cargo(
            &[
                "tree",
                "--locked",
                "--offline",
                "--config",
                "build.rustc-wrapper = ''",
                "-e",
                "normal",
                "--manifest-path",
            ],
            &manifest,
            None,
        ),
        consumer,
        "tree",
    );
    let output = cargo(
        &[
            "check",
            "--locked",
            "--offline",
            "--color",
            "never",
            "--config",
            "build.rustc-wrapper = ''",
            "--manifest-path",
        ],
        &manifest,
        Some(&target),
    );
    assert!(
        !output.status.success(),
        "negative consumer unexpectedly compiled: {}/{}/{}",
        consumer.kind,
        consumer.feature_set,
        consumer.slug
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("src\\lib.rs") || stderr.contains("src/lib.rs"),
        "negative consumer failed outside its generated src/lib.rs:\n{stderr}"
    );
    let expected_category = case
        .expected_codes
        .iter()
        .any(|code| stderr.contains(&format!("error[{code}]")));
    let nightly_private_literal = consumer.slug.starts_with("opaque-literal-")
        && stderr.contains("cannot construct")
        && stderr.contains("with struct literal syntax due to private fields");
    assert!(
        expected_category || nightly_private_literal,
        "negative consumer {}/{}/{} missed expected categories {:?}:\n{stderr}",
        consumer.kind,
        consumer.feature_set,
        consumer.slug,
        case.expected_codes
    );
    for infrastructure in [
        "failed to download",
        "failed to parse manifest",
        "current package believes it's in a workspace",
        "failed to get",
        "linking with",
    ] {
        assert!(
            !stderr.contains(infrastructure),
            "negative consumer failed for infrastructure reason '{infrastructure}':\n{stderr}"
        );
    }
    assert_lock_is_pinned(&case_dir.join("Cargo.lock"));
}

fn consumer_root() -> PathBuf {
    workspace_dir().join("target/facade-contract-cases")
}

fn write_consumer(case: &ConsumerCase) -> PathBuf {
    let root = consumer_root()
        .join(case.kind)
        .join(case.feature_set)
        .join(&case.slug);
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|error| panic!("failed to create '{}': {error}", root.display()));
    let facade = facade_dir()
        .canonicalize()
        .expect("facade path must canonicalize")
        .display()
        .to_string();
    let facade = facade
        .strip_prefix(r"\\?\")
        .unwrap_or(&facade)
        .replace('\\', "/");
    let features = case
        .features
        .iter()
        .map(|feature| format!("\"{feature}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let package = format!(
        "facade-contract-{}-{}",
        case.feature_set,
        case.slug.replace('_', "-")
    );
    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{package}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n\n[dependencies]\nbevy_ui_router = {{ path = \"{facade}\", default-features = {}, features = [{features}] }}\n",
            case.default_features
        ),
    )
    .expect("consumer manifest must be writable");
    fs::write(root.join("src/lib.rs"), &case.source).expect("consumer source must be writable");
    root
}

fn cargo(prefix: &[&str], manifest: &Path, target_dir: Option<&Path>) -> Output {
    let mut command = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    command.args(prefix).arg(manifest);
    if let Some(target_dir) = target_dir {
        command.arg("--target-dir").arg(target_dir);
    }
    command.output().expect("isolated cargo command must run")
}

fn run_success(output: Output, case: &ConsumerCase, command: &str) -> Output {
    assert!(
        output.status.success(),
        "{} failed for {}/{}/{}:\n{}",
        command,
        case.kind,
        case.feature_set,
        case.slug,
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn assert_lock_is_pinned(path: &Path) {
    let lock = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read '{}': {error}", path.display()));
    for line in lock
        .lines()
        .filter(|line| line.starts_with("source = \"git+"))
    {
        if line.contains("github.com/shengyusi-SYS/bevy") {
            assert!(
                line.ends_with("#61f78f75684e6b0bc28a13a5933f8f55ca653fa7\""),
                "isolated lock contains a noncanonical Bevy revision: {line}"
            );
            assert!(!line.contains("?branch="));
        }
    }
}

pub fn public_facade_source_violations(source: &str) -> Vec<String> {
    let file = syn::parse_file(source).expect("facade source must parse");
    let mut violations = Vec::new();
    let mut modules = BTreeSet::new();
    for item in &file.items {
        match item {
            syn::Item::Mod(module) if matches!(module.vis, syn::Visibility::Public(_)) => {
                let path = module.ident.to_string();
                modules.insert(path.clone());
                inspect_module(module, &path, &mut modules, &mut violations);
            }
            syn::Item::Use(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                violations
                    .push("facade root contains a public use instead of a curated module".into());
                inspect_use_tree(&item.tree, &mut violations);
            }
            syn::Item::ExternCrate(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                violations.push("facade contains pub extern crate".into());
            }
            syn::Item::Macro(_) => {
                violations.push("facade contains a source macro that can create public API".into());
            }
            syn::Item::Impl(_) => {
                violations.push("facade root contains an impl that can create public API".into());
            }
            item if public_definition_kind(item).is_some() => violations.push(format!(
                "facade root contains public {} definition",
                public_definition_kind(item).unwrap()
            )),
            _ => {}
        }
    }
    let expected = BTreeSet::from([
        "bsn".to_owned(),
        "core".to_owned(),
        "ecs".to_owned(),
        "platform".to_owned(),
        "platform::prelude".to_owned(),
        "prelude".to_owned(),
        "view".to_owned(),
    ]);
    if modules != expected {
        violations.push(format!("public module set mismatch: {modules:?}"));
    }
    violations
}

fn inspect_module(
    module: &syn::ItemMod,
    module_path: &str,
    modules: &mut BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let Some((_, items)) = &module.content else {
        violations.push(format!("public module `{}` is not inline", module.ident));
        return;
    };
    for item in items {
        match item {
            syn::Item::Use(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                inspect_use_tree(&item.tree, violations);
            }
            syn::Item::Mod(child) if matches!(child.vis, syn::Visibility::Public(_)) => {
                let child_path = format!("{module_path}::{}", child.ident);
                modules.insert(child_path.clone());
                inspect_module(child, &child_path, modules, violations);
            }
            syn::Item::ExternCrate(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                violations.push(format!(
                    "module `{}` contains pub extern crate",
                    module.ident
                ));
            }
            syn::Item::Macro(_) => violations.push(format!(
                "module `{}` contains a source macro that can create public API",
                module_path
            )),
            syn::Item::Impl(_) => violations.push(format!(
                "module `{module_path}` contains an impl that can create public API"
            )),
            item if public_definition_kind(item).is_some() => violations.push(format!(
                "module `{module_path}` contains public {} definition",
                public_definition_kind(item).unwrap()
            )),
            _ => {}
        }
    }
}

fn public_definition_kind(item: &syn::Item) -> Option<&'static str> {
    let (visibility, kind) = match item {
        syn::Item::Const(item) => (&item.vis, "const"),
        syn::Item::Enum(item) => (&item.vis, "enum"),
        syn::Item::Fn(item) => (&item.vis, "fn"),
        syn::Item::Static(item) => (&item.vis, "static"),
        syn::Item::Struct(item) => (&item.vis, "struct"),
        syn::Item::Trait(item) => (&item.vis, "trait"),
        syn::Item::TraitAlias(item) => (&item.vis, "trait alias"),
        syn::Item::Type(item) => (&item.vis, "type"),
        syn::Item::Union(item) => (&item.vis, "union"),
        _ => return None,
    };
    is_public(visibility).then_some(kind)
}

fn inspect_use_tree(tree: &syn::UseTree, violations: &mut Vec<String>) {
    match tree {
        syn::UseTree::Glob(_) => violations.push("facade contains a public glob re-export".into()),
        syn::UseTree::Path(path) => inspect_use_tree(&path.tree, violations),
        syn::UseTree::Group(group) => {
            for item in &group.items {
                inspect_use_tree(item, violations);
            }
        }
        syn::UseTree::Rename(rename) => violations.push(format!(
            "facade contains public rename `{} as {}`",
            rename.ident, rename.rename
        )),
        syn::UseTree::Name(_) => {}
    }
}

#[cfg(test)]
mod mutation_tests {
    use super::*;

    fn contract_rows() -> (Vec<SymbolRow>, Vec<TableRow>) {
        let allowlist = read_workspace("docs/facade-export-allowlist.md");
        let profile = read_workspace("docs/root-push-contract-profile.md");
        let symbols = markdown_table(
            &allowlist,
            "## 3. Allowed Type/Value/Trait Symbols",
            &["Symbol", "Module path", "Prelude", "Required feature"],
        );
        let exclusions = markdown_table(
            &profile,
            "### 3.1 Machine-Readable Exclusions",
            &["Kind", "Canonical path or normalized item", "First slice"],
        );
        (root_symbols(&symbols, &exclusions), exclusions)
    }

    #[test]
    fn public_rename_and_intermediate_source_mutations_are_rejected() {
        let rename = "pub mod core { pub use bevy_ui_router_core::SecretAuthority as BackIntent; }";
        assert!(
            public_facade_source_violations(rename)
                .iter()
                .any(|violation| violation.contains("public rename"))
        );

        let (symbols, _) = contract_rows();
        let source = fs::read_to_string(facade_dir().join("src/lib.rs"))
            .unwrap()
            .replace(
                "bevy_ui_router_core::back::{",
                "bevy_ui_router_core::fake::{",
            );
        assert!(
            std::panic::catch_unwind(|| assert_facade_exports(&source, &symbols)).is_err(),
            "intermediate owner module mutation escaped the export gate"
        );
    }

    #[test]
    fn composite_feature_cfg_cannot_merge_devtools_with_ecs_public_surface() {
        let (symbols, _) = contract_rows();
        let original = fs::read_to_string(facade_dir().join("src/lib.rs")).unwrap();
        let source = original.replace(
            "#[cfg(feature = \"ecs\")]\npub mod ecs {",
            "#[cfg(any(feature = \"ecs\", feature = \"devtools\"))]\npub mod ecs {",
        );
        assert_ne!(
            source, original,
            "feature mutation did not modify facade source"
        );

        let rejected_by_source_gate = !public_facade_source_violations(&source).is_empty();
        let rejected_by_export_gate =
            std::panic::catch_unwind(|| assert_facade_exports(&source, &symbols)).is_err();
        assert!(
            rejected_by_source_gate || rejected_by_export_gate,
            "composite feature cfg exposed the ECS surface under devtools without rejection"
        );
    }

    #[test]
    fn absolute_and_relative_import_renames_resolve_to_the_real_private_type() {
        let catalogue = BTreeMap::from([
            (
                "RouteId".to_owned(),
                BTreeSet::from([("owner".to_owned(), "ids".to_owned(), true)]),
            ),
            (
                "SecretAuthority".to_owned(),
                BTreeSet::from([
                    ("owner".to_owned(), "secret".to_owned(), false),
                    ("owner".to_owned(), "public_api::secret".to_owned(), false),
                ]),
            ),
        ]);
        for (module, alias, import, expected_module) in [
            (
                "public_api",
                "RouteId",
                "crate::secret::SecretAuthority",
                "secret",
            ),
            (
                "public_api",
                "RouteId",
                "self::secret::SecretAuthority",
                "public_api::secret",
            ),
            (
                "public_api::nested",
                "RouteId",
                "super::secret::SecretAuthority",
                "public_api::secret",
            ),
            (
                "public_api::nested::deeper",
                "RouteId",
                "super::super::secret::SecretAuthority",
                "public_api::secret",
            ),
            (
                "public_api",
                "String",
                "secret::SecretAuthority",
                "public_api::secret",
            ),
        ] {
            let scope = SourceScope {
                module: module.to_owned(),
                imports: BTreeMap::from([(alias.to_owned(), import.to_owned())]),
            };
            let (identifier, resolved) = resolve_project_type(
                "owner",
                &scope,
                &format!("pub fn authority(&self) -> {alias}"),
                alias,
                &catalogue,
            )
            .unwrap();
            assert_eq!(identifier, "SecretAuthority", "import `{import}`");
            assert_eq!(
                resolved,
                vec![("owner".to_owned(), expected_module.to_owned(), false)],
                "import `{import}`"
            );
        }
    }

    #[test]
    fn public_signature_closure_rejects_private_aliases_before_external_leaf_filtering() {
        let catalogue = BTreeMap::from([
            (
                "RouteId".to_owned(),
                BTreeSet::from([("owner".to_owned(), "ids".to_owned(), true)]),
            ),
            (
                "SecretAuthority".to_owned(),
                BTreeSet::from([
                    ("owner".to_owned(), "secret".to_owned(), false),
                    ("owner".to_owned(), "public_api::secret".to_owned(), false),
                ]),
            ),
        ]);
        let golden = vec![GoldenRow {
            feature: "default".to_owned(),
            path: "bevy_ui_router::core::RouteId".to_owned(),
            owner: "owner".to_owned(),
            kind: "method".to_owned(),
            api: "pub fn authority(&self) -> String".to_owned(),
        }];
        for (module, import) in [
            ("public_api", "crate::secret::SecretAuthority"),
            ("public_api", "self::secret::SecretAuthority"),
            ("public_api::nested", "super::secret::SecretAuthority"),
            (
                "public_api::nested::deeper",
                "super::super::secret::SecretAuthority",
            ),
            ("public_api", "secret::SecretAuthority"),
        ] {
            let scopes = ProjectSourceScopes {
                declarations: BTreeMap::new(),
                items: BTreeMap::from([(
                    source_record(
                        "owner",
                        "RouteId",
                        "method",
                        "pub fn authority(&self) -> String",
                    ),
                    SourceScope {
                        module: module.to_owned(),
                        imports: BTreeMap::from([("String".to_owned(), import.to_owned())]),
                    },
                )]),
            };
            assert!(
                std::panic::catch_unwind(|| {
                    assert_public_signature_closure(&golden, &catalogue, &scopes)
                })
                .is_err(),
                "private alias `{import} as String` escaped the complete signature closure gate"
            );
        }
    }

    #[test]
    fn public_signature_closure_ignores_real_external_generic_types() {
        let catalogue = BTreeMap::from([
            (
                "RouteId".to_owned(),
                BTreeSet::from([("owner".to_owned(), "ids".to_owned(), true)]),
            ),
            (
                "String".to_owned(),
                BTreeSet::from([("other_owner".to_owned(), "text".to_owned(), true)]),
            ),
        ]);
        let golden = vec![GoldenRow {
            feature: "default".to_owned(),
            path: "bevy_ui_router::core::RouteId".to_owned(),
            owner: "owner".to_owned(),
            kind: "method".to_owned(),
            api: "pub fn labels(&self) -> Vec<Option<String>>".to_owned(),
        }];
        let scopes = ProjectSourceScopes {
            declarations: BTreeMap::new(),
            items: BTreeMap::from([(
                source_record(
                    "owner",
                    "RouteId",
                    "method",
                    "pub fn labels(&self) -> Vec<Option<String>>",
                ),
                SourceScope {
                    module: "public_api".to_owned(),
                    imports: BTreeMap::from([(
                        "String".to_owned(),
                        "std::string::String".to_owned(),
                    )]),
                },
            )]),
        };
        assert_public_signature_closure(&golden, &catalogue, &scopes);
    }

    #[test]
    fn associated_signature_uses_the_impl_modules_real_import_scope() {
        let source = r#"
            mod secret {
                pub(crate) struct SecretAuthority;
            }
            mod public_api {
                pub struct RouteId(u64);
            }
            mod sibling {
                use crate::secret::SecretAuthority as String;

                impl crate::public_api::RouteId {
                    pub fn authority(&self) -> String {
                        unreachable!()
                    }
                }
            }
        "#;
        let parsed = syn::parse_file(source).unwrap();
        let mut catalogue = BTreeMap::new();
        collect_project_types("owner", "", &parsed.items, &mut catalogue);
        let mut scopes = ProjectSourceScopes::default();
        collect_source_scopes("owner", "", &parsed.items, &mut scopes);
        let golden = vec![GoldenRow {
            feature: "default".to_owned(),
            path: "bevy_ui_router::core::RouteId".to_owned(),
            owner: "owner".to_owned(),
            kind: "method".to_owned(),
            api: "pub fn authority(&self) -> String".to_owned(),
        }];

        assert!(
            std::panic::catch_unwind(|| {
                assert_public_signature_closure(&golden, &catalogue, &scopes)
            })
            .is_err(),
            "private alias in a sibling impl module escaped associated signature closure"
        );
    }

    #[test]
    fn source_scope_rejects_duplicate_aliases_from_every_use_shape() {
        let conditional_private =
            "#[cfg(feature = \"test-support\")] use crate::secret::SecretAuthority as String;";
        let conditional_external =
            "#[cfg(not(feature = \"test-support\"))] use std::string::String;";
        let import_cases = [
            format!("{conditional_private}\n{conditional_external}"),
            format!("{conditional_external}\n{conditional_private}"),
            "use crate::{secret::SecretAuthority as String, alternate::AlternateString as String};"
                .to_owned(),
            "use crate::secret::String;\nuse std::string::String;".to_owned(),
        ];

        for imports in import_cases {
            let source = format!(
                r#"
                    mod secret {{
                        pub(crate) struct SecretAuthority;
                        pub(crate) struct String;
                    }}
                    mod alternate {{
                        pub(crate) struct AlternateString;
                    }}
                    mod public_api {{
                        pub struct RouteId(u64);
                    }}
                    mod sibling {{
                        {imports}

                        impl crate::public_api::RouteId {{
                            pub fn authority(&self) -> impl Into<String> {{
                                unreachable!()
                            }}
                        }}
                    }}
                "#
            );
            let parsed = syn::parse_file(&source).unwrap();
            let mut scopes = ProjectSourceScopes::default();
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    collect_source_scopes("owner", "", &parsed.items, &mut scopes)
                }))
                .is_err(),
                "duplicate alias escaped source-scope construction:\n{imports}"
            );
        }
    }

    #[test]
    fn required_compile_assert_trio_cannot_disappear() {
        let (_, exclusions) = contract_rows();
        let golden = read_workspace("docs/facade-associated-api-golden.txt").replace(
            "default | bevy_ui_router::core::RouteError | bevy_ui_router_core | compile-assert | Self: Send\n",
            "",
        );
        assert!(
            std::panic::catch_unwind(|| parse_golden(&golden, &exclusions)).is_err(),
            "required error bound mutation escaped the golden parser"
        );
    }
}
