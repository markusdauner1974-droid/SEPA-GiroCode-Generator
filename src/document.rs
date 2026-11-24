use std::{ffi::OsStr, path::Path};
use image::{DynamicImage, GenericImageView};
use glib::translate::ToGlibPtr;
use crate::selection::{self, Word};

fn layout_text(tsv: &str) -> Result<String, String> {
    let mut words = Vec::new();
    let mut heights = Vec::new();
    let mut units = Vec::new();
    for row in tsv.lines().skip(1) {
        let f: Vec<_> = row.splitn(12,'\t').collect();
        if f.len()!=12 || f[0]!="5" || f[11].trim().is_empty() { continue; }
        let coords: Result<Vec<i32>,_> = f[6..10].iter().map(|s| s.parse()).collect();
        let v = coords.map_err(|_| "Ungültige OCR-Koordinaten.")?;
        words.push((v[1]+v[3]/2,v[0],v[2],f[11].trim()));
        heights.push(v[3]);
        units.push(v[2]/f[11].chars().count().max(1) as i32);
    }
    if words.is_empty() { return Ok(String::new()); }
    heights.sort(); units.sort(); words.sort();
    let tolerance = (heights[heights.len()/2]*65/100).max(1);
    let unit = (units[units.len()/2]*8/10).max(1);
    let mut text = String::new();
    let mut i = 0;
    while i<words.len() {
        let mut j=i+1;
        while j<words.len() && words[j].0-words[i].0<=tolerance { j+=1; }
        words[i..j].sort_by_key(|w| w.1);
        let mut line=String::new();
        let mut end=None;
        for &(_,x,width,word) in &words[i..j] {
            let spaces = if end.is_none() || x-end.unwrap_or(0)>3*unit {
                (x/unit-line.chars().count() as i32).max(if end.is_some(){2}else{0})
            } else {1};
            line.push_str(&" ".repeat(spaces as usize));
            line.push_str(word); end=Some(x+width);
        }
        text.push_str(&line); text.push('\n'); i=j;
    }
    Ok(text)
}

pub struct Page {
    pub image: DynamicImage,
    pub words: Vec<crate::selection::Word>,
}

fn pdf_text(page: &poppler::Page, width: f64, height: f64) -> Result<(String, Vec<Word>), String> {
    let Some(text) = page.text() else { return Ok((String::new(), Vec::new())); };
    let mut boxes = std::ptr::null_mut();
    let mut count = 0;
    // Poppler allocates one rectangle per character. Copy before freeing with GLib.
    let boxes = unsafe {
        poppler::ffi::poppler_page_get_text_layout(page.to_glib_none().0, &mut boxes, &mut count);
        let values = if boxes.is_null() {
            Vec::new()
        } else {
            std::slice::from_raw_parts(boxes, count as usize).to_vec()
        };
        glib::ffi::g_free(boxes.cast());
        values
    };
    if text.chars().count() != boxes.len() {
        return Ok((text.to_string(), Vec::new()));
    }

    let scale = 2100.0 / width.max(height);
    let mut tsv = format!("header\n1\t1\t0\t0\t0\t0\t0\t0\t{}\t{}\t-1\tpage\n",
        (width * scale).ceil(), (height * scale).ceil());
    let mut line = 1;
    let mut token = String::new();
    let mut bounds: Option<(f64, f64, f64, f64)> = None;
    let mut previous: Option<(f64, f64, f64)> = None;
    for (ch, rect) in text.chars().zip(boxes).chain(std::iter::once(('\n', poppler::ffi::PopplerRectangle {
        x1: 0., y1: 0., x2: 0., y2: 0.,
    }))) {
        // Poppler sometimes returns adjacent columns without a separating
        // character. Split a word when the glyphs jump to another line or column.
        let height = (rect.y2 - rect.y1).max(1.);
        let y = (rect.y1 + rect.y2) / 2.;
        let crossed_gap = previous.is_some_and(|(right, center, prev_height)| {
            let size = height.max(prev_height);
            rect.x1 - right > size * 0.85 || (y - center).abs() > size * 0.7
        });
        if ch.is_whitespace() || crossed_gap {
            if let Some((x1, y1, x2, y2)) = bounds.take() {
                use std::fmt::Write;
                writeln!(tsv, "5\t1\t0\t0\t{line}\t0\t{}\t{}\t{}\t{}\t100\t{token}",
                    (x1 * scale).round(), (y1 * scale).round(),
                    ((x2 - x1) * scale).round(), ((y2 - y1) * scale).round()).unwrap();
                token.clear();
            }
        }
        if ch.is_whitespace() {
            previous = None;
            if ch == '\n' { line += 1; }
            continue;
        }
        token.push(ch);
        bounds = Some(match bounds {
            Some((x1, y1, x2, y2)) => (x1.min(rect.x1), y1.min(rect.y1), x2.max(rect.x2), y2.max(rect.y2)),
            None => (rect.x1, rect.y1, rect.x2, rect.y2),
        });
        previous = Some((rect.x2, y, height));
    }
    Ok((layout_text(&tsv)?, selection::words(&tsv)))
}

