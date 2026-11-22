mod document;
mod ocr;
mod payment;
mod selection;

use image::{DynamicImage, GrayImage, ImageFormat, Luma};
use serde_json::{json, Value};
use std::{io::Cursor, os::unix::ffi::OsStrExt, path::Path, slice};

pub struct Invoice {
    pages: Vec<document::Page>,
    details: Value,
}

#[repr(C)]
pub struct Bytes {
    data: *mut u8,
    len: usize,
}

impl From<Vec<u8>> for Bytes {
    fn from(data: Vec<u8>) -> Self {
        let data = data.into_boxed_slice();
        let len = data.len();
        Self { data: Box::into_raw(data).cast(), len }
    }
}

fn json_bytes(value: Value) -> Bytes {
    value.to_string().into_bytes().into()
}

#[no_mangle]
pub unsafe extern "C" fn giro_open(path: *const u8, len: usize) -> *mut Invoice {
    let path = std::ffi::OsStr::from_bytes(slice::from_raw_parts(path, len));
    let invoice = match document::load(Path::new(path)) {
        Ok((pages, text)) => {
            let (fields, accounts) = payment::extract(&text);
            Invoice {
                details: json!({
                    "pages": pages.len(), "fields": [fields.name, fields.iban, fields.bic, fields.amount, fields.purpose],
                    "accounts": accounts, "externalPayment": payment::external_payment_notice(&text),
                }),
                pages,
            }
        }
        Err(error) => Invoice { pages: Vec::new(), details: json!({ "error": error }) },
    };
    Box::into_raw(Box::new(invoice))
}

#[no_mangle]
pub unsafe extern "C" fn giro_close(invoice: *mut Invoice) {
    drop(Box::from_raw(invoice));
}

#[no_mangle]
pub unsafe extern "C" fn giro_details(invoice: &Invoice) -> Bytes {
    json_bytes(invoice.details.clone())
}

#[no_mangle]
pub unsafe extern "C" fn giro_page(invoice: &Invoice, index: usize) -> Bytes {
    let mut output = Cursor::new(Vec::new());
    if invoice.pages[index].image.write_to(&mut output, ImageFormat::Png).is_err() {
        return Vec::new().into();
    }
    output.into_inner().into()
}

#[no_mangle]
pub unsafe extern "C" fn giro_words(invoice: &Invoice, index: usize) -> Bytes {
    json_bytes(json!(invoice.pages[index].words.iter().map(|word| {
        let r = word.bounds;
        [r.min.x, r.min.y, r.max.x, r.max.y]
    }).collect::<Vec<_>>()))
}

#[no_mangle]
pub unsafe extern "C" fn giro_copy(invoice: &Invoice, index: usize, left: f32, top: f32, right: f32, bottom: f32) -> Bytes {
    let area = emath::Rect::from_two_pos(emath::pos2(left, top), emath::pos2(right, bottom));
    selection::selected_text(&invoice.pages[index].words, area).into_bytes().into()
}

#[no_mangle]
pub unsafe extern "C" fn giro_code(input: *const u8, len: usize) -> Bytes {
    let fields = serde_json::from_slice::<[String; 5]>(slice::from_raw_parts(input, len));
    let Ok([name, iban, bic, amount, purpose]) = fields else {
        return json_bytes(json!({"error": "Zahlungsdaten konnten nicht gelesen werden."}));
    };
    let fields = payment::Payment { name, iban, bic, amount, purpose };
    match fields.payload().and_then(qr_png) {
        Ok(png) => png.into(),
        Err(error) => json_bytes(json!({"error": error})),
    }
}

fn qr_png(payload: String) -> Result<Vec<u8>, String> {
    let code = qrcode::QrCode::with_error_correction_level(payload.as_bytes(), qrcode::EcLevel::M)
        .map_err(|error| error.to_string())?;
    let side = (code.width() + 8) as u32;
    let mut image = GrayImage::from_pixel(side, side, Luma([255]));
    for y in 0..code.width() {
        for x in 0..code.width() {
            if code[(x, y)] == qrcode::Color::Dark {
                image.put_pixel(x as u32 + 4, y as u32 + 4, Luma([0]));
            }
        }
    }
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageLuma8(image).write_to(&mut output, ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(output.into_inner())
}

#[no_mangle]
pub unsafe extern "C" fn giro_free(bytes: Bytes) {
    drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(bytes.data, bytes.len)));
}
