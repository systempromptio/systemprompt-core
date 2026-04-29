use crate::ids::BearerToken;

pub type Secret = BearerToken;

impl BearerToken {
    pub fn expose(&self) -> &str {
        self.as_str()
    }

    pub fn is_empty(&self) -> bool {
        self.as_str().is_empty()
    }

    pub fn len(&self) -> usize {
        self.as_str().len()
    }
}

impl Default for BearerToken {
    fn default() -> Self {
        Self::new(String::new())
    }
}
