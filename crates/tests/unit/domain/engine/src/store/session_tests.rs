use std::collections::HashSet;

use systemprompt_engine::store::{
    Change, MessageId, MessagePage, NewSession, RevertMarker, Store, StoreError, Todo,
    TodoPriority, TodoStatus,
};

use crate::store::support::{TestResult, assistant, session, text_of, user};

#[test]
fn fork_copies_history_before_the_cut_with_fresh_ids() -> TestResult {
    let store = Store::open_in_memory()?;
    let source = session(&store)?;
    let first = user(&store, &source.id, "one")?;
    assistant(&store, &source.id, &first, "reply one")?;
    let second = user(&store, &source.id, "two")?;
    assistant(&store, &source.id, &second, "reply two")?;
    let third = user(&store, &source.id, "three")?;
    store.replace_todos(
        &source.id,
        &[Todo {
            content: "x".into(),
            status: TodoStatus::Pending,
            priority: TodoPriority::Low,
        }],
    )?;

    let fork = store.fork_session(&source.id, Some(&third))?;
    assert_ne!(fork.id, source.id);
    assert_eq!(fork.project_id, source.project_id);
    assert!(fork.parent_id.is_none());

    let copied = store.messages(&fork.id, &MessagePage::all())?;
    assert_eq!(copied.len(), 4);
    let texts: Vec<&str> = copied
        .iter()
        .flat_map(|entry| text_of(&entry.parts))
        .collect();
    assert_eq!(texts, vec!["one", "reply one", "two", "reply two"]);
    let copied_ids: HashSet<&MessageId> = copied.iter().map(|entry| entry.message.id()).collect();
    assert!(!copied_ids.contains(&first) && !copied_ids.contains(&second));
    let Some(reply) = copied[3].message.as_assistant() else {
        return Err("expected an assistant message".into());
    };
    assert_eq!(&reply.parent_id, copied[2].message.id());
    assert!(
        copied
            .iter()
            .flat_map(|entry| &entry.parts)
            .all(|part| part.session_id == fork.id)
    );
    assert_eq!(store.todos(&fork.id)?, Vec::<Todo>::new());

    assert_eq!(store.messages(&source.id, &MessagePage::all())?.len(), 5);
    assert_eq!(
        store
            .messages(
                &store.fork_session(&source.id, None)?.id,
                &MessagePage::all()
            )?
            .len(),
        5
    );

    let other = session(&store)?;
    let foreign = user(&store, &other.id, "elsewhere")?;
    assert!(matches!(
        store.fork_session(&source.id, Some(&foreign)),
        Err(StoreError::NotFound { .. })
    ));
    Ok(())
}

#[test]
fn deleting_a_session_removes_children_and_contents() -> TestResult {
    let store = Store::open_in_memory()?;
    let root = session(&store)?;
    let mut child_new = NewSession::new(root.project_id.clone(), "/work/demo");
    child_new.parent_id = Some(root.id.clone());
    let child = store.create_session(child_new)?;
    let mut grandchild_new = NewSession::new(root.project_id.clone(), "/work/demo");
    grandchild_new.parent_id = Some(child.id.clone());
    let grandchild = store.create_session(grandchild_new)?;
    let keep = session(&store)?;

    let root_message = user(&store, &root.id, "root")?;
    let grandchild_message = user(&store, &grandchild.id, "deep")?;
    let deep_parts = store.parts(&grandchild_message)?;
    store.replace_todos(
        &child.id,
        &[Todo {
            content: "t".into(),
            status: TodoStatus::InProgress,
            priority: TodoPriority::High,
        }],
    )?;
    let kept_message = user(&store, &keep.id, "kept")?;

    assert_eq!(store.root_sessions(&root.project_id, false)?.len(), 2);
    assert_eq!(store.child_sessions(&root.id)?.len(), 1);

    let mut feed = store.subscribe();
    assert!(store.delete_session(&root.id)?);
    assert!(!store.delete_session(&root.id)?);

    for id in [&root.id, &child.id, &grandchild.id] {
        assert!(store.get_session(id)?.is_none());
    }
    assert!(store.get_message(&root_message)?.is_none());
    assert!(store.get_message(&grandchild_message)?.is_none());
    assert!(store.get_part(&deep_parts[0].id)?.is_none());
    assert_eq!(store.todos(&child.id)?, Vec::<Todo>::new());
    assert!(store.get_message(&kept_message)?.is_some());

    let mut deleted = Vec::new();
    while let Ok(change) = feed.try_recv() {
        if let Change::SessionDeleted { id, .. } = change {
            deleted.push(id);
        }
    }
    assert_eq!(deleted, vec![grandchild.id, child.id, root.id]);
    Ok(())
}

#[test]
fn revert_marker_sets_and_clears() -> TestResult {
    let store = Store::open_in_memory()?;
    let created = session(&store)?;
    let first = user(&store, &created.id, "one")?;
    let second = user(&store, &created.id, "two")?;

    let marker = RevertMarker {
        message_id: second.clone(),
        part_id: None,
        snapshot: Some("abc".into()),
        diff: None,
    };
    let reverted = store.set_revert(&created.id, marker.clone())?;
    assert_eq!(reverted.revert, Some(marker));
    assert_eq!(
        store.get_session(&created.id)?.ok_or("missing")?.revert,
        reverted.revert
    );

    let cleared = store.clear_revert(&created.id)?;
    assert!(cleared.revert.is_none());

    let removed = store.delete_messages_from(&created.id, &second)?;
    assert_eq!(removed, vec![second]);
    let remaining = store.messages(&created.id, &MessagePage::all())?;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].message.id(), &first);
    Ok(())
}

#[test]
fn sessions_are_found_by_title() -> TestResult {
    let store = Store::open_in_memory()?;
    let alpha = session(&store)?;
    let beta = session(&store)?;
    store.set_title(&alpha.id, "Fix 100% of the parser")?;
    store.set_title(&beta.id, "Refactor the Parser cache")?;

    assert_eq!(store.search_sessions(None, "parser", 10)?.len(), 2);
    let percent = store.search_sessions(Some(&alpha.project_id), "100%", 10)?;
    assert_eq!(percent.len(), 1);
    assert_eq!(percent[0].id, alpha.id);
    assert_eq!(
        store.search_sessions(None, "1_0", 10)?,
        Vec::<systemprompt_engine::store::Session>::new()
    );

    store.set_archived(&beta.id, true)?;
    assert_eq!(store.search_sessions(None, "parser", 10)?.len(), 1);
    assert_eq!(store.root_sessions(&alpha.project_id, false)?.len(), 1);
    assert_eq!(store.root_sessions(&alpha.project_id, true)?.len(), 2);
    Ok(())
}
