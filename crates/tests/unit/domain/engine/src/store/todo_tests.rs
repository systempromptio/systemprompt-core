use systemprompt_engine::store::{
    Change, SessionId, Store, StoreError, Todo, TodoPriority, TodoStatus,
};

use crate::store::support::{TestResult, session};

#[test]
fn todos_are_replaced_whole_and_published() -> TestResult {
    let store = Store::open_in_memory()?;
    let created = session(&store)?;
    let first = vec![
        Todo {
            content: "a".into(),
            status: TodoStatus::Pending,
            priority: TodoPriority::High,
        },
        Todo {
            content: "b".into(),
            status: TodoStatus::InProgress,
            priority: TodoPriority::Medium,
        },
    ];
    store.replace_todos(&created.id, &first)?;
    assert_eq!(store.todos(&created.id)?, first);

    let mut feed = store.subscribe();
    let second = vec![Todo {
        content: "c".into(),
        status: TodoStatus::Completed,
        priority: TodoPriority::Low,
    }];
    store.replace_todos(&created.id, &second)?;
    assert_eq!(store.todos(&created.id)?, second);
    let Change::TodosUpdated { todos, .. } = feed.try_recv()? else {
        return Err("expected todos".into());
    };
    assert_eq!(todos, second);
    assert!(matches!(
        store.replace_todos(&SessionId::mint(store.ids()), &second),
        Err(StoreError::NotFound { .. })
    ));
    Ok(())
}
