use std::cell::RefCell;
use std::path::Path;
use std::sync::Mutex;
use std::sync::mpsc::Sender;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{ClassType, DeclaredClass, declare_class, msg_send_id, mutability, sel};
use objc2_app_kit::{
    NSBackingStoreType, NSButton, NSColor, NSControl, NSScrollView, NSSecureTextField,
    NSStackView, NSStackViewDistribution, NSTextField, NSTextView, NSUserInterfaceLayoutOrientation,
    NSView, NSWindow, NSWindowStyleMask, NSWorkspace,
};
use objc2_foundation::{
    MainThreadMarker, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSURL,
};

use crate::gui::events::UiEvent;
use crate::gui::state::AppStateSnapshot;

const ACTION_LOGIN: i64 = 1;
const ACTION_LOGOUT: i64 = 2;
const ACTION_SYNC: i64 = 3;
const ACTION_VALIDATE: i64 = 4;
const ACTION_OPEN_FOLDER: i64 = 5;
const ACTION_CLOSE: i64 = 6;

struct TargetIvars {
    tx: Sender<UiEvent>,
    pat_field: Mutex<Option<Retained<NSSecureTextField>>>,
    gateway_field: Mutex<Option<Retained<NSTextField>>>,
    window: Mutex<Option<Retained<NSWindow>>>,
}

declare_class!(
    struct ActionTarget;

    unsafe impl ClassType for ActionTarget {
        type Super = NSObject;
        type Mutability = mutability::Mutable;
        const NAME: &'static str = "SpcoworkActionTarget";
    }

    impl DeclaredClass for ActionTarget {
        type Ivars = TargetIvars;
    }

    unsafe impl NSObjectProtocol for ActionTarget {}

    unsafe impl ActionTarget {
        #[method(handleAction:)]
        fn handle_action(&self, sender: &NSButton) {
            let tag = unsafe { sender.tag() };
            let ivars = self.ivars();
            match tag {
                ACTION_LOGIN => {
                    let token = ivars
                        .pat_field
                        .lock()
                        .unwrap()
                        .as_ref()
                        .map(|f| unsafe { f.stringValue() }.to_string())
                        .unwrap_or_default();
                    let gateway = ivars
                        .gateway_field
                        .lock()
                        .unwrap()
                        .as_ref()
                        .map(|f| unsafe { f.stringValue() }.to_string())
                        .unwrap_or_default();
                    let gateway = if gateway.trim().is_empty() {
                        None
                    } else {
                        Some(gateway)
                    };
                    let _ = ivars.tx.send(UiEvent::LoginRequested { token, gateway });
                },
                ACTION_LOGOUT => { let _ = ivars.tx.send(UiEvent::LogoutRequested); },
                ACTION_SYNC => { let _ = ivars.tx.send(UiEvent::SyncRequested); },
                ACTION_VALIDATE => { let _ = ivars.tx.send(UiEvent::ValidateRequested); },
                ACTION_OPEN_FOLDER => { let _ = ivars.tx.send(UiEvent::OpenConfigFolder); },
                ACTION_CLOSE => {
                    if let Some(w) = ivars.window.lock().unwrap().as_ref() {
                        unsafe { w.orderOut(None) };
                    }
                },
                _ => {},
            }
        }
    }
);

impl ActionTarget {
    fn new(mtm: MainThreadMarker, tx: Sender<UiEvent>) -> Retained<Self> {
        let _ = mtm;
        let this = Self::alloc().set_ivars(TargetIvars {
            tx,
            pat_field: Mutex::new(None),
            gateway_field: Mutex::new(None),
            window: Mutex::new(None),
        });
        unsafe { msg_send_id![super(this), init] }
    }
}

pub struct PlatformWindow {
    target: Retained<ActionTarget>,
    window: RefCell<Option<Retained<NSWindow>>>,
    status_view: RefCell<Option<Retained<NSTextView>>>,
    identity_label: RefCell<Option<Retained<NSTextField>>>,
    last_sync_label: RefCell<Option<Retained<NSTextField>>>,
}

unsafe impl Send for PlatformWindow {}
unsafe impl Sync for PlatformWindow {}

