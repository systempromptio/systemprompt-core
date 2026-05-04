//! API-key identifier and its companion secret token.

use crate::{define_id, define_token};

define_token!(ApiKeySecret);
define_id!(ApiKeyId, generate, schema);