fn render(page: &poppler::Page, width: f64, height: f64, long_edge: f64) -> Result<DynamicImage, String> {
    if !(width.is_finite() && height.is_finite() && width > 0. && height > 0.) {
        return Err("Ungültige PDF-Seitengröße.".into());
    }
    let scale = long_edge / width.max(height);
    let surface = cairo::ImageSurface::create(cairo::Format::Rgb24,
        (width * scale).ceil() as i32, (height * scale).ceil() as i32).map_err(|e| e.to_string())?;
    let context = cairo::Context::new(&surface).map_err(|e| e.to_string())?;
    context.set_source_rgb(1., 1., 1.);
    context.paint().map_err(|e| e.to_string())?;
    context.scale(scale, scale);
    page.render(&context);
    let mut bytes = Vec::new();
    surface.write_to_png(&mut bytes).map_err(|e| e.to_string())?;
    image::load_from_memory(&bytes).map_err(|e| e.to_string())
}

pub fn load(path: &Path, force_ocr: bool) -> Result<(Vec<Page>, String), String> {
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let mut pages = Vec::new();
    let mut text = String::new();
    let mut ocr = None;
    if path.extension().and_then(OsStr::to_str).is_some_and(|s| s.eq_ignore_ascii_case("pdf")) {
        let uri = glib::filename_to_uri(&path, None).map_err(|e| e.to_string())?;
        let doc = poppler::Document::from_file(uri.as_str(), None).map_err(|e| e.to_string())?;
        for index in 0..doc.n_pages() {
            let page = doc.page(index).ok_or_else(|| format!("PDF-Seite {} fehlt.", index + 1))?;
            let (width, height) = page.size();
            let (embedded_text, embedded_words) = if force_ocr {
                (String::new(), Vec::new())
            } else {
                pdf_text(&page, width, height)?
            };
            let needs_ocr = force_ocr || embedded_text.trim().chars().count() < 24;
            let image = render(&page, width, height, if needs_ocr { 3200. } else { 2100. })?;
            let (part, words) = if needs_ocr {
                recognize(&mut ocr, &image)?
            } else {
                (embedded_text, embedded_words)
            };
            text.push_str(&part);
            text.push('\n');
            pages.push(Page { image, words });
        }
    } else {
        use image::ImageDecoder;
        let mut decoder = image::ImageReader::open(&path).map_err(|e| e.to_string())?
            .into_decoder().map_err(|e| e.to_string())?;
        let orientation = decoder.orientation().map_err(|e| e.to_string())?;
        let mut image = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
        image.apply_orientation(orientation);
        let (width, height) = image.dimensions();
        if width.max(height) > 3200 { image = image.thumbnail(3200, 3200); }
        let (part, words) = recognize(&mut ocr, &image)?;
        text = part;
        pages.push(Page { image, words });
    }
    if pages.is_empty() { return Err("Keine lesbare Seite gefunden.".into()); }
    Ok((pages, text))
}

fn recognize(engine: &mut Option<crate::ocr::Engine>, image: &DynamicImage) -> Result<(String, Vec<Word>), String> {
    if engine.is_none() { *engine = Some(crate::ocr::Engine::new()?); }
    let tsv = engine.as_mut().unwrap().recognize(image)?;
    Ok((layout_text(&tsv)?, selection::words(&tsv)))
}

#[cfg(test)]
mod library_tests {
    use super::*;

    #[test]
    #[ignore = "Manueller Vergleich mit anonymisiertem OCR-Testbild"]
    fn end_to_end_ocr_ergebnis() {
        let path = std::env::var("GIRO_TEST_IMAGE").expect("GIRO_TEST_IMAGE fehlt");
        let (_, text) = load(Path::new(&path), false).expect("Rechnung konnte nicht gelesen werden");
        let (payment, _) = crate::payment::extract(&text);
        println!("ERKANNTER TEXT:\n{text}");
        println!("FELDER: Empfänger={:?}; IBAN={:?}; Betrag={:?}; Rechnungsnummer={:?}; Kundennummer={:?}; Zweck={:?}",
            payment.name, payment.iban, payment.amount, payment.invoice_number, payment.customer_number, payment.purpose);
        assert_eq!(payment.name, "Jürgen Möller Heiztechnik-Solaranlagen");
        assert_eq!(payment.iban, "DE89370400440532013000");
        assert_eq!(payment.amount, "297,50");
        assert_eq!(payment.invoice_number, "RG2026/082");
        assert_eq!(payment.customer_number, "13757");
    }

