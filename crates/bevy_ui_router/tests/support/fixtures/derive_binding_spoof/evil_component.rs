extern crate proc_macro;

use proc_macro::TokenStream;

#[proc_macro_derive(Component, attributes(component))]
pub fn derive_component(_: TokenStream) -> TokenStream {
    r#"
        impl RouterHandle {
            pub fn glob_authority(&self) {}
            pub fn extern_authority(&self) {}
        }
    "#
    .parse()
    .unwrap()
}
