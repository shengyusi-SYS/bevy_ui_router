#![allow(dead_code, non_local_definitions)]

mod hidden {
    fn container() {
        #[macro_export]
        macro_rules! leaked_trait_item {
            () => {
                fn force_private_authority(&self);
            };
        }
    }
}

pub trait AppRouterExt {
    crate::leaked_trait_item!();
}
