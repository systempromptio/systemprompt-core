#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemStatus {
    Missing,
    Applied,
    Failed,
    Valid,
    Disabled,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleType {
    Schema,
    Seed,
    Module,
    Configuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageLevel {
    Success,
    Warning,
    Error,
    Info,
}

#[derive(Debug, Clone, Copy)]
pub enum IconType {
    Status(ItemStatus),
    Module(ModuleType),
    Message(MessageLevel),
    Action(ActionType),
}

#[derive(Debug, Clone, Copy)]
pub enum ColorType {
    Status(ItemStatus),
    Message(MessageLevel),
    Emphasis(EmphasisType),
}

#[derive(Debug, Clone, Copy)]
pub enum ActionType {
    Install,
    Update,
    Arrow,
}

#[derive(Debug, Clone, Copy)]
pub enum EmphasisType {
    Highlight,
    Dim,
    Bold,
    Underlined,
}

impl From<ItemStatus> for IconType {
    fn from(status: ItemStatus) -> Self {
        Self::Status(status)
    }
}

impl From<ModuleType> for IconType {
    fn from(module_type: ModuleType) -> Self {
        Self::Module(module_type)
    }
}

impl From<MessageLevel> for IconType {
    fn from(level: MessageLevel) -> Self {
        Self::Message(level)
    }
}

impl From<ActionType> for IconType {
    fn from(action: ActionType) -> Self {
        Self::Action(action)
    }
}

impl From<ItemStatus> for ColorType {
    fn from(status: ItemStatus) -> Self {
        Self::Status(status)
    }
}

impl From<MessageLevel> for ColorType {
    fn from(level: MessageLevel) -> Self {
        Self::Message(level)
    }
}

impl From<EmphasisType> for ColorType {
    fn from(emphasis: EmphasisType) -> Self {
        Self::Emphasis(emphasis)
    }
}
