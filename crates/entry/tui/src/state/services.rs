use std::time::Instant;

pub use systemprompt_models::{RuntimeStatus, ServiceType};

#[derive(Debug, Clone, Copy, Eq)]
pub enum ServiceListItem {
    GroupHeader(ServiceType, usize),
    Service(usize),
}

impl PartialEq for ServiceListItem {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::GroupHeader(a, _), Self::GroupHeader(b, _)) => a == b,
            (Self::Service(a), Self::Service(b)) => a == b,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct ServicesState {
    pub services: Vec<ServiceStatus>,
    pub selected_item: ServiceListItem,
    pub last_refresh: Option<Instant>,
    pub auto_refresh: bool,
    pub expanded_groups: ExpandedGroups,
}

impl ServicesState {
    pub fn new() -> Self {
        Self {
            services: Vec::new(),
            selected_item: ServiceListItem::GroupHeader(ServiceType::Api, 0),
            last_refresh: None,
            auto_refresh: true,
            expanded_groups: ExpandedGroups::default(),
        }
    }

    pub fn update_services(&mut self, services: Vec<ServiceStatus>) {
        self.services = services;
        self.last_refresh = Some(Instant::now());
        if let ServiceListItem::Service(index) = self.selected_item {
            if index >= self.services.len() {
                let api_count = self.api_services().count();
                self.selected_item = ServiceListItem::GroupHeader(ServiceType::Api, api_count);
            }
        }
    }

    pub fn visible_items(&self) -> Vec<ServiceListItem> {
        let mut items = Vec::new();

        let api_services: Vec<_> = self.api_services_with_index().collect();
        if !api_services.is_empty() {
            items.push(ServiceListItem::GroupHeader(
                ServiceType::Api,
                api_services.len(),
            ));
            if self.expanded_groups.api {
                for (index, _) in api_services {
                    items.push(ServiceListItem::Service(index));
                }
            }
        }

        let agent_services: Vec<_> = self.agent_services_with_index().collect();
        if !agent_services.is_empty() {
            items.push(ServiceListItem::GroupHeader(
                ServiceType::Agent,
                agent_services.len(),
            ));
            if self.expanded_groups.agents {
                for (index, _) in agent_services {
                    items.push(ServiceListItem::Service(index));
                }
            }
        }

        let mcp_services: Vec<_> = self.mcp_services_with_index().collect();
        if !mcp_services.is_empty() {
            items.push(ServiceListItem::GroupHeader(
                ServiceType::Mcp,
                mcp_services.len(),
            ));
            if self.expanded_groups.mcp {
                for (index, _) in mcp_services {
                    items.push(ServiceListItem::Service(index));
                }
            }
        }

        items
    }

    pub fn select_next_visible(&mut self) {
        let items = self.visible_items();
        if items.is_empty() {
            return;
        }

        let current_index = items
            .iter()
            .position(|item| *item == self.selected_item)
            .unwrap_or(0);
        let next_index = (current_index + 1) % items.len();
        self.selected_item = items[next_index];
    }

    pub fn select_prev_visible(&mut self) {
        let items = self.visible_items();
        if items.is_empty() {
            return;
        }

        let current_index = items
            .iter()
            .position(|item| *item == self.selected_item)
            .unwrap_or(0);
        let prev_index = if current_index == 0 {
            items.len() - 1
        } else {
            current_index - 1
        };
        self.selected_item = items[prev_index];
    }

    pub fn toggle_selected_group(&mut self) {
        match self.selected_item {
            ServiceListItem::GroupHeader(group, _) => {
                self.toggle_group(group);
            },
            ServiceListItem::Service(index) => {
                if let Some(service) = self.services.get(index) {
                    self.toggle_group(service.service_type);
                }
            },
        }
    }

    pub fn collapse_selected_group(&mut self) {
        match self.selected_item {
            ServiceListItem::GroupHeader(group, _) => {
                self.set_group_expanded(group, false);
            },
            ServiceListItem::Service(index) => {
                if let Some(service) = self.services.get(index) {
                    let group = service.service_type;
                    self.set_group_expanded(group, false);
                    let count = self.count_services_in_group(group);
                    self.selected_item = ServiceListItem::GroupHeader(group, count);
                }
            },
        }
    }

