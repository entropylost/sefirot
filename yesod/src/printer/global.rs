use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};

use fragile::Fragile;

use super::*;

static CAPACITY: AtomicUsize = AtomicUsize::new(1024 * 1024);
static GLOBAL_PRINT_BUFFER: LazyLock<Fragile<PrintBuffer>> =
    LazyLock::new(|| Fragile::new(PrintBuffer::new(CAPACITY.load(Ordering::SeqCst))));

static CURRENT_PRINTER: Mutex<Option<Fragile<Printer>>> = Mutex::new(None);

pub fn set_capacity(capacity: usize) {
    CAPACITY.store(capacity, Ordering::SeqCst);
    GLOBAL_PRINT_BUFFER.get();
}

pub trait PrintExt {
    type Output;
    fn host(self) -> Self::Output;
}
impl<T> PrintExt for Expr<T>
where
    T: Default + Value,
    Expr<T>: PushBytes,
{
    type Output = T;
    fn host(self) -> T {
        let mut guard = CURRENT_PRINTER.lock().unwrap();
        let printer = guard
            .as_mut()
            .expect("This can only be called inside a print closure.");
        printer.get_mut().load(self)
    }
}

pub fn device_print(closure: impl Fn() -> String + 'static) {
    GLOBAL_PRINT_BUFFER.get().print(move |printer| {
        let mut output = String::new();
        take_mut::take(printer, |printer| {
            let mut guard = CURRENT_PRINTER.lock().unwrap();
            *guard = Some(Fragile::new(printer));
            drop(guard);
            output = closure();
            CURRENT_PRINTER.lock().unwrap().take().unwrap().into_inner()
        });
        output
    });
}
pub fn device_println(closure: impl Fn() -> String + 'static) {
    device_print(move || format!("{}\n", closure()));
}

#[macro_export]
macro_rules! device_print {
    ($($arg:tt)*) => {
        ::yesod::printer::global::device_print(move || format!($($arg)*))
    };
}
#[macro_export]
macro_rules! device_println {
    ($($arg:tt)*) => {
        ::yesod::printer::global::device_println(move || format!($($arg)*))
    };
}

pub fn flush_printer() {
    GLOBAL_PRINT_BUFFER.get().flush();
}
