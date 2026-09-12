use crate::state::DisplayState;
use komorebi_client::SocketMessage;
use komorebi_client::SubscribeOptions;
use komorebi_client::UnixListener;
use std::io;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::Instant;

const RETRY_INTERVAL: Duration = Duration::from_secs(1);
const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const SOCKET_TIMEOUT: Duration = Duration::from_secs(1);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

pub struct Worker {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn start(publish: impl Fn(DisplayState) + Send + 'static) -> io::Result<Self> {
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| io::Error::other("No local application data directory"))?
            .join("komorebi");
        std::fs::create_dir_all(&data_dir)?;
        let name = format!("komorebi-tray-{}", std::process::id());
        let path = data_dir.join(&name);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let thread = thread::Builder::new()
            .name("komorebi-tray-subscription".to_owned())
            .spawn(move || {
                let subscription = Subscription {
                    name,
                    path,
                    listener: None,
                };
                run(subscription, &worker_stop, publish);
            })?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

struct Subscription {
    name: String,
    path: PathBuf,
    listener: Option<UnixListener>,
}

impl Subscription {
    fn register(&mut self) -> io::Result<()> {
        let options = SubscribeOptions {
            filter_state_changes: true,
        };
        if self.listener.is_some() && self.path.exists() {
            komorebi_client::send_message(&SocketMessage::AddSubscriberSocketWithOptions(
                self.name.clone(),
                options,
            ))
        } else {
            self.listener = None;
            let listener = komorebi_client::subscribe_with_options(&self.name, options)?;
            listener.set_nonblocking(true)?;
            self.listener = Some(listener);
            Ok(())
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let _ = komorebi_client::send_message(&SocketMessage::RemoveSubscriberSocket(
            self.name.clone(),
        ));
        self.listener = None;
        let _ = std::fs::remove_file(&self.path);
    }
}

fn run(mut subscription: Subscription, stop: &AtomicBool, publish: impl Fn(DisplayState)) {
    let mut next_registration = Instant::now();
    let mut snapshot_deadline = None;
    let mut current = DisplayState::disconnected();
    let mut update = |display: DisplayState| {
        if current != display {
            current = display.clone();
            publish(display);
        }
    };

    while !stop.load(Ordering::Relaxed) {
        // Bound each batch so a busy daemon cannot starve shutdown or the watchdog.
        for _ in 0..16 {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let Some(listener) = &subscription.listener else {
                break;
            };
            match listener.accept() {
                Ok((mut stream, _)) => {
                    // Winsock can inherit nonblocking mode from the listener.
                    let mut bytes = Vec::new();
                    let received = stream
                        .set_nonblocking(false)
                        .and_then(|()| stream.set_read_timeout(Some(SOCKET_TIMEOUT)))
                        .and_then(|()| stream.read_to_end(&mut bytes));
                    match received {
                        Ok(0) => {
                            update(DisplayState::disconnected());
                            next_registration = Instant::now();
                            snapshot_deadline = None;
                            break;
                        }
                        Ok(_) => {
                            if let Ok(display) = DisplayState::from_notification(&bytes) {
                                update(display);
                                snapshot_deadline = None;
                                next_registration = Instant::now() + REFRESH_INTERVAL;
                            }
                        }
                        Err(_) => {}
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    subscription.listener = None;
                    update(DisplayState::disconnected());
                    snapshot_deadline = None;
                    next_registration = Instant::now() + RETRY_INTERVAL;
                    break;
                }
            }
        }

        let now = Instant::now();
        if snapshot_deadline.is_some_and(|deadline| now >= deadline) {
            update(DisplayState::disconnected());
            subscription.listener = None;
            snapshot_deadline = None;
            next_registration = now + RETRY_INTERVAL;
        }
        if now >= next_registration {
            match subscription.register() {
                Ok(()) => {
                    // Registration always emits a complete state, even with state filtering.
                    snapshot_deadline = Some(Instant::now() + SOCKET_TIMEOUT);
                    next_registration = Instant::now() + REFRESH_INTERVAL;
                }
                Err(_) => {
                    update(DisplayState::disconnected());
                    subscription.listener = None;
                    snapshot_deadline = None;
                    next_registration = Instant::now() + RETRY_INTERVAL;
                }
            }
        }
        thread::park_timeout(POLL_INTERVAL);
    }
}