    fn set_group_expanded(&mut self, group: ServiceType, expanded: bool) {
        match group {
            ServiceType::Api => self.expanded_groups.api = expanded,
            ServiceType::Agent => self.expanded_groups.agents = expanded,
            ServiceType::Mcp => self.expanded_groups.mcp = expanded,
        }
    }

    pub fn selected_service(&self) -> Option<&ServiceStatus> {
        match self.selected_item {
            ServiceListItem::Service(index) => self.services.get(index),
            ServiceListItem::GroupHeader(_, _) => None,
        }
    }

    fn count_services_in_group(&self, group: ServiceType) -> usize {
        match group {
            ServiceType::Api => self.api_services().count(),
            ServiceType::Agent => self.agent_services().count(),
            ServiceType::Mcp => self.mcp_services().count(),
        }
    }

    pub fn api_services(&self) -> impl Iterator<Item = &ServiceStatus> {
        self.services
            .iter()
            .filter(|service| service.service_type == ServiceType::Api)
    }

    pub fn agent_services(&self) -> impl Iterator<Item = &ServiceStatus> {
        self.services
            .iter()
            .filter(|service| service.service_type == ServiceType::Agent)
    }

    pub fn mcp_services(&self) -> impl Iterator<Item = &ServiceStatus> {
        self.services
            .iter()
            .filter(|service| service.service_type == ServiceType::Mcp)
    }

    fn api_services_with_index(&self) -> impl Iterator<Item = (usize, &ServiceStatus)> {
        self.services
            .iter()
            .enumerate()
            .filter(|(_, service)| service.service_type == ServiceType::Api)
    }

    fn agent_services_with_index(&self) -> impl Iterator<Item = (usize, &ServiceStatus)> {
        self.services
            .iter()
            .enumerate()
            .filter(|(_, service)| service.service_type == ServiceType::Agent)
    }

    fn mcp_services_with_index(&self) -> impl Iterator<Item = (usize, &ServiceStatus)> {
        self.services
            .iter()
            .enumerate()
            .filter(|(_, service)| service.service_type == ServiceType::Mcp)
    }

    pub fn toggle_group(&mut self, group: ServiceType) {
        match group {
            ServiceType::Api => self.expanded_groups.api = !self.expanded_groups.api,
            ServiceType::Agent => self.expanded_groups.agents = !self.expanded_groups.agents,
            ServiceType::Mcp => self.expanded_groups.mcp = !self.expanded_groups.mcp,
        }
    }

    pub const fn is_group_expanded(&self, group: ServiceType) -> bool {
        match group {
            ServiceType::Api => self.expanded_groups.api,
            ServiceType::Agent => self.expanded_groups.agents,
            ServiceType::Mcp => self.expanded_groups.mcp,
        }
    }

    pub fn select_service_by_index(&mut self, index: usize) {
        if index < self.services.len() {
            self.selected_item = ServiceListItem::Service(index);
            if let Some(service) = self.services.get(index) {
                self.set_group_expanded(service.service_type, true);
            }
        }
    }
}

impl Default for ServicesState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ExpandedGroups {
    pub api: bool,
    pub agents: bool,
    pub mcp: bool,
}

#[derive(Debug, Clone)]
pub struct ServiceStatus {
    pub name: String,
    pub service_type: ServiceType,
    pub status: RuntimeStatus,
    pub pid: Option<i32>,
    pub port: Option<u16>,
    pub uptime_secs: Option<u64>,
}

impl ServiceStatus {
    pub const fn status_symbol(&self) -> &'static str {
        match self.status {
            RuntimeStatus::Running => "●",
            RuntimeStatus::Starting => "◐",
            RuntimeStatus::Stopped => "○",
            RuntimeStatus::Crashed => "✗",
            RuntimeStatus::Orphaned => "◌",
        }
    }
}