    #[test]
    fn poppler_reads_pdf_text_and_positions() {
        let surface = cairo::PdfSurface::for_stream(600., 400., Vec::new()).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        cr.set_font_size(16.);
        cr.move_to(36., 48.);
        cr.show_text("Kunde").unwrap();
        cr.move_to(300., 48.);
        cr.show_text("Beispiel GmbH").unwrap();
        cr.show_page().unwrap();
        let pdf = *surface.finish_output_stream().unwrap().downcast::<Vec<u8>>().unwrap();
        let doc = poppler::Document::from_data(&pdf, None).unwrap();
        let page = doc.page(0).unwrap();
        let (width, height) = page.size();
        let (text, words) = pdf_text(&page, width, height).unwrap();
        assert!(text.lines().any(|line| line.contains("Kunde") && line.contains("Beispiel GmbH")));
        let word = words.iter().find(|word| word.text == "Beispiel").unwrap();
        assert!((word.bounds.min.x - 0.5).abs() < 0.02);
        assert_eq!(render(&page, width, height, 2100.).unwrap().width(), 2100);
    }

    #[test]
    fn reads_amount_at_right_edge_of_invoice() {
        let surface = cairo::PdfSurface::for_stream(600., 400., Vec::new()).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        cr.set_font_size(12.);
        for (x, y, text) in [
            (25., 32., "Musterkunde"),
            (300., 32., "Muster Steuerberatungsgesellschaft"),
            (300., 48., "GmbH & Co. KG"),
            (300., 70., "USt-ID: TEST"),
            (300., 110., "Rechnung"),
            (300., 140., "Rechnungsnummer:"),
            (520., 140., "2026/0123"),
            (25., 200., "Rechnungssumme netto"),
            (540., 200., "42,00"),
            (25., 222., "Zu zahlender Betrag"),
            (540., 222., "49,98"),
            (25., 290., "Musterbank IBAN: DE89370400440532013000 BIC: COBADEFFXXX"),
        ] {
            cr.move_to(x, y);
            cr.show_text(text).unwrap();
        }
        cr.show_page().unwrap();
        let pdf = *surface.finish_output_stream().unwrap().downcast::<Vec<u8>>().unwrap();
        let doc = poppler::Document::from_data(&pdf, None).unwrap();
        let (text, _) = pdf_text(&doc.page(0).unwrap(), 600., 400.).unwrap();
        let (payment, _) = crate::payment::extract(&text);
        assert_eq!(payment.name, "Muster Steuerberatungsgesellschaft GmbH & Co. KG", "{text}");
        assert_eq!(payment.iban, "DE89370400440532013000", "{text}");
        assert_eq!(payment.bic, "COBADEFFXXX", "{text}");
        assert_eq!(payment.amount, "49,98", "{text}");
        assert_eq!(payment.purpose, "Rechnung 2026/0123", "{text}");
    }

    #[test]
    fn tesseract_reads_pale_red_and_blue_text_with_positions() {
        let surface = cairo::ImageSurface::create(cairo::Format::Rgb24, 1050, 260).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        cr.set_source_rgb(1., 1., 1.);
        cr.paint().unwrap();
        cr.set_font_size(53.);
        cr.set_source_rgb(0.96, 0.83, 0.81);
        cr.move_to(25., 95.);
        cr.show_text("Muster Heiztechnik").unwrap();
        cr.set_source_rgb(0.77, 0.90, 0.97);
        cr.move_to(25., 191.);
        cr.show_text("Muster Abrechnung").unwrap();
        let mut bytes = Vec::new();
        surface.write_to_png(&mut bytes).unwrap();
        let image = image::load_from_memory(&bytes).unwrap();
        let (text, words) = recognize(&mut None, &image).unwrap();
        assert!(text.contains("Muster Heiztechnik"), "{text:?}");
        assert!(text.contains("Muster Abrechnung"), "{text:?}");
        assert!(words.iter().any(|word| word.text == "Heiztechnik" && word.bounds.is_positive()));
    }

    #[test]
    fn tesseract_reads_image_text_and_positions() {
        let surface = cairo::ImageSurface::create(cairo::Format::Rgb24, 530, 120).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        cr.set_source_rgb(1., 1., 1.);
        cr.paint().unwrap();
        cr.set_source_rgb(0., 0., 0.);
        cr.set_font_size(46.);
        cr.move_to(20., 80.);
        cr.show_text("Rechnung 123").unwrap();
        let mut bytes = Vec::new();
        surface.write_to_png(&mut bytes).unwrap();
        let image = image::load_from_memory(&bytes).unwrap();
        let (text, words) = recognize(&mut None, &image).unwrap();
        assert!(text.contains("Rechnung 123"), "{text:?}");
        assert!(words.iter().any(|word| word.text == "123" && word.bounds.is_positive()));
    }
}
