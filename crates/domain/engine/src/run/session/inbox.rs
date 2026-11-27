//! Prompts that arrived while a run was going, in arrival order.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::VecDeque;

use super::{Admission, Delivery};

#[derive(Debug, Default)]
pub(crate) struct Inbox {
    items: VecDeque<Admission>,
}

impl Inbox {
    pub(crate) fn push(&mut self, admission: Admission) {
        self.items.push_back(admission);
    }

    pub(crate) fn take_steers(&mut self) -> Vec<Admission> {
        let items = std::mem::take(&mut self.items);
        let (steers, rest): (VecDeque<Admission>, VecDeque<Admission>) = items
            .into_iter()
            .partition(|item| item.delivery == Delivery::Steer);
        self.items = rest;
        steers.into_iter().collect()
    }

    pub(crate) fn pop(&mut self) -> Option<Admission> {
        self.items.pop_front()
    }

    pub(crate) fn clear(&mut self) {
        self.items.clear();
    }
}
