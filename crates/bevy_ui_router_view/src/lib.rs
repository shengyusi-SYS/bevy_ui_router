#![forbid(unsafe_code)]

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ViewBindingKey(String);

impl ViewBindingKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub mod prelude {
    pub use crate::ViewBindingKey;
}
