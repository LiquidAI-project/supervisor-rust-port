use crate::lib::constants::GUI_ENDPOINT;
use crate::lib::{constants::INPUT, memory::Memory};
use crate::lib::stack::Stack;
use std::net::{Shutdown, TcpStream};
use std::{io::{Read, Write}, thread, time::Duration};
use std::sync::{Arc, Mutex};
use rand::Rng;
use crate::lib::wain_ast::ValType;

pub enum ImportInvalidError {
    NotFound,
    SignatureMismatch {
        expected_params: &'static [ValType],
        expected_ret: Option<ValType>,
    },
}

pub enum ImportInvokeError {
    Fatal { message: String },
}

pub trait Importer {
    const MODULE_NAME: &'static str = "env";
    fn validate(&self, name: &str, params: &[ValType], ret: Option<ValType>) -> Option<ImportInvalidError>;
    fn call(&mut self, name: &str, stack: &mut Stack, memory: &mut Memory) -> Result<(), ImportInvokeError>;
}

pub fn check_func_signature(
    actual_params: &[ValType],
    actual_ret: Option<ValType>,
    expected_params: &'static [ValType],
    expected_ret: Option<ValType>,
) -> Option<ImportInvalidError> {
    if actual_params.eq(expected_params) && actual_ret == expected_ret {
        return None;
    }
    Some(ImportInvalidError::SignatureMismatch {
        expected_params,
        expected_ret,
    })
}

pub struct DefaultImporter<R: Read, W: Write> {
    stdout: W,
    stdin: R,
    nb_reader: Option<termion::AsyncReader>,
    out_stream: Option<std::net::TcpStream>,
    pub output_buf: Arc<Mutex<Vec<u8>>>,
}

impl<R: Read, W: Write> Drop for DefaultImporter<R, W> {
    fn drop(&mut self) {
        let _ = self.stdout.flush();
        if let Some(stream) = &self.out_stream {
            let _ = stream.shutdown(Shutdown::Both);
        }
        //let _ = self.out_stream.as_ref().unwrap().shutdown(Shutdown::Both);
    }
}

impl<R: Read, W: Write> DefaultImporter<R, W> {
    pub fn with_stdio(stdin: R, stdout: W, output_buf: Arc<Mutex<Vec<u8>>>) -> Self {
        let guarded = GUI_ENDPOINT.lock().unwrap();
        let gui_url = &guarded.clone();
        if *gui_url == "".to_string() {
            return Self { stdout, stdin, nb_reader: None, out_stream: None, output_buf }
        }
        let stream = TcpStream::connect(gui_url);
        match stream {
            Ok(tcp_stream) => Self { stdout, stdin, nb_reader: None, out_stream: Some(tcp_stream), output_buf },
            Err(error) => {
                println!("{}", error);
                return Self { stdout, stdin, nb_reader: None, out_stream: None, output_buf }
            }
        }
    }

    fn usleep(&mut self, stack: &mut Stack) {
        let v: i32 = stack.pop();
        thread::sleep(Duration::from_micros(v as u64));
        stack.push(0);
    }

    fn rand(&mut self, stack: &mut Stack) {
        let mut rng = rand::thread_rng();
        let v: u32 = rng.gen_range(0..0xFFFFFFFF);
        stack.push(v as i32);
    }

    // (func (param i32) (result i32))
    fn putchar(&mut self, stack: &mut Stack) {
        //let v: i32 = stack.pop();
        //let b = v as u8;
        //let ret = match self.stdout.write(&[b]) {
        //    Ok(_) => b as i32,
        //    Err(_) => -1, // EOF
        //};
        //stack.push(ret);
        self.send_char(stack);
    }

    fn send_char(&mut self, stack: &mut Stack) {
        let v: i32 = stack.pop();
        let b = v as u8;
        self.output_buf.lock().unwrap().push(b);
        if let Some(stream) = &mut self.out_stream {
            let _ = stream.write_all(&[b]);
        }
        stack.push(v);
    }

