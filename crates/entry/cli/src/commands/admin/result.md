# Admin CLI Results

See [../result.md](../result.md) for full documentation.

## Admin Command Patterns

### Users List

```rust
Ok(CommandResult::table(UserListOutput { users, total, limit, offset })
    .with_title("Users")
    .with_columns(vec!["id", "name", "email", "status", "roles"]))
```

### Agents List

```rust
Ok(CommandResult::table(AgentListOutput { agents, total })
    .with_title("Agents")
    .with_columns(vec!["id", "name", "status", "created_at"]))
```

### Config Show

```rust
Ok(CommandResult::card(ConfigOutput { ... })
    .with_title("Configuration"))
```

## Migration Checklist

Commands to migrate from legacy pattern:

- [ ] `users list` - currently uses `CliService::json()` directly
- [ ] `users show`
- [ ] `agents list`
- [ ] `agents show`
- [ ] `config show`
- [ ] `config set`
