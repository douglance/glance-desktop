use super::{Editor, Message, actions::Action};
use crate::accessibility::{Callback, Node, Request};
use gpui::*;
use std::rc::Rc;

impl Editor {
    pub(super) fn accessible_action(&self, action: Action) -> Callback {
        let sender = self.sender.clone();
        let scope = self.tool_scope();
        Rc::new(move |request| {
            if matches!(request, Request::Press) {
                let _ = sender.try_send(Message::Accessibility(scope, action.clone()));
            }
        })
    }
    pub(super) fn accessible_button(
        &self,
        label: impl Into<String>,
        enabled: bool,
        action: Action,
        child: impl IntoElement,
    ) -> AnyElement {
        self.accessibility
            .element(
                child,
                Node::button(label, enabled, self.accessible_action(action)),
            )
            .into_any_element()
    }
    pub(super) fn accessible_color(
        &self,
        label: &str,
        color: [u8; 3],
        action: impl Fn([u8; 3]) -> Action + 'static,
        child: impl IntoElement,
    ) -> AnyElement {
        let sender = self.sender.clone();
        let scope = self.tool_scope();
        let callback = Rc::new(move |request| {
            if let Request::SetText(text) = request
                && let Some(rgb) = crate::color_picker::parse_hex(&text)
            {
                let _ = sender.try_send(Message::Accessibility(scope, action(rgb)));
            }
        });
        self.accessibility
            .element(
                child,
                Node::text(
                    format!("{label} hex"),
                    format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2]),
                    callback,
                ),
            )
            .into_any_element()
    }
    pub(super) fn accessible_slider(
        &self,
        label: impl Into<String>,
        value: f64,
        limits: (f64, f64, f64),
        action: impl Fn(f64) -> Action + 'static,
        child: impl IntoElement,
    ) -> AnyElement {
        let sender = self.sender.clone();
        let scope = self.tool_scope();
        let (min, max, step) = limits;
        let callback = Rc::new(move |request| {
            let next = match request {
                Request::SetValue(value) => value,
                Request::Increment => (value + step).min(max),
                Request::Decrement => (value - step).max(min),
                Request::Press | Request::SetText(_) => return,
            };
            let _ = sender.try_send(Message::Accessibility(scope, action(next)));
        });
        self.accessibility
            .element(child, Node::slider(label, value, (min, max), callback))
            .into_any_element()
    }
}
