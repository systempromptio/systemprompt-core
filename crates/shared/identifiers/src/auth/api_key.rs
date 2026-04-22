use crate::{define_id, define_token};

define_token!(ApiKeySecret);
define_id!(ApiKeyId, generate, schema);