impl PlatformWindow {
    pub fn new(tx: Sender<UiEvent>) -> Result<Self, String> {
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| "settings window must be opened on main thread".to_string())?;
        let target = ActionTarget::new(mtm, tx);
        Ok(Self {
            target,
            window: RefCell::new(None),
            status_view: RefCell::new(None),
            identity_label: RefCell::new(None),
            last_sync_label: RefCell::new(None),
        })
    }

    pub fn show(&self, snapshot: &AppStateSnapshot) {
        let mtm = match MainThreadMarker::new() {
            Some(m) => m,
            None => return,
        };
        if self.window.borrow().is_none() {
            self.build(mtm);
        }
        if let Some(w) = self.window.borrow().as_ref() {
            unsafe {
                w.makeKeyAndOrderFront(None);
            }
        }
        self.refresh(snapshot);
    }

    pub fn refresh(&self, snap: &AppStateSnapshot) {
        if let Some(label) = self.identity_label.borrow().as_ref() {
            let id = snap
                .identity
                .clone()
                .unwrap_or_else(|| "(not signed in)".to_string());
            let s = NSString::from_str(&format!(
                "Identity: {id}\nGateway:  {}",
                snap.gateway_url
            ));
            unsafe { label.setStringValue(&s) };
        }
        if let Some(label) = self.last_sync_label.borrow().as_ref() {
            let s = NSString::from_str(&format!(
                "Last sync: {}\nPlugins dir: {}\nSkills: {}    Agents: {}",
                snap.last_sync_summary
                    .clone()
                    .unwrap_or_else(|| "never".into()),
                snap.plugins_dir.clone().unwrap_or_else(|| "?".into()),
                snap.skill_count
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "?".into()),
                snap.agent_count
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "?".into()),
            ));
            unsafe { label.setStringValue(&s) };
        }
        if let Some(view) = self.status_view.borrow().as_ref() {
            let mut text = String::new();
            if snap.sync_in_flight {
                text.push_str("Sync in progress…\n\n");
            }
            if let Some(msg) = &snap.last_action_message {
                text.push_str(msg);
                text.push_str("\n\n");
            }
            if let Some(report) = &snap.last_validation {
                text.push_str(&report.rendered());
            }
            let s = NSString::from_str(&text);
            unsafe { view.setString(&s) };
        }
    }

    pub fn close(&self) {
        if let Some(w) = self.window.borrow().as_ref() {
            unsafe { w.orderOut(None) };
        }
    }

    fn build(&self, mtm: MainThreadMarker) {
        unsafe {
            let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(560.0, 520.0));
            let style = NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Miniaturizable;
            let window = NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                frame,
                style,
                NSBackingStoreType::NSBackingStoreBuffered,
                false,
            );
            window.setTitle(&NSString::from_str("systemprompt-cowork"));
            window.setReleasedWhenClosed(false);

            let content_view = window.contentView().unwrap();

            let stack = NSStackView::initWithFrame(
                NSStackView::alloc(mtm),
                NSRect::new(NSPoint::new(16.0, 16.0), NSSize::new(528.0, 488.0)),
            );
            stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            stack.setSpacing(8.0);
            stack.setDistribution(NSStackViewDistribution::Fill);
            stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);

            let identity_label = make_label(mtm, "Identity:");
            stack.addArrangedSubview(&identity_label);
            *self.identity_label.borrow_mut() = Some(identity_label);

            let last_sync_label = make_label(mtm, "Last sync:");
            stack.addArrangedSubview(&last_sync_label);
            *self.last_sync_label.borrow_mut() = Some(last_sync_label);

            let gateway_field: Retained<NSTextField> =
                msg_send_id![NSTextField::alloc(mtm), init];
            gateway_field
                .setFrame(NSRect::new(NSPoint::ZERO, NSSize::new(528.0, 22.0)));
            gateway_field.setPlaceholderString(Some(&NSString::from_str("gateway URL")));
            stack.addArrangedSubview(&gateway_field);
            *self.target.ivars().gateway_field.lock().unwrap() = Some(gateway_field);

            let pat_field: Retained<NSSecureTextField> =
                msg_send_id![NSSecureTextField::alloc(mtm), init];
            pat_field.setFrame(NSRect::new(NSPoint::ZERO, NSSize::new(528.0, 22.0)));
            pat_field.setPlaceholderString(Some(&NSString::from_str(
                "PAT (sp-live-…)",
            )));
            stack.addArrangedSubview(&pat_field);
            *self.target.ivars().pat_field.lock().unwrap() = Some(pat_field);

            let button_row = NSStackView::initWithFrame(
                NSStackView::alloc(mtm),
                NSRect::new(NSPoint::ZERO, NSSize::new(528.0, 32.0)),
            );
            button_row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            button_row.setSpacing(8.0);
            button_row.addArrangedSubview(&make_button(mtm, "Login", ACTION_LOGIN, &self.target));
            button_row.addArrangedSubview(&make_button(mtm, "Logout", ACTION_LOGOUT, &self.target));
            button_row.addArrangedSubview(&make_button(mtm, "Sync now", ACTION_SYNC, &self.target));
            button_row.addArrangedSubview(&make_button(mtm, "Validate", ACTION_VALIDATE, &self.target));
            stack.addArrangedSubview(&button_row);

            let scroll = NSScrollView::initWithFrame(
                NSScrollView::alloc(mtm),
                NSRect::new(NSPoint::ZERO, NSSize::new(528.0, 280.0)),
            );
            scroll.setHasVerticalScroller(true);
            scroll.setBorderType(objc2_app_kit::NSBorderType::BezelBorder);

            let text_view = NSTextView::initWithFrame(
                NSTextView::alloc(mtm),
                NSRect::new(NSPoint::ZERO, NSSize::new(528.0, 280.0)),
            );
            text_view.setEditable(false);
            text_view.setRichText(false);
            scroll.setDocumentView(Some(&text_view));
            stack.addArrangedSubview(&scroll);
            *self.status_view.borrow_mut() = Some(text_view);

            let footer = NSStackView::initWithFrame(
                NSStackView::alloc(mtm),
                NSRect::new(NSPoint::ZERO, NSSize::new(528.0, 32.0)),
            );
            footer.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            footer.setSpacing(8.0);
            footer.addArrangedSubview(&make_button(
                mtm,
                "Open config folder",
                ACTION_OPEN_FOLDER,
                &self.target,
            ));
            footer.addArrangedSubview(&make_button(mtm, "Close", ACTION_CLOSE, &self.target));
            stack.addArrangedSubview(&footer);

            content_view.addSubview(&stack);

            window.center();
            *self.window.borrow_mut() = Some(window.clone());
            *self.target.ivars().window.lock().unwrap() = Some(window);
        }
    }
}

