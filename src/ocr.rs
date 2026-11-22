// Thin binding to the stable Tesseract C API. The engine and returned text
// are always released by Tesseract, the library that allocated them.
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::ptr::NonNull;
use image::DynamicImage;

#[link(name = "tesseract")]
extern "C" {
    fn TessBaseAPICreate() -> *mut c_void;
    fn TessBaseAPIDelete(engine: *mut c_void);
    fn TessBaseAPIInit3(engine: *mut c_void, path: *const c_char, language: *const c_char) -> c_int;
    fn TessBaseAPISetImage(engine: *mut c_void, image: *const u8, width: c_int, height: c_int,
        bytes_per_pixel: c_int, bytes_per_line: c_int);
    fn TessBaseAPIRecognize(engine: *mut c_void, monitor: *mut c_void) -> c_int;
    fn TessBaseAPIGetTsvText(engine: *mut c_void, page: c_int) -> *mut c_char;
    fn TessDeleteText(text: *mut c_char);
}

pub struct Engine(NonNull<c_void>);

impl Engine {
    pub fn new() -> Result<Self, String> {
        let path = std::env::var_os("SEPA_GIROCODE_TESSDATA")
            .map(|path| CString::new(path.as_encoded_bytes()))
            .transpose().map_err(|e| e.to_string())?;
        let path = path.as_ref().map_or(std::ptr::null(), |p| p.as_ptr());
        for language in [c"deu+eng", c"deu", c"eng"] {
            // The handle is exclusively owned by this Engine.
            let engine = Self(NonNull::new(unsafe { TessBaseAPICreate() })
                .ok_or("Tesseract konnte nicht gestartet werden.")?);
            if unsafe { TessBaseAPIInit3(engine.0.as_ptr(), path, language.as_ptr()) } == 0 {
                return Ok(engine);
            }
        }
        Err("Tesseract konnte die deutschen oder englischen Sprachdaten nicht laden.".into())
    }

    pub fn recognize(&mut self, image: &DynamicImage) -> Result<String, String> {
        let pixels = image.to_luma8();
        let (width, height) = (pixels.width() as c_int, pixels.height() as c_int);
        let engine = self.0.as_ptr();
        // Grayscale pixels use one byte each and remain valid throughout recognition.
        unsafe {
            TessBaseAPISetImage(engine, pixels.as_ptr(), width, height, 1, width);
            if TessBaseAPIRecognize(engine, std::ptr::null_mut()) != 0 {
                return Err("Texterkennung fehlgeschlagen.".into());
            }
            let text = NonNull::new(TessBaseAPIGetTsvText(engine, 0)).ok_or("Keine OCR-Ausgabe.")?;
            let tsv = format!("header\n{}", CStr::from_ptr(text.as_ptr()).to_string_lossy());
            TessDeleteText(text.as_ptr());
            Ok(tsv)
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        unsafe { TessBaseAPIDelete(self.0.as_ptr()) };
    }
}
