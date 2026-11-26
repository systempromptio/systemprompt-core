//! The manifest an instance serves, and the ways it can be made wrong.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManifestFixture {
    fault: Option<ManifestFault>,
}

impl ManifestFixture {
    pub fn minimal() -> Self {
        Self::default()
    }

    pub fn with_fault(mut self, fault: ManifestFault) -> Self {
        self.fault = Some(fault);
        self
    }

    pub fn fault(&self) -> Option<ManifestFault> {
        self.fault
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestFault {
    StaleVersion,
    FutureNotBefore,
    WrongUser,
    ChangedKey,
    BadSignature,
}