unsafe fn make_label(mtm: MainThreadMarker, text: &str) -> Retained<NSTextField> {
    let label: Retained<NSTextField> = msg_send_id![NSTextField::alloc(mtm), init];
    label.setStringValue(&NSString::from_str(text));
    label.setBezeled(false);
    label.setDrawsBackground(false);
    label.setEditable(false);
    label.setSelectable(true);
    label.setFrame(NSRect::new(NSPoint::ZERO, NSSize::new(528.0, 36.0)));
    label
}

unsafe fn make_button(
    mtm: MainThreadMarker,
    title: &str,
    tag: i64,
    target: &Retained<ActionTarget>,
) -> Retained<NSButton> {
    let button: Retained<NSButton> = msg_send_id![NSButton::alloc(mtm), init];
    button.setTitle(&NSString::from_str(title));
    button.setBezelStyle(objc2_app_kit::NSBezelStyle::Rounded);
    button.setTag(tag);
    button.setTarget(Some(ProtocolObject::from_ref(target.as_ref()).into_ref()));
    button.setAction(Some(sel!(handleAction:)));
    button.setFrame(NSRect::new(NSPoint::ZERO, NSSize::new(110.0, 28.0)));
    button
}

pub fn open_path(path: &Path) {
    unsafe {
        let url_str = NSString::from_str(&path.display().to_string());
        if let Some(url) = NSURL::fileURLWithPath(&url_str) {
            let workspace = NSWorkspace::sharedWorkspace();
            workspace.openURL(&url);
        }
    }
}
