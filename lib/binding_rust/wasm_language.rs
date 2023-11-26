use crate::{ffi, Language, LanguageError, Parser};
use std::{
    error,
    ffi::CString,
    fmt,
    mem::{self, MaybeUninit},
    os::raw::c_char,
};
pub use wasmtime;

#[cfg(feature = "wasm")]
pub fn test() {
    // Force Cargo to include wasmtime-c-api as a dependency of this crate,
    // even though it is only used by the C code.
    wasmtime_c_api::wasm_engine_new();
}

#[repr(C)]
#[derive(Clone)]
#[allow(non_camel_case_types)]
pub struct wasm_engine_t {
    pub(crate) engine: wasmtime::Engine,
}

pub struct WasmStore(*mut ffi::TSWasmStore);

#[derive(Debug, PartialEq, Eq)]
pub struct WasmError {
    pub kind: WasmErrorKind,
    pub message: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum WasmErrorKind {
    Parse,
    Compile,
    Instantiate,
    Other,
}

impl WasmStore {
    pub fn new(engine: wasmtime::Engine) -> Self {
        let engine = Box::new(wasm_engine_t { engine });
        WasmStore(unsafe {
            ffi::ts_wasm_store_new(Box::leak(engine) as *mut wasm_engine_t as *mut _)
        })
    }

    pub fn load_language(&mut self, name: &str, bytes: &[u8]) -> Result<Language, WasmError> {
        let name = CString::new(name).unwrap();
        unsafe {
            let mut error = MaybeUninit::<ffi::TSWasmError>::uninit();
            let mut message = MaybeUninit::<*mut c_char>::uninit();
            let language = ffi::ts_wasm_store_load_language(
                self.0,
                name.as_ptr(),
                bytes.as_ptr() as *const c_char,
                bytes.len() as u32,
                error.as_mut_ptr(),
                message.as_mut_ptr(),
            );

            if language.is_null() {
                let error = error.assume_init();
                let message = message.assume_init();
                let message = CString::from_raw(message);
                Err(WasmError {
                    kind: match error {
                        ffi::TSWasmErrorParse => WasmErrorKind::Parse,
                        ffi::TSWasmErrorCompile => WasmErrorKind::Compile,
                        ffi::TSWasmErrorInstantiate => WasmErrorKind::Instantiate,
                        _ => WasmErrorKind::Other,
                    },
                    message: message.into_string().unwrap(),
                })
            } else {
                Ok(Language(language))
            }
        }
    }
}

impl Language {
    pub fn is_wasm(&self) -> bool {
        unsafe { ffi::ts_language_is_wasm(self.0) }
    }
}

impl Parser {
    pub fn set_wasm_store(&mut self, store: WasmStore) -> Result<(), LanguageError> {
        unsafe { ffi::ts_parser_set_wasm_store(self.0.as_ptr(), store.0) };
        mem::forget(store);
        Ok(())
    }

    pub fn take_wasm_store(&mut self) -> Option<WasmStore> {
        let ptr = unsafe { ffi::ts_parser_take_wasm_store(self.0.as_ptr()) };
        if ptr.is_null() {
            None
        } else {
            Some(WasmStore(ptr))
        }
    }
}

impl Drop for WasmStore {
    fn drop(&mut self) {
        unsafe { ffi::ts_wasm_store_delete(self.0) };
    }
}

impl fmt::Display for WasmError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let kind = match self.kind {
            WasmErrorKind::Parse => "Failed to parse wasm",
            WasmErrorKind::Compile => "Failed to compile wasm",
            WasmErrorKind::Instantiate => "Failed to instantiate wasm module",
            WasmErrorKind::Other => "Unknown error",
        };
        write!(f, "{kind} {}", self.message)
    }
}

impl error::Error for WasmError {}
