use std::sync::mpsc;

use systemprompt_bridge::update::UpdateError;
use systemprompt_bridge::update::completion::Completion;

fn channel() -> (
    Completion<u32, UpdateError>,
    mpsc::Receiver<Result<u32, UpdateError>>,
) {
    let (sender, receiver) = mpsc::channel();
    let completion = Completion::new(move |result| {
        sender.send(result).expect("receiver alive");
    });
    (completion, receiver)
}

#[test]
fn a_finished_install_delivers_its_outcome_once() {
    let (completion, outcomes) = channel();
    completion.finish(Ok(7));
    assert!(matches!(outcomes.recv(), Ok(Ok(7))));
    assert!(outcomes.try_recv().is_err(), "nothing is delivered twice");
}

#[test]
fn an_install_task_dropped_before_finishing_reports_that_it_ended() {
    let (completion, outcomes) = channel();
    drop(completion);
    assert!(matches!(
        outcomes.recv(),
        Ok(Err(UpdateError::InstallTaskEnded))
    ));
}

#[test]
fn an_install_task_that_panics_still_reports_an_outcome() {
    let (completion, outcomes) = channel();
    let joined = std::thread::spawn(move || {
        let _owned = completion;
        panic!("install task failed");
    })
    .join();
    assert!(joined.is_err(), "the task panicked");
    assert!(matches!(
        outcomes.recv(),
        Ok(Err(UpdateError::InstallTaskEnded))
    ));
    assert!(
        UpdateError::InstallTaskEnded
            .user_message()
            .contains("Try again"),
        "{}",
        UpdateError::InstallTaskEnded.user_message()
    );
}