    fn getchar_nonblocking(&mut self, stack: &mut Stack) {
        use termion::raw::IntoRawMode;
        let reader = self.nb_reader.get_or_insert_with(termion::async_stdin);
        let _raw = std::io::stdout().into_raw_mode();
        let mut buf = [0u8];
        let v = match reader.read(&mut buf) {
            Ok(n) if n > 0 => buf[0] as i32,
            _ => -1,
        };
        stack.push(v);
    }

    fn readkey(&mut self, stack: &mut Stack) {
        let mut guarded = INPUT.lock().unwrap();
        let key = guarded.clone().chars().nth(0).unwrap();
        *guarded = " ".to_string();
        let v = match key {
            'w' => 65,
            's' => 66,
            'd' => 67,
            'a' => 68,
            'q' => 113,
            'p' => 112,
            _ => -1
        };
        stack.push(v);
    }

    // (func () (result i32))
    fn getchar(&mut self, stack: &mut Stack) {
        let mut buf = [0u8];
        let v = match self.stdin.read_exact(&mut buf) {
            Ok(()) => buf[0] as i32,
            Err(_) => -1, // EOF
        };
        stack.push(v);
    }

    // (func (param i32 i32 i32) (result i32))
    fn memcpy(&mut self, stack: &mut Stack, memory: &mut Memory) -> Result<(), ImportInvokeError> {
        // memcpy(void *dest, void *src, size_t n)
        let size = stack.pop::<i32>() as u32 as usize;
        let src_start = stack.pop::<i32>() as u32 as usize;
        let dest_i32: i32 = stack.pop();
        let dest_start = dest_i32 as u32 as usize;
        let src_end = src_start + size;
        let dest_end = dest_start + size;

        let (dest, src) = if dest_end <= src_start {
            let (dest, src) = memory.data_mut().split_at_mut(src_start);
            (&mut dest[dest_start..dest_end], &mut src[..size])
        } else if src_end <= dest_start {
            let (src, dest) = memory.data_mut().split_at_mut(dest_start);
            (&mut dest[..size], &mut src[src_start..src_end])
        } else {
            return Err(ImportInvokeError::Fatal {
                message: format!(
                    "range overwrap on memcpy: src={}..{} and dest={}..{}",
                    src_start, src_end, dest_start, dest_end
                ),
            });
        };

        dest.copy_from_slice(src);
        stack.push(dest_i32);
        Ok(())
    }
}

impl<R: Read, W: Write> Importer for DefaultImporter<R, W> {
    fn validate(&self, name: &str, params: &[ValType], ret: Option<ValType>) -> Option<ImportInvalidError> {
        use ValType::*;
        match name {
            "putchar" => check_func_signature(params, ret, &[I32], Some(I32)),
            "getchar" => check_func_signature(params, ret, &[], Some(I32)),
            "memcpy" => check_func_signature(params, ret, &[I32, I32, I32], Some(I32)),
            "usleep" => check_func_signature(params, ret, &[I32], Some(I32)),
            "rand" => check_func_signature(params, ret, &[], Some(I32)),
            "getchar_nonblocking" => check_func_signature(params, ret, &[], Some(I32)),
            "readkey" => check_func_signature(params, ret, &[], Some(I32)),
            "send_char" => check_func_signature(params, ret, &[I32], Some(I32)),
            "abort" => check_func_signature(params, ret, &[], None),
            _ => Some(ImportInvalidError::NotFound),
        }
    }

    fn call(&mut self, name: &str, stack: &mut Stack, memory: &mut Memory) -> Result<(), ImportInvokeError> {
        match name {
            "putchar" => {
                self.putchar(stack);
                Ok(())
            }
            "getchar" => {
                self.getchar(stack);
                Ok(())
            }
            "abort" => Err(ImportInvokeError::Fatal {
                message: "aborted".to_string(),
            }),
            "memcpy" => self.memcpy(stack, memory),
            "usleep" => {
                self.usleep(stack);
                Ok(())
            }
            "rand" => {
                self.rand(stack);
                Ok(())
            }
            "getchar_nonblocking" => {
                self.getchar_nonblocking(stack);
                Ok(())
            }
            "readkey" => {
                self.readkey(stack);
                Ok(())
            }
            "send_char" => {
                self.send_char(stack);
                Ok(())
            }
            _ => unreachable!("fatal: invalid import function '{}'", name),
        }
    }
}
