//! Accessibility metadata for GPUI 0.2.2, which has no platform tree adapter.
//! Wrappers preserve layout/input and publish the painted, clipped controls.
use gpui::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) enum Request {
    Press,
    SetValue(f64),
    SetText(String),
    Increment,
    Decrement,
}
pub(crate) type Callback = Rc<dyn Fn(Request)>;

#[derive(Clone)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) struct Node {
    pub label: String,
    pub role: &'static str,
    pub value: Option<f64>,
    pub text: Option<String>,
    pub range: Option<(f64, f64)>,
    pub enabled: bool,
    pub callback: Option<Callback>,
    pub bounds: Bounds<Pixels>,
}
impl Node {
    pub fn button(label: impl Into<String>, enabled: bool, callback: Callback) -> Self {
        Self {
            label: label.into(),
            role: "AXButton",
            value: None,
            text: None,
            range: None,
            enabled,
            callback: Some(callback),
            bounds: Bounds::default(),
        }
    }
    pub fn slider(
        label: impl Into<String>,
        value: f64,
        range: (f64, f64),
        callback: Callback,
    ) -> Self {
        Self {
            role: "AXSlider",
            value: Some(value),
            range: Some(range),
            ..Self::button(label, true, callback)
        }
    }
    pub fn text(label: impl Into<String>, value: String, callback: Callback) -> Self {
        Self {
            role: "AXTextField",
            text: Some(value),
            ..Self::button(label, true, callback)
        }
    }
    pub fn group(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            role: "AXGroup",
            value: None,
            text: None,
            range: None,
            enabled: true,
            callback: None,
            bounds: Bounds::default(),
        }
    }
    #[cfg(any(target_os = "macos", test))]
    pub fn invoke(&self, request: Request) -> bool {
        if !self.enabled {
            return false;
        }
        if matches!(
            request,
            Request::Increment | Request::Decrement | Request::SetValue(_)
        ) && self.role != "AXSlider"
        {
            return false;
        }
        if matches!(request, Request::SetText(_)) && self.role != "AXTextField" {
            return false;
        }
        if matches!(request, Request::Press) && self.role != "AXButton" {
            return false;
        }
        if let Request::SetValue(value) = request
            && (!value.is_finite()
                || !self
                    .range
                    .is_some_and(|(min, max)| (min..=max).contains(&value)))
        {
            return false;
        }
        if let Some(callback) = &self.callback {
            callback(request);
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct Tree(Rc<TreeState>);
#[derive(Default)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct TreeState {
    nodes: RefCell<Vec<Node>>,
    native: bool,
    view: Cell<usize>,
}
impl Drop for TreeState {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        native::clear(self.view.get());
    }
}
impl Tree {
    pub fn new(native: bool) -> Self {
        Self(Rc::new(TreeState {
            native,
            nodes: RefCell::default(),
            view: Cell::default(),
        }))
    }
    pub fn element(&self, child: impl IntoElement, node: Node) -> Accessible {
        Accessible {
            child: child.into_any_element(),
            tree: self.clone(),
            node: Some(node),
        }
    }
    pub fn root(&self, child: impl IntoElement) -> Accessible {
        Accessible {
            child: child.into_any_element(),
            tree: self.clone(),
            node: None,
        }
    }
    #[cfg(test)]
    pub fn nodes(&self) -> Vec<Node> {
        self.0.nodes.borrow().clone()
    }
}
pub(crate) struct Accessible {
    child: AnyElement,
    tree: Tree,
    node: Option<Node>,
}
impl IntoElement for Accessible {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for Accessible {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.node.is_none() {
            self.tree.0.nodes.borrow_mut().clear();
        }
        self.child.prepaint(window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(mut node) = self.node.clone() {
            node.bounds = bounds.intersect(&window.content_mask().bounds);
            if node.bounds.size.width > px(0.) && node.bounds.size.height > px(0.) {
                self.tree.0.nodes.borrow_mut().push(node);
            }
        }
        self.child.paint(window, cx);
        if self.node.is_none() {
            let tree = self.tree.clone();
            // Deferred color popups paint after their parent; publish after all painting.
            window.defer(cx, move |window, _| {
                #[cfg(target_os = "macos")]
                if tree.0.native {
                    tree.0
                        .view
                        .set(native::publish(&tree.0.nodes.borrow(), window));
                }
                #[cfg(not(target_os = "macos"))]
                let _ = (tree, window);
            });
        }
    }
}

#[cfg(target_os = "macos")]
mod native {
    #![allow(unexpected_cfgs)]
    use super::*;
    use cocoa::{
        base::{NO, YES, id, nil},
        foundation::{NSPoint, NSRect, NSSize, NSString},
    };
    use objc::{
        class,
        declare::ClassDecl,
        msg_send,
        runtime::{Class, Object, Sel},
        sel, sel_impl,
    };
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::{collections::HashMap, sync::OnceLock};

