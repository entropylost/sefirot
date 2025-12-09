use std::cell::RefCell;
use std::io::Write;
use std::ptr::read_unaligned;
use std::sync::{Arc, Mutex};

use keter::lang::types::vector::{Vector, VectorAlign};
use keter::prelude::*;

#[cfg(feature = "global-print")]
pub mod global;

pub struct PrintType {
    closure: Box<dyn Fn(&mut Printer) -> String>,
    size: u32,
}
impl PrintType {}

pub struct PrintBuffer {
    types: RefCell<Vec<PrintType>>,
    data: Buffer<u8>,
    head: Buffer<u64>,
    host_data: Arc<Mutex<Vec<u8>>>,
}
impl PrintBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            types: RefCell::new(Vec::new()),
            data: DEVICE.create_buffer::<u8>(capacity),
            head: DEVICE.create_buffer_from_slice::<u64>(&[0]),
            host_data: Arc::new(Mutex::new(vec![0; capacity])),
        }
    }
    pub fn add_type(&self, closure: impl Fn(&mut Printer) -> String + 'static) -> u32 {
        let type_id = self.types.borrow().len() as u32;
        let mut printer = Printer::SizeQuery(PrintSize { size: 0 });
        closure(&mut printer);
        let size = match printer {
            Printer::SizeQuery(size) => size.size,
            _ => panic!("Expected SizeQuery printer"),
        };
        self.types.borrow_mut().push(PrintType {
            closure: Box::new(closure),
            size,
        });
        type_id
    }
    #[tracked]
    pub fn write(&self, type_id: u32) {
        let types = self.types.borrow();
        let print_type = &types[type_id as usize];
        let head = self
            .head
            .atomic_ref(0)
            .fetch_add(print_type.size as u64 + 4);
        if head + print_type.size as u64 <= self.data.len_expr() {
            let mut writer = PrintWriter {
                data: self.data.view(..),
                head,
            };
            type_id.expr().push_bytes(&mut writer);
            let mut printer = Printer::Writer(writer);
            (print_type.closure)(&mut printer);
        }
    }
    pub fn print(&self, closure: impl Fn(&mut Printer) -> String + 'static) {
        let type_id = self.add_type(closure);
        self.write(type_id);
    }
    pub fn flush(&self) {
        self.data.copy_to(&mut self.host_data.lock().unwrap());
        let mut size = 0;
        self.head.copy_to(std::slice::from_mut(&mut size));
        let mut printer = Printer::Reader(PrintReader {
            data: self.host_data.clone(),
            head: 0,
        });
        while printer.as_reader().head < size {
            let type_id = printer.as_reader().pop_type();
            let output = (self.types.borrow()[type_id as usize].closure)(&mut printer);
            print!("{output}");
        }
        std::io::stdout().flush().unwrap();
        self.head.copy_from(&[0]);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PrintSize {
    size: u32,
}
impl PrintSize {
    #[tracked]
    pub fn add(&mut self, size: u32) {
        self.size += size;
    }
}

pub struct PrintWriter {
    data: BufferView<u8>,
    head: Expr<u64>,
}
impl PrintWriter {
    #[tracked]
    fn write_byte(&mut self, byte: Expr<u8>) {
        let head = self.head;
        self.data.write(head, byte);
        self.head = head + 1;
    }
}

#[derive(Debug, Clone)]
pub struct PrintReader {
    data: Arc<Mutex<Vec<u8>>>,
    head: u64,
}
impl PrintReader {
    fn read<T>(&mut self, size: usize, f: impl FnOnce(&[u8]) -> T) -> T {
        let output = f(&self.data.lock().unwrap()[self.head as usize..(self.head as usize + size)]);
        self.head += size as u64;
        output
    }
    fn pop_type(&mut self) -> u32 {
        self.read(4, |data| {
            let data: [u8; 4] = data.try_into().unwrap();
            u32::from_ne_bytes(data)
        })
    }
}

pub enum Printer {
    SizeQuery(PrintSize),
    Writer(PrintWriter),
    Reader(PrintReader),
}

impl Printer {
    fn as_reader(&mut self) -> &mut PrintReader {
        match self {
            Self::Reader(reader) => reader,
            _ => panic!("Printer is not a Reader"),
        }
    }
    pub fn load<T>(&mut self, value: Expr<T>) -> T
    where
        T: Default + Value,
        Expr<T>: PushBytes,
    {
        match self {
            Self::SizeQuery(size) => {
                size.add(std::mem::size_of::<T>() as u32);
                T::default()
            }
            Self::Writer(writer) => {
                value.push_bytes(writer);
                T::default()
            }
            Self::Reader(reader) => reader.read(std::mem::size_of::<T>(), |data| unsafe {
                read_unaligned(data.as_ptr() as *const T)
            }),
        }
    }
}

pub trait PushBytes {
    fn push_bytes(self, writer: &mut PrintWriter);
}
impl PushBytes for Expr<u8> {
    fn push_bytes(self, writer: &mut PrintWriter) {
        writer.write_byte(self);
    }
}
macro_rules! gen_push_bytes {
    ($t:ty: $n:literal $(, $($tt:tt)*)?) => {
        impl PushBytes for Expr<$t> {
            fn push_bytes(self, writer: &mut PrintWriter) {
                let bytes: Expr<[u8; $n]> = self.bitcast();
                for i in 0..$n {
                    let byte: Expr<u8> = bytes.read(i as u32);
                    writer.write_byte(byte);
                }
                // bytes.push_bytes(writer);
            }
        }
        $(gen_push_bytes!($($tt)*);)?
    };
}
gen_push_bytes!(
    u16: 2,
    u32: 4,
    u64: 8,
    i8: 1,
    i16: 2,
    i32: 4,
    i64: 8,
    f16: 2,
    f32: 4,
    f64: 8,
    bool: 1
);

// TODO: Weird bug happens if we try to implement it directly.
/*
error[E0275]: overflow evaluating the requirement `Expr<keter::lang::types::vector::Vector<_, _>>: PushBytes`
   --> yesod/examples/print.rs:12:25
    |
12  |                 printer.load(index),
    |                         ^^^^
    |
    = help: consider increasing the recursion limit by adding a `#![recursion_limit = "256"]` attribute to your crate (`print`)
    = note: required for `Expr<Vector<Vector<_, _>, _>>` to implement `PushBytes`
    = note: 126 redundant requirements hidden
    = note: required for `Expr<Vector<Vector<Vector<Vector<..., _>, _>, _>, _>>` to implement `PushBytes`
note: required by a bound in `yesod::printer::Printer::<'a>::load`
   --> /home/keter/Documents/sefirot/yesod/src/printer.rs:141:18
    |
138 |     pub fn load<T>(&mut self, value: Expr<T>) -> T
    |            ---- required by a bound in this associated function
...
141 |         Expr<T>: PushBytes,
    |                  ^^^^^^^^^ required by this bound in `Printer::<'a>::load`
    = note: the full name for the type has been written to '/home/keter/Documents/sefirot/target/debug/examples/print-2a58c877743e6fbd.long-type-1279998347952659411.txt'
    = note: consider using `--verbose` to print the full type name to the console
*/
macro_rules! gen_vector_push_bytes {
    ($t:ty $(, $($tt:tt)*)?) => {
        impl<const N: usize> PushBytes for Expr<Vector<$t, N>>
        where
            Expr<$t>: PushBytes,
            Expr<[ $t; N ]>: From<Expr<Vector<$t, N>>>,
            $t: VectorAlign<N>,
        {
            fn push_bytes(self, writer: &mut PrintWriter) {
                let v: Expr<[ $t; N ]> = self.into();
                for i in 0..N {
                    let element: Expr<$t> = v.read(i as u32);
                    element.push_bytes(writer);
                }
            }
        }
        $(gen_vector_push_bytes!($($tt)*);)?
    }
}
gen_vector_push_bytes!(u8, u16, u32, u64, i8, i16, i32, i64, f16, f32, f64, bool);

/*
impl<T: Value, const N: usize> PushBytes for Expr<[T; N]>
where
    Expr<T>: PushBytes,
{
    fn push_bytes(self, writer: &mut PrintWriter) {
        for i in 0..N {
            let element: Expr<T> = self.read(i as u32);
            element.push_bytes(writer);
        }
    }
}
impl<const N: usize, T: VectorAlign<N>> PushBytes for Expr<Vector<T, N>>
where
    Expr<T>: PushBytes,
    Expr<[T; N]>: From<Expr<Vector<T, N>>>,
{
    fn push_bytes(self, writer: &mut PrintWriter) {
        let v: Expr<[T; N]> = self.into();
        v.push_bytes(writer);
    }
}
 */
