use eframe::egui::{self, pos2, Pos2, Rect};

pub struct Word {
    pub bounds: Rect,
    pub text: String,
    line: String,
}

// Both OCR engines report rectangles relative to the page in their TSV output.
pub fn words(tsv: &str) -> Vec<Word> {
    let mut size = None;
    let mut result = Vec::new();
    for row in tsv.lines().skip(1) {
        let f: Vec<_> = row.splitn(12, '\t').collect();
        if f.len() != 12 { continue; }
        let coord = |i: usize| f[i].parse::<f32>().ok().filter(|v| v.is_finite());
        let (Some(x), Some(y), Some(w), Some(h)) = (coord(6),coord(7),coord(8),coord(9)) else {continue};
        if f[0] == "1" && w > 0.0 && h > 0.0 { size = Some((w,h)); }
        if f[0] != "5" || f[11].trim().is_empty() { continue; }
        if let Some((pw,ph)) = size {
            result.push(Word {
                bounds: Rect::from_min_max(pos2(x/pw,y/ph),pos2((x+w)/pw,(y+h)/ph)),
                text: f[11].trim().to_owned(),
                line: f[1..5].join("/"),
            });
        }
    }
    result
}

#[derive(Default)]
pub struct Selection { anchor: Option<Pos2>, area: Option<Rect> }

impl Selection {
    pub fn text(&self, words: &[Word]) -> String {
        let mut text = String::new();
        let mut line = None;
        for w in words.iter().filter(|w| self.area.is_some_and(|a| a.intersects(w.bounds))) {
            if !text.is_empty() { text.push(if line == Some(&w.line) {' '} else {'\n'}); }
            text.push_str(&w.text);
            line = Some(&w.line);
        }
        text
    }

    pub fn interact(&mut self, ui: &mut egui::Ui, response: &egui::Response, words: &[Word]) {
        let rect = response.rect;
        let transform = egui::emath::RectTransform::from_to(Rect::from_min_max(Pos2::ZERO,pos2(1.,1.)),rect);
        let local = |p| transform.inverse().transform_pos_clamped(p);
        if response.clicked() || response.drag_started() {
            response.request_focus();
            self.anchor = ui.input(|i| i.pointer.press_origin()).or(response.interact_pointer_pos()).map(local);
            self.area = self.anchor.map(|p| Rect::from_min_max(p,p));
        }
        if (response.dragged() || response.drag_stopped()) && self.anchor.is_some() {
            if let Some(p) = response.interact_pointer_pos() {
                self.area = Some(Rect::from_two_pos(self.anchor.unwrap(),local(p)));
            }
        }
        if let Some(area) = self.area {
            let painter = ui.painter_at(rect);
            for w in words.iter().filter(|w| area.intersects(w.bounds)) {
                painter.rect_filled(transform.transform_rect(w.bounds),0.,egui::Color32::from_rgba_unmultiplied(35,125,235,88));
            }
            if response.has_focus() && ui.input(|i| i.events.iter().any(|e| matches!(e,egui::Event::Copy))) {
                let text = self.text(words);
                if !text.is_empty() { ui.ctx().copy_text(text); }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copies_words_in_selected_column_and_preserves_lines() {
        let data = "header\n1\t1\t0\t0\t0\t0\t0\t0\t200\t200\t-1\tpage\n5\t1\t1\t1\t1\t1\t10\t10\t30\t12\t98\tAndere\n5\t1\t1\t2\t1\t1\t90\t10\t45\t12\t98\tMüller\n5\t1\t1\t2\t2\t1\t90\t30\t45\t12\t98\tGmbH";
        let words = words(data);
        let s = Selection {anchor:None,area:Some(Rect::from_two_pos(pos2(0.8,0.22),pos2(0.44,0.04)))};
        assert_eq!(s.text(&words),"Müller\nGmbH");
    }
}
