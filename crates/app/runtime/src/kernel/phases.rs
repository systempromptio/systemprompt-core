//! The resolution phases of [`Kernel::resolve`]: running the provide fns of
//! one phase, building the slots, inserting the governance surfaces and
//! checking every member's declared requirements.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_extension::contribute::{SlotBuildInput, build_slot};
use systemprompt_extension::{Admitted, BuiltState, ProvideError, ProvidePhase, Resolved};
use systemprompt_governance::authz::SubjectProviderSet;
use systemprompt_governance::policy::PolicyCatalog;

use super::reserved::gateway_guards;
use super::{Kernel, KernelError};

impl Kernel {
    pub(super) fn provide_all(
        &mut self,
        admitted: &Admitted,
        phase: ProvidePhase,
    ) -> Result<(), KernelError> {
        for member in admitted.built().members() {
            for entry in member
                .sink
                .provides()
                .iter()
                .filter(|entry| entry.phase() == phase)
            {
                let scope = member.scope();
                match entry.compute(&self.member_resolved(&scope)) {
                    Ok(value) => value.insert_into(&mut self.state, &member.owner)?,
                    Err(ProvideError::Withheld) => entry.withhold_into(&mut self.state),
                    Err(source) => {
                        return Err(KernelError::Provide {
                            owner: member.owner.clone(),
                            ty: entry.type_name(),
                            source,
                        });
                    },
                }
            }
        }
        Ok(())
    }

    pub(super) fn check_requirements(&self, admitted: &Admitted) -> Result<(), KernelError> {
        for member in admitted.built().members() {
            if let Some(unmet) = member
                .sink
                .requires()
                .iter()
                .find(|requirement| !requirement.is_met_by(&self.state))
            {
                return Err(KernelError::Missing {
                    owner: member.owner.clone(),
                    ty: unmet.type_name(),
                });
            }
        }
        Ok(())
    }

    pub(super) fn build_slots(
        &mut self,
        admitted: &Admitted,
        late: bool,
    ) -> Result<(), KernelError> {
        let mut order: Vec<TypeId> = Vec::new();
        let mut grouped: HashMap<TypeId, Vec<SlotBuildInput>> = HashMap::new();
        for member in admitted.built().members() {
            for record in member
                .sink
                .slots()
                .iter()
                .filter(|record| record.late() == late)
            {
                let input = record
                    .take_entry(member.owner.clone(), member.standing)
                    .ok_or_else(|| KernelError::AlreadyResolved {
                        owner: member.owner.clone(),
                    })?;
                if !grouped.contains_key(&input.slot()) {
                    order.push(input.slot());
                }
                grouped.entry(input.slot()).or_default().push(input);
            }
        }
        for slot in order {
            let inputs = grouped.remove(&slot).unwrap_or_default();
            let built = build_slot(inputs, &Resolved::__new(&*self, &self.state))?;
            if let Some(built) = built {
                built.insert_into(&mut self.state)?;
            }
        }
        Ok(())
    }

    pub(super) fn build_governance(&mut self, built: &BuiltState) -> Result<(), KernelError> {
        let r = Resolved::__new(&*self, &self.state);
        let subjects = SubjectProviderSet::from_admitted(built, &r)?;
        let policies = PolicyCatalog::from_admitted(built, &r)?;
        let guards = gateway_guards(built, &r)?;
        self.state.insert(Arc::new(subjects));
        self.state.insert(Arc::new(policies));
        self.state.insert(Arc::new(guards));
        Ok(())
    }
}
