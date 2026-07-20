#[path = "support/facade_contract_cases.rs"]
mod facade_contract_cases;

use std::collections::BTreeSet;

use facade_contract_cases::{
    assert_consumer_case_files, feature_disabled_cases, forbidden_capability_cases,
    forbidden_module_cases, forbidden_name_cases, history_contract_profile,
    is_forbidden_capability, markdown_table, markdown_table_by_header, opaque_struct_cases,
    parse_golden, prepare_consumer_cases, profile_exclusion_cases, read_workspace, root_symbols,
    run_negative_consumer,
};

#[test]
fn history_profile_forbidden_consumers_fail_with_expected_rustc_categories() {
    let allowlist = read_workspace("docs/facade-export-allowlist.md");
    let profile = history_contract_profile();
    let modules = markdown_table(
        &allowlist,
        "## 2. Allowed Modules",
        &["Facade path", "Required feature", "Contract"],
    );
    let symbols = markdown_table(
        &allowlist,
        "## 3. Allowed Type/Value/Trait Symbols",
        &["Symbol", "Module path", "Prelude", "Required feature"],
    );
    let forbidden = markdown_table(
        &allowlist,
        "## 4. Forbidden Symbols",
        &["Symbol", "Former/likely module", "Negative assertion"],
    );
    let forbidden_modules =
        markdown_table_by_header(&allowlist, &["Module path", "Negative assertion"], 0);
    let exclusions = profile.exclusions;
    let symbols = root_symbols(&symbols, &exclusions);
    let golden = parse_golden(
        &read_workspace("docs/facade-associated-api-golden.txt"),
        &exclusions,
    );

    let name_cases = forbidden_name_cases(&forbidden);
    let capability_cases = forbidden_capability_cases(&forbidden);
    assert_eq!(
        name_cases.len()
            + forbidden
                .iter()
                .filter(|row| is_forbidden_capability(row))
                .count(),
        forbidden.len(),
        "every forbidden row must be classified"
    );
    assert!(
        capability_cases.len()
            >= forbidden
                .iter()
                .filter(|row| is_forbidden_capability(row))
                .count(),
        "grouped forbidden capabilities must expand into independent consumers"
    );
    let module_cases = forbidden_module_cases(&forbidden_modules);
    assert!(!module_cases.is_empty());
    let profile_cases = profile_exclusion_cases(&exclusions);
    assert_eq!(profile_cases.len(), exclusions.len());
    let disabled_cases = feature_disabled_cases(&modules, &symbols);
    assert!(!disabled_cases.is_empty());
    let opaque_cases = opaque_struct_cases(&golden);
    assert!(!opaque_cases.is_empty());

    let cases = opaque_cases
        .into_iter()
        .chain(name_cases)
        .chain(capability_cases)
        .chain(module_cases)
        .chain(profile_cases)
        .chain(disabled_cases)
        .collect::<Vec<_>>();
    assert!(
        !cases.is_empty(),
        "compile-fail catalogue must not be empty"
    );
    let identities = cases
        .iter()
        .map(|case| {
            (
                case.consumer.kind,
                case.consumer.feature_set,
                case.consumer.slug.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        identities.len(),
        cases.len(),
        "compile-fail case identities must be unique"
    );
    eprintln!("facade compile-fail cases: {}", cases.len());

    prepare_consumer_cases("fail");
    for case in &cases {
        run_negative_consumer(case);
    }
    let consumers = cases
        .iter()
        .map(|case| case.consumer.clone())
        .collect::<Vec<_>>();
    assert_consumer_case_files("fail", &consumers);
}
