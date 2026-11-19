use std::{io::{self, Write}, sync::{Arc, Mutex}};

#[derive(Clone)]
struct Buffer(Arc<Mutex<Vec<u8>>>);
impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[derive(Debug)]
struct Error;
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("error\x1b[2J\x07")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&Cause)
    }
}
#[derive(Debug)]
struct Cause;
impl std::fmt::Display for Cause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("cause\u{009b}2J")
    }
}
impl std::error::Error for Cause {}

#[test]
fn terminal_controls_and_error_sources_are_escaped_in_all_formatters() {
    for mode in 0..3 {
        let buffer = Buffer(Arc::new(Mutex::new(Vec::new())));
        let writer = buffer.clone();
        let builder = tracing_subscriber::fmt().without_time().with_ansi(false).with_writer(move || writer.clone());
        let subscriber: Box<dyn tracing::Subscriber + Send + Sync> = match mode {
            0 => Box::new(builder.finish()),
            1 => Box::new(builder.compact().finish()),
            _ => Box::new(builder.pretty().finish()),
        };
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("peer input {}", "\x1b]0;attacker\x07\x08\x0c\x7f\u{009b}2J");
            tracing::error!(error = &Error as &dyn std::error::Error, "failure");
        });
        let text = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
        for control in ['\x1b', '\x07', '\x08', '\x0c', '\x7f', '\u{009b}'] {
            assert!(!text.contains(control), "raw control {:?}: {:?}", control, text);
        }
        for ordinary in ["attacker", "error", "cause", "\\x1b", "\\u{9b}"] {
            assert!(text.contains(ordinary), "missing text {:?}: {:?}", ordinary, text);
        }
    }
}
