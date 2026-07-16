#[test]
fn bsn_facade_reserves_an_empty_module() {
    #[allow(unused_imports)]
    use bevy_ui_router::bsn;
    let _ = std::any::type_name::<fn() -> ()>();
    let _ = stringify!(bsn);
}
