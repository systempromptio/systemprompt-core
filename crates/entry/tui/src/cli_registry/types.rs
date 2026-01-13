use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionMode {
    #[default]
    Deterministic,
    AiAssisted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliArgType {
    String,
    Bool,
    Number,
    Path,
}

impl Default for CliArgType {
    fn default() -> Self {
        Self::String
    }
}

#[derive(Debug, Clone)]
pub struct CliArgumentInfo {
    pub name: Cow<'static, str>,
    pub arg_type: CliArgType,
    pub required: bool,
    pub default_value: Option<Cow<'static, str>>,
    pub help: Cow<'static, str>,
    pub short: Option<char>,
    pub long: Option<Cow<'static, str>>,
    pub possible_values: Vec<Cow<'static, str>>,
}

impl CliArgumentInfo {
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self {
            name: name.into(),
            arg_type: CliArgType::default(),
            required: false,
            default_value: None,
            help: Cow::Borrowed(""),
            short: None,
            long: None,
            possible_values: Vec::new(),
        }
    }

    pub fn with_type(mut self, arg_type: CliArgType) -> Self {
        self.arg_type = arg_type;
        self
    }

    pub fn with_required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn with_default(mut self, default: impl Into<Cow<'static, str>>) -> Self {
        self.default_value = Some(default.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<Cow<'static, str>>) -> Self {
        self.help = help.into();
        self
    }

    pub fn with_short(mut self, short: char) -> Self {
        self.short = Some(short);
        self
    }

    pub fn with_long(mut self, long: impl Into<Cow<'static, str>>) -> Self {
        self.long = Some(long.into());
        self
    }

    pub fn with_possible_values(mut self, values: Vec<Cow<'static, str>>) -> Self {
        self.possible_values = values;
        self
    }
}

#[derive(Debug, Clone)]
pub struct CliCommandInfo {
    pub path: Vec<Cow<'static, str>>,
    pub name: Cow<'static, str>,
    pub description: Cow<'static, str>,
    pub arguments: Vec<CliArgumentInfo>,
    pub execution_mode: ExecutionMode,
    pub subcommands: Vec<CliCommandInfo>,
}

impl CliCommandInfo {
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self {
            path: Vec::new(),
            name: name.into(),
            description: Cow::Borrowed(""),
            arguments: Vec::new(),
            execution_mode: ExecutionMode::default(),
            subcommands: Vec::new(),
        }
    }

    pub fn with_path(mut self, path: Vec<Cow<'static, str>>) -> Self {
        self.path = path;
        self
    }

    pub fn with_description(mut self, description: impl Into<Cow<'static, str>>) -> Self {
        self.description = description.into();
        self
    }

    pub fn with_arguments(mut self, arguments: Vec<CliArgumentInfo>) -> Self {
        self.arguments = arguments;
        self
    }

    pub fn with_execution_mode(mut self, mode: ExecutionMode) -> Self {
        self.execution_mode = mode;
        self
    }

    pub fn with_subcommands(mut self, subcommands: Vec<CliCommandInfo>) -> Self {
        self.subcommands = subcommands;
        self
    }

    pub fn full_path(&self) -> String {
        self.path.join(" ")
    }

    pub fn has_subcommands(&self) -> bool {
        !self.subcommands.is_empty()
    }

    pub fn is_executable(&self) -> bool {
        self.subcommands.is_empty()
    }

    pub fn required_arguments(&self) -> impl Iterator<Item = &CliArgumentInfo> {
        self.arguments.iter().filter(|arg| arg.required)
    }

    pub fn optional_arguments(&self) -> impl Iterator<Item = &CliArgumentInfo> {
        self.arguments.iter().filter(|arg| !arg.required)
    }
}

#[derive(Debug, Clone)]
pub enum CommandTreeItem {
    Domain {
        name: Cow<'static, str>,
        path: String,
        is_expanded: bool,
        child_count: usize,
        depth: usize,
    },
    Command {
        info: CliCommandInfo,
        depth: usize,
    },
}

impl CommandTreeItem {
    pub fn depth(&self) -> usize {
        match self {
            Self::Domain { depth, .. } | Self::Command { depth, .. } => *depth,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Domain { name, .. } => name,
            Self::Command { info, .. } => &info.name,
        }
    }

    pub fn is_domain(&self) -> bool {
        matches!(self, Self::Domain { .. })
    }

    pub fn is_command(&self) -> bool {
        matches!(self, Self::Command { .. })
    }
}
