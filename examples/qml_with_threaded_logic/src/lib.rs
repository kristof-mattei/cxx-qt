// SPDX-FileCopyrightText: 2021 Klarälvdalens Datakonsult AB, a KDAB Group company <info@kdab.com>
// SPDX-FileContributor: Andrew Hayzen <andrew.hayzen@kdab.com>
// SPDX-FileContributor: Gerhard de Clercq <gerhard.declercq@kdab.com>
//
// SPDX-License-Identifier: MIT OR Apache-2.0
use cxx_qt::make_qobject;

// TODO: update this example to also respond to changes in the url property
// once we have implemented the PropertyChangeHandler trait

// TODO: maybe we want to make it possible to define an enum inside the mod?
enum Event {
    TitleArrived(String),
}

#[make_qobject]
mod website {
    use super::Event;
    use cxx_qt_lib::let_qstring;
    use futures::{
        channel::mpsc::{UnboundedReceiver, UnboundedSender},
        executor::block_on,
        FutureExt, StreamExt,
    };
    use futures_timer::Delay;
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        thread,
        time::Duration,
    };

    pub struct Data {
        url: String,
        title: String,
    }

    impl Default for Data {
        fn default() -> Self {
            Self {
                url: "known".to_owned(),
                title: "Press refresh to get a title...".to_owned(),
            }
        }
    }

    struct RustObj {
        event_sender: UnboundedSender<Event>,
        event_queue: UnboundedReceiver<Event>,
        loading: AtomicBool,
    }

    impl Default for RustObj {
        fn default() -> Self {
            let (event_sender, event_queue) = futures::channel::mpsc::unbounded();

            Self {
                event_sender,
                event_queue,
                loading: AtomicBool::new(false),
            }
        }
    }

    impl RustObj {
        fn change_url(&self, cpp: Pin<&mut CppObj>) {
            let mut wrapper = CppObjWrapper::new(cpp);

            let url = wrapper.url().to_rust();
            let new_url = if url == "known" { "unknown" } else { "known" };

            let_qstring!(new_url = new_url);
            wrapper.set_url(&new_url);
        }

        fn refresh_title(&self, cpp: Pin<&mut CppObj>) {
            let mut wrapper = CppObjWrapper::new(cpp);

            // TODO: SeqCst is probably not the most efficient solution
            let new_load =
                self.loading
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst);
            if new_load.is_err() {
                println!("Skipped refresh_title request, because already in progress.");
                return;
            }

            let_qstring!(s = "Loading...");
            wrapper.set_title(&s);

            let url = wrapper.url().to_rust();
            let update_requester = wrapper.update_requester();
            let event_sender = self.event_sender.clone();

            let fetch_title = async move {
                // Simulate the delay of a network request with a simple timer
                Delay::new(Duration::from_secs(1)).await;

                let title = if url == "known" {
                    "Known website"
                } else {
                    "Unknown website"
                };

                event_sender
                    .unbounded_send(Event::TitleArrived(title.to_owned()))
                    .unwrap();
                update_requester.request_update();
            };
            thread::spawn(move || block_on(fetch_title));
        }
    }

    impl UpdateRequestHandler<CppObj> for RustObj {
        fn handle_update_request(&mut self, cpp: Pin<&mut CppObj>) {
            let mut wrapper = CppObjWrapper::new(cpp);

            while let Some(event) = self.event_queue.next().now_or_never() {
                if let Some(event) = event {
                    super::process_event(&event, &mut wrapper);

                    // TODO: this makes more sense inside process_event, but can only be moved there once
                    // we can implement process_event as a non-invokable member.
                    if matches!(event, Event::TitleArrived(_)) {
                        self.loading.store(false, Ordering::Relaxed);
                    }
                }
            }
        }
    }

    impl PropertyChangeHandler<CppObj, Property> for RustObj {
        fn handle_property_change(&mut self, cpp: Pin<&mut CppObj>, property: Property) {
            match property {
                Property::Url => self.refresh_title(cpp),
                Property::Title => println!("title changed"),
                _ => unreachable!(),
            }
        }
    }
}

// TODO: convert this to a member function we can have "private" RustObj methods
// TODO: maybe we want to make it possible to define a free function inside the mod?
fn process_event(event: &Event, cpp: &mut website::CppObjWrapper) {
    match event {
        Event::TitleArrived(title) => {
            use cxx_qt_lib::let_qstring;

            let_qstring!(s = title);
            cpp.set_title(&s);
        }
    }
}