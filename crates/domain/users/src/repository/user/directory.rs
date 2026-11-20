//! Subject lookups for sharing: active users matched word by word and the
//! roles active users hold.
//!
//! A search splits its text on whitespace into at most four words. A user
//! matches when every word starts some word of their display name, name,
//! email, full name or id, ignoring case and the common Latin accents; `.`,
//! `-`, `_` and `@` separate words as spaces do.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::Result;
use crate::models::{User, UserRole, UserRow, UserStatus};
use crate::repository::UserRepository;

const MAX_DIRECTORY_PAGE: i64 = 50;
const MAX_SEARCH_WORDS: usize = 4;

fn like_word(word: &str) -> String {
    word.replace(['.', '-', '_', '@'], " ")
        .trim()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
}

fn search_words(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(like_word)
        .filter(|word| !word.is_empty())
        .take(MAX_SEARCH_WORDS)
        .collect()
}

impl UserRepository {
    pub async fn search_active_by_prefix(&self, query: &str, limit: i64) -> Result<Vec<User>> {
        let words = search_words(query);
        let active = UserStatus::Active.as_str();
        let anonymous = UserRole::Anonymous.as_str();
        sqlx::query_as!(
            UserRow,
            r#"SELECT id, name, email, full_name, display_name, status, email_verified,
                      roles, avatar_url, is_bot, is_scanner, created_at, updated_at, account_kind
               FROM users
               WHERE status = $1 AND NOT ($2 = ANY(roles)) AND NOT is_bot
                 AND NOT EXISTS (
                     SELECT 1 FROM UNNEST($3::text[]) AS w(word)
                     WHERE (' ' || translate(lower(concat_ws(' ', display_name, name, email, full_name, id)), 'áàäâãåéèëêíìïîóòöôõúùüûñçÁÀÄÂÃÅÉÈËÊÍÌÏÎÓÒÖÔÕÚÙÜÛÑÇ.-_@', 'aaaaaaeeeeiiiiooooouuuuncaaaaaaeeeeiiiiooooouuuunc    '))
                           NOT LIKE ('% ' || translate(lower(w.word), 'áàäâãåéèëêíìïîóòöôõúùüûñçÁÀÄÂÃÅÉÈËÊÍÌÏÎÓÒÖÔÕÚÙÜÛÑÇ.-_@', 'aaaaaaeeeeiiiiooooouuuuncaaaaaaeeeeiiiiooooouuuunc    ') || '%')
                 )
               ORDER BY COALESCE(display_name, full_name, name), id
               LIMIT $4"#,
            active,
            anonymous,
            &words,
            limit.clamp(1, MAX_DIRECTORY_PAGE),
        )
        .fetch_all(&*self.pool)
        .await?
        .into_iter()
        .map(User::try_from)
        .collect()
    }

    pub async fn list_active_roles(&self) -> Result<Vec<String>> {
        let active = UserStatus::Active.as_str();
        let anonymous = UserRole::Anonymous.as_str();
        let roles = sqlx::query_scalar!(
            r#"SELECT DISTINCT role AS "role!" FROM users, UNNEST(roles) AS role
               WHERE status = $1 AND role <> $2 ORDER BY 1"#,
            active,
            anonymous,
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(roles)
    }
}
