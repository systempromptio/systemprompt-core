//! A host that serves `cloud doctor` and `cloud deploy` the target-admission
//! grant over the reference composition, as the runner does.

use systemprompt_cli::kernel::MemberHost;
use systemprompt_cli::{AdmitFn, CommandContext};
use systemprompt_extension::cli::{CommandDescriptor, KernelGrant};

pub(crate) fn reference_admit() -> AdmitFn {
    Box::new(|extensions| {
        let declared = systemprompt::reference().declare()?;
        let admitted = match extensions {
            Some(extensions) => declared.admit(extensions)?,
            None => declared.admit_unprofiled()?,
        };
        Ok(admitted)
    })
}

pub(crate) const fn target_admission() -> CommandDescriptor {
    CommandDescriptor::NONE.with_grant(KernelGrant::TargetAdmission)
}

pub(crate) fn target_host(ctx: &CommandContext) -> MemberHost<'_> {
    MemberHost::new(ctx, target_admission())
}
