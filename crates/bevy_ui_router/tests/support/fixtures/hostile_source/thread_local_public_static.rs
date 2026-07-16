std::thread_local! {
    pub static LEAKED_FACADE_STATIC: std::cell::Cell<u32> = const {
        std::cell::Cell::new(0)
    };
}

pub fn assert_api() {
    LEAKED_FACADE_STATIC.with(|value| value.set(1));
}
