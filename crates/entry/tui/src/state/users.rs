use std::time::Instant;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::UserId;
use systemprompt_models::admin::UserInfo;
use systemprompt_models::BaseRoles;

#[derive(Debug, Clone)]
pub struct UserDisplay {
    pub id: UserId,
    pub name: String,
    pub sessions: i64,
    pub last_accessed: Option<DateTime<Utc>>,
    pub roles: Vec<String>,
}

impl From<UserInfo> for UserDisplay {
    fn from(u: UserInfo) -> Self {
        Self {
            id: u.id,
            name: u.name,
            sessions: u.active_sessions,
            last_accessed: u.last_session_at,
            roles: u.roles,
        }
    }
}

pub const SYSTEM_USER_ID: &str = "00000000-0000-0000-0000-000000000001";

impl UserDisplay {
    pub fn role_display(&self) -> String {
        if self.roles.is_empty() {
            "none".to_string()
        } else {
            self.roles.join(", ")
        }
    }

    pub fn is_admin(&self) -> bool {
        self.roles.contains(&"admin".to_string())
    }

    pub fn is_system(&self) -> bool {
        self.id.as_ref() == SYSTEM_USER_ID || self.name == "system"
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }
}

#[derive(Debug)]
pub struct UsersState {
    pub users: Vec<UserDisplay>,
    pub selected_index: usize,
    pub last_refresh: Option<Instant>,
    pub selected_role_index: usize,
}

impl UsersState {
    pub const fn new() -> Self {
        Self {
            users: Vec::new(),
            selected_index: 0,
            last_refresh: None,
            selected_role_index: 0,
        }
    }

    pub fn update_users(&mut self, users: Vec<UserDisplay>) {
        self.users = users;
        self.last_refresh = Some(Instant::now());
        if self.selected_index >= self.users.len() {
            self.selected_index = self.users.len().saturating_sub(1);
        }
    }

    pub fn select_next(&mut self) {
        if !self.users.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.users.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.users.is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(self.users.len() - 1);
        }
    }

    pub fn selected_user(&self) -> Option<&UserDisplay> {
        self.users.get(self.selected_index)
    }

    pub fn selected_user_mut(&mut self) -> Option<&mut UserDisplay> {
        self.users.get_mut(self.selected_index)
    }

    pub fn can_edit_selected(&self) -> bool {
        self.selected_user().is_some_and(|u| !u.is_system())
    }

    pub fn select_next_role(&mut self) {
        let roles = BaseRoles::available_roles();
        self.selected_role_index = (self.selected_role_index + 1) % roles.len();
    }

    pub fn select_prev_role(&mut self) {
        let roles = BaseRoles::available_roles();
        self.selected_role_index = self
            .selected_role_index
            .checked_sub(1)
            .unwrap_or(roles.len() - 1);
    }

    pub fn selected_role(&self) -> Option<&'static str> {
        BaseRoles::available_roles()
            .get(self.selected_role_index)
            .copied()
    }

    pub fn toggle_selected_role(&mut self) -> Option<(UserId, String, bool)> {
        let role = self.selected_role()?.to_string();
        let user = self.selected_user_mut()?;

        if user.is_system() {
            return None;
        }

        let user_id = user.id.clone();
        let has_role = user.has_role(&role);

        if has_role {
            user.roles.retain(|r| r != &role);
        } else {
            user.roles.push(role.clone());
        }

        Some((user_id, role, !has_role))
    }
}

impl Default for UsersState {
    fn default() -> Self {
        Self::new()
    }
}
