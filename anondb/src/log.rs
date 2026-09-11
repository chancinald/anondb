/// Standardize and identify library logs.
///
/// This exposes a macro api identical to the public api of `log`.
/// Use log::info!(), log::error!(), etc as normal
#[cfg(test)]
pub(crate) mod test {
    use std::sync::LazyLock;
    pub static SETUP: LazyLock<()> = LazyLock::new(|| {
        env_logger::builder()
            .filter_level(log_ext::LevelFilter::Info)
            .init();
    });
}

// pub(crate) static LISTENER_MAYBE: LazyLock<RwLock<Option<flume::Sender<String>>>> =
//     LazyLock::new(|| RwLock::new(None));
//
// pub fn listen_output() -> flume::Receiver<String> {
//     let (sndr, rcvr) = flume::unbounded();
//     *LISTENER_MAYBE.write().unwrap() = Some(sndr);
//     rcvr
// }

pub(crate) const PREFIX: &'static str = "";

// pub(crate) fn send_log(err: &str) {
//     if let Some(sndr) = LISTENER_MAYBE.read().unwrap().clone() {
//         if let Err(_) = sndr.send(err.to_string()) {
//             *LISTENER_MAYBE.write().unwrap() = None;
//         }
//     }
// }

macro_rules! warn_ {
    ($($input:expr),*) => {
        #[cfg(test)]
        std::sync::LazyLock::<()>::force(&log::test:::SETUP);
        let msg = format!($($input,)*);
        // log::send_log(&msg);
        // always print, even if no logging backend is connected
        if log_ext::log_enabled!(log_ext::Level::Info) {
            log_ext::warn!("{}{msg}", crate::log::PREFIX);
        } else {
            println!("WARNING: {}{msg}", crate::log::PREFIX);
        }

    };
}

macro_rules! error {
    ($($input:expr),*) => {
        #[cfg(test)]
        std::sync::LazyLock::<()>::force(&log::test::SETUP);
        let msg = format!($($input,)*);
        // log::send_log(&msg);
        // always print, even if no logging backend is connected
        if log_ext::log_enabled!(log_ext::Level::Info) {
            log_ext::error!("{}{msg}", crate::log::PREFIX);
        } else {
            println!("ERROR: {}{msg}", crate::log::PREFIX);
        }
    };
}

macro_rules! info {
    ($($input:expr),*) => {
        #[cfg(test)]
        std::sync::LazyLock::<()>::force(&log::test::SETUP);
        let msg = format!($($input,)*);
        // log::send_log(&msg);
        // always print, even if no logging backend is connected
        if log_ext::log_enabled!(log_ext::Level::Info) {
            log_ext::info!("{}{msg}", crate::log::PREFIX);
        } else {
            println!("INFO: {}{msg}", crate::log::PREFIX);
        }
    };
}

macro_rules! debug {
    ($($input:expr),*) => {
        #[cfg(test)]
        std::sync::LazyLock::<()>::force(&log::test::SETUP);
        let msg = format!($($input,)*);
        log::send_log(&msg);
        log_ext::debug!("{}{msg}", crate::log::PREFIX);
    };
}

pub(crate) use debug;
pub(crate) use error;
pub(crate) use info;
pub(crate) use warn_ as warn;
