use gtk4::Button;
use gtk4::prelude::*;
use pulse::context::subscribe::{Facility, InterestMaskSet};
use pulse::context::{Context, FlagSet as ContextFlagSet};
use pulse::mainloop::standard::Mainloop;
use std::cell::RefCell;
use std::rc::Rc;

pub fn init(container: &gtk4::Box) {
    let btn = Button::builder().label("").build();
    btn.add_css_class("btn");
    btn.set_visible(false); // Hidden by default, does not occupy space when hidden
    container.append(&btn);

    btn.connect_clicked(|_| {
        let _ = std::process::Command::new("wpctl")
            .arg("set-mute")
            .arg("@DEFAULT_AUDIO_SOURCE@")
            .arg("toggle")
            .spawn();
    });

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<bool>();
    let b = btn.clone();
    gtk4::glib::MainContext::default().spawn_local(async move {
        while let Some(muted) = rx.recv().await {
            b.set_visible(muted);
        }
    });

    std::thread::spawn(move || {
        let mut mainloop = Mainloop::new().expect("Failed to create pulse mainloop");
        let mut proplist = pulse::proplist::Proplist::new().unwrap();
        proplist
            .set_str(
                pulse::proplist::properties::APPLICATION_NAME,
                "vibebar-p4-mic",
            )
            .unwrap();

        let context = Rc::new(RefCell::new(
            Context::new_with_proplist(&mainloop, "MicContext", &proplist)
                .expect("Failed to create pulse context"),
        ));

        {
            let mut ctx = context.borrow_mut();
            ctx.connect(None, ContextFlagSet::NOFLAGS, None)
                .expect("Failed to connect context");
        }

        // Wait for context to be ready
        loop {
            let _ = mainloop.iterate(false);
            let state = context.borrow().get_state();
            if state == pulse::context::State::Ready {
                break;
            }
            if !state.is_good() {
                return;
            }
        }

        let tx_cb = tx.clone();
        let context_cb = context.clone();

        let refresh_mic = move || {
            let tx_inner = tx_cb.clone();
            let context_inner = context_cb.clone();

            // Get introspector fresh from context borrow
            let introspect = context_inner.borrow().introspect();

            introspect.get_server_info(move |server_info| {
                if let Some(default_source_name) = &server_info.default_source_name {
                    let source_name: String = default_source_name.to_string();
                    let tx_innermost = tx_inner.clone();
                    let context_innermost = context_inner.clone();

                    // Get introspector again fresh for the nested callback
                    context_innermost
                        .borrow()
                        .introspect()
                        .get_source_info_by_name(&source_name, move |source_res| {
                            if let pulse::callbacks::ListResult::Item(source_info) = source_res {
                                let muted = source_info.mute;
                                let _ = tx_innermost.send(muted);
                            }
                        });
                }
            });
        };

        // Initial update
        refresh_mic();

        let refresh_mic_cb = Rc::new(refresh_mic);
        let refresh_mic_cb_inner = refresh_mic_cb.clone();

        context
            .borrow_mut()
            .set_subscribe_callback(Some(Box::new(move |fac, _op, _idx| {
                if fac == Some(Facility::Source) || fac == Some(Facility::Server) {
                    refresh_mic_cb_inner();
                }
            })));

        context
            .borrow_mut()
            .subscribe(InterestMaskSet::SOURCE | InterestMaskSet::SERVER, |_| {});

        let _ = mainloop.run();
    });
}
