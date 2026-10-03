//! AppKit magnify bridge for GPUI 0.2.2, which doesn't expose pinch events.
#![allow(unexpected_cfgs)]
use crate::Message;
use cocoa::{
    appkit::{NSEvent, NSView, NSWindow},
    base::id,
};
use gpui::{Bounds, Pixels, point, px};
use objc::{class, msg_send, sel, sel_impl};
use std::{cell::Cell, rc::Rc};

// Owns a retained monitor token, removed on the same UI thread on drop.
pub struct Monitor {
    token: id,
    _main_thread: std::marker::PhantomData<Rc<()>>,
}
impl Monitor {
    pub fn new(sender: async_channel::Sender<Message>, bounds: Rc<Cell<Bounds<Pixels>>>) -> Self {
        // SAFETY: Editor construction and drop happen on GPUI's AppKit main thread.
        // NSEvent copies the block, which owns the channel and canvas bounds.
        let token = unsafe {
            let handler = block::ConcreteBlock::new(move |event: id| -> id {
                let window: id = msg_send![event, window];
                if !window.is_null() {
                    let view = window.contentView();
                    let height = NSView::frame(view).size.height;
                    let position = event.locationInWindow();
                    let x = position.x as f32;
                    let y = (height - position.y) as f32;
                    if bounds.get().contains(&point(px(x), px(y))) {
                        let kind: usize = msg_send![event, type];
                        let smart = kind == 32; // NSEventTypeSmartMagnify
                        let delta = if smart {
                            0.
                        } else {
                            event.magnification() as f32
                        };
                        let _ = sender.try_send(Message::Magnify(delta, (x, y), smart));
                    }
                }
                event
            })
            .copy();
            let monitor: id = msg_send![class!(NSEvent), addLocalMonitorForEventsMatchingMask: ((1_u64<<30)|(1_u64<<32)) handler: &*handler];
            if !monitor.is_null() {
                let _: id = msg_send![monitor, retain];
            }
            monitor
        };
        Self {
            token,
            _main_thread: std::marker::PhantomData,
        }
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        // SAFETY: The token is retained above, remains valid, and isn't shared.
        unsafe {
            if !self.token.is_null() {
                let _: () = msg_send![class!(NSEvent), removeMonitor: self.token];
                let _: () = msg_send![self.token, release];
            }
        }
    }
}