    // AppKit invokes accessibility methods on the main thread. Released/stale
    // nodes have no callback, even when an assistive client retains the object.
    thread_local! {
        static CALLBACKS: RefCell<HashMap<u64, Node>> = RefCell::new(HashMap::new());
        static WINDOWS: RefCell<HashMap<usize, Vec<(u64, usize)>>> = RefCell::new(HashMap::new());
    }
    static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    fn invoke(object: &Object, request: Request) -> bool {
        let token = unsafe { *object.get_ivar::<u64>("glanceToken") };
        // Release the registry borrow before calling application code.
        let node = CALLBACKS.with(|nodes| nodes.borrow().get(&token).cloned());
        node.is_some_and(|node| node.invoke(request))
    }
    extern "C" fn press(object: &Object, _: Sel) -> bool {
        invoke(object, Request::Press)
    }
    extern "C" fn increment(object: &Object, _: Sel) -> bool {
        invoke(object, Request::Increment)
    }
    extern "C" fn decrement(object: &Object, _: Sel) -> bool {
        invoke(object, Request::Decrement)
    }
    extern "C" fn set_value(object: &Object, _: Sel, value: id) {
        if value == nil {
            return;
        }
        let token = unsafe { *object.get_ivar::<u64>("glanceToken") };
        let text = CALLBACKS.with(|nodes| {
            nodes
                .borrow()
                .get(&token)
                .is_some_and(|n| n.role == "AXTextField")
        });
        unsafe {
            if text {
                let supported: bool = msg_send![value, respondsToSelector: sel!(UTF8String)];
                if supported {
                    let bytes: *const std::ffi::c_char = msg_send![value, UTF8String];
                    if !bytes.is_null() {
                        invoke(
                            object,
                            Request::SetText(
                                std::ffi::CStr::from_ptr(bytes)
                                    .to_string_lossy()
                                    .into_owned(),
                            ),
                        );
                    }
                }
            } else {
                let numeric: bool = msg_send![value, respondsToSelector: sel!(doubleValue)];
                if numeric {
                    invoke(object, Request::SetValue(msg_send![value, doubleValue]));
                }
            }
        }
    }
    fn element_class(role: &str) -> &'static Class {
        static BUTTON: OnceLock<&'static Class> = OnceLock::new();
        static SLIDER: OnceLock<&'static Class> = OnceLock::new();
        static TEXT: OnceLock<&'static Class> = OnceLock::new();
        static GROUP: OnceLock<&'static Class> = OnceLock::new();
        let (cell, name) = match role {
            "AXButton" => (&BUTTON, "GlanceAccessibleButton"),
            "AXSlider" => (&SLIDER, "GlanceAccessibleSlider"),
            "AXTextField" => (&TEXT, "GlanceAccessibleText"),
            _ => (&GROUP, "GlanceAccessibleGroup"),
        };
        cell.get_or_init(|| {
            let mut class = ClassDecl::new(name, class!(NSAccessibilityElement)).unwrap();
            class.add_ivar::<u64>("glanceToken");
            unsafe {
                if role == "AXButton" {
                    class.add_method(
                        sel!(accessibilityPerformPress),
                        press as extern "C" fn(&Object, Sel) -> bool,
                    );
                }
                if role == "AXSlider" {
                    class.add_method(
                        sel!(accessibilityPerformIncrement),
                        increment as extern "C" fn(&Object, Sel) -> bool,
                    );
                    class.add_method(
                        sel!(accessibilityPerformDecrement),
                        decrement as extern "C" fn(&Object, Sel) -> bool,
                    );
                }
                if matches!(role, "AXSlider" | "AXTextField") {
                    class.add_method(
                        sel!(setAccessibilityValue:),
                        set_value as extern "C" fn(&Object, Sel, id),
                    );
                }
            }
            class.register()
        })
    }
    pub fn clear(key: usize) {
        let old = WINDOWS.with(|windows| windows.borrow_mut().remove(&key).unwrap_or_default());
        CALLBACKS.with(|callbacks| {
            let mut callbacks = callbacks.borrow_mut();
            for (token, _) in old {
                callbacks.remove(&token);
            }
        });
    }
    pub fn publish(nodes: &[Node], window: &Window) -> usize {
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return 0;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return 0;
        };
        let view = handle.ns_view.as_ptr() as id;
        // SAFETY: GPUI owns this view; paint and deferred callbacks run on the
        // AppKit main thread. NSView retains the array and its elements.
        unsafe {
            let key = view as usize;
            let old = WINDOWS.with(|windows| windows.borrow_mut().remove(&key).unwrap_or_default());
            let reuse = old.len() == nodes.len()
                && CALLBACKS.with(|callbacks| {
                    let callbacks = callbacks.borrow();
                    old.iter().zip(nodes).all(|((token, _), node)| {
                        callbacks
                            .get(token)
                            .is_some_and(|old| old.label == node.label && old.role == node.role)
                    })
                });
            if !reuse {
                CALLBACKS.with(|callbacks| {
                    let mut callbacks = callbacks.borrow_mut();
                    for (token, _) in &old {
                        callbacks.remove(token);
                    }
                });
            }
            let array: id = msg_send![class!(NSMutableArray), array];
            let mut tokens = Vec::new();
            let frame: NSRect = msg_send![view, bounds];
            for (index, node) in nodes.iter().enumerate() {
                let (token, element) = if reuse {
                    (old[index].0, old[index].1 as id)
                } else {
                    let element: id = msg_send![element_class(node.role), new];
                    (
                        NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                        element,
                    )
                };
                (*element).set_ivar("glanceToken", token);
                let label = NSString::alloc(nil).init_str(&node.label);
                let role = NSString::alloc(nil).init_str(node.role);
                let _: () = msg_send![element, setAccessibilityLabel: label];
                let _: () = msg_send![element, setAccessibilityRole: role];
                let _: () = msg_send![label, release];
                let _: () = msg_send![role, release];
                let _: () = msg_send![element, setAccessibilityParent: view];
                let _: () = msg_send![element, setAccessibilityEnabled: if node.enabled { YES } else { NO }];
                let b = node.bounds;
                let rect = NSRect::new(
                    NSPoint::new(
                        f32::from(b.left()) as f64,
                        frame.size.height - f32::from(b.bottom()) as f64,
                    ),
                    NSSize::new(
                        f32::from(b.size.width) as f64,
                        f32::from(b.size.height) as f64,
                    ),
                );
                let _: () = msg_send![element, setAccessibilityFrameInParentSpace: rect];
                if let Some(value) = node.value {
                    let number: id = msg_send![class!(NSNumber), numberWithDouble: value];
                    // Call the superclass property setter, not our user action setter.
                    let _: () = msg_send![super(element, class!(NSAccessibilityElement)), setAccessibilityValue: number];
                }
                if let Some(text) = &node.text {
                    let text = NSString::alloc(nil).init_str(text);
                    let _: () = msg_send![super(element, class!(NSAccessibilityElement)), setAccessibilityValue: text];
                    let _: () = msg_send![text, release];
                }
                if let Some((min, max)) = node.range {
                    let min: id = msg_send![class!(NSNumber), numberWithDouble: min];
                    let max: id = msg_send![class!(NSNumber), numberWithDouble: max];
                    let _: () = msg_send![element, setAccessibilityMinValue: min];
                    let _: () = msg_send![element, setAccessibilityMaxValue: max];
                }
                CALLBACKS.with(|callbacks| callbacks.borrow_mut().insert(token, node.clone()));
                tokens.push((token, element as usize));
                let _: () = msg_send![array, addObject: element];
                if !reuse {
                    let _: () = msg_send![element, release];
                }
            }
            let _: () = msg_send![view, setAccessibilityChildren: array];
            WINDOWS.with(|windows| windows.borrow_mut().insert(key, tokens));
            key
        }
    }
}
