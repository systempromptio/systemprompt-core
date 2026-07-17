//! API-key identifier and its companion secret token.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::{define_id, define_token};

define_token!(ApiKeySecret);
define_id!(ApiKeyId, generate, schema);
