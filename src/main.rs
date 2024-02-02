use std::{
    sync::{Arc, RwLock},
    thread,
};

use backend::ViewerBackend;
use slint::PlatformError;

mod backend;

#[derive(Debug)]
enum AppError {
    GUIError(PlatformError),
    BackendError(backend::ViewerBackendError),
}

slint::slint! {
    export component Gauge inherits Image {
        in property <int> value;

        Image {
            source: @image-url("needle.png");
            rotation-angle: ((value*1deg) / 4096deg) * 260deg;
            height: 200px;
            width: 200px;
        }
        Image {
            source: @image-url("gauge.png");
            height: 200px;
            width: 200px;
        }
    }
    export component App {

        in property <int> a0;
        in property <int> a1;
        in property <int> a2;
        in property <int> a3;

        callback click_reconnect();

        GridLayout {
            spacing: 25px;
            Row {
                Gauge {
                    value: a0;
                }
                Gauge {
                    value: a1;
                }
                Gauge {
                    value: a2;
                }
                Gauge {
                    value: a3;
                }
            }
            Row {
                Text {
                    text: round(a0 / 4096 * 3333) + " mV";
                    font-size: 25px;
                    color: blue;
                    horizontal-alignment: center;
                    width: 200px;
                }

                Text {
                    text: round(a1 / 4096 * 3333) + " mV";
                    font-size: 25px;
                    color: blue;
                    horizontal-alignment: center;
                    width: 200px;
                }

                Text {
                    text: round(a2 / 4096 * 3333) + " mV";
                    font-size: 25px;
                    color: blue;
                    horizontal-alignment: center;
                    width: 200px;
                }

                Text {
                    text: round(a3 / 4096 * 3333) + " mV";
                    font-size: 25px;
                    color: blue;
                    horizontal-alignment: center;
                    width: 200px;
                }
            }
        }
    }
}

fn main() -> Result<(), AppError> {
    simple_logger::SimpleLogger::new().env().init().unwrap();

    let backend = Arc::new(RwLock::new(
        ViewerBackend::connect().map_err(|e| AppError::BackendError(e))?,
    ));

    backend
        .write()
        .map(|mut be| be.connect_socket().map_err(|e| AppError::BackendError(e)))
        .unwrap()
        .unwrap();

    // handle updates offthread
    let be_clone = backend.clone();
    thread::spawn(move || {
        let backend = be_clone;
        log::info!("backend thread started");
        loop {
            match backend.write().map(|mut wl| match wl.poll() {
                Ok(_) => {} // TODO: figure out if we're wasting cycles by not reading polled val here
                Err(e) => {
                    log::error!("error polling backend: {:?}", e);
                }
            }) {
                Ok(_) => {}
                Err(e) => {
                    log::error!("error locking backend: {:?}", e);
                }
            }
        }
    });

    let app = App::new().map_err(|e| AppError::GUIError(e))?;

    let weak_app = app.as_weak();
    thread::spawn(move || {
        let app = weak_app;

        loop {
            // thread::sleep(std::time::Duration::from_millis(1));

            let (a0, a1, a2, a3) = match backend.read().map(|be| {
                be.read().map(|vals| {
                    // TODO: Why read here instead of poll?
                    (
                        vals.a0 as i32,
                        vals.a1 as i32,
                        vals.a2 as i32,
                        vals.a3 as i32,
                    )
                })
            }) {
                Ok(v) => match v {
                    Ok(v) => v,
                    Err(e) => {
                        log::error!("error reading backend: {:?}", e);
                        continue;
                    }
                },
                Err(e) => {
                    log::error!("error locking backend: {:?}", e);
                    continue;
                }
            };

            match app.upgrade_in_event_loop(move |handle| {
                handle.set_a0(a0);
                handle.set_a1(a1);
                handle.set_a2(a2);
                handle.set_a3(a3);
            }) {
                Ok(_) => {}
                Err(e) => {
                    log::error!("error updating frontend: {:?}", e);
                }
            }
        }
    });

    Ok(app.run().map_err(|e| AppError::GUIError(e))?)
}
