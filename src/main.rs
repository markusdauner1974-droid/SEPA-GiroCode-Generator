mod document;
mod payment;
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions, Vec2};
use payment::Payment;
use qrcode::{QrCode, EcLevel, Color};
use std::sync::mpsc::{self,Receiver};

type Document = Result<(Vec<image::DynamicImage>,String),String>;
#[derive(Default)]
struct App {
    pages: Vec<image::DynamicImage>, index: usize, preview: Option<TextureHandle>,
    fields: Payment, accounts: Vec<String>, qr: Option<TextureHandle>, status: String,
    pending: Option<Receiver<Document>>, zoom: f32,
}
impl App {
    fn open(&mut self,ctx:&egui::Context) {
        let Some(path)=rfd::FileDialog::new().add_filter("Rechnung",&["pdf","png","jpg","jpeg"]).pick_file() else {return};
        *self=Self {zoom:1.0,status:"Rechnung wird lokal gelesen …".into(),..Default::default()};
        let (tx,rx)=mpsc::channel(); self.pending=Some(rx);
        let ctx=ctx.clone();
        std::thread::spawn(move || { let _=tx.send(document::load(&path)); ctx.request_repaint(); });
    }
    fn show_page(&mut self,ctx:&egui::Context) {
        let img=self.pages[self.index].to_rgba8();
        self.preview=Some(ctx.load_texture("Rechnung",ColorImage::from_rgba_unmultiplied(
            [img.width() as usize,img.height() as usize],&img),TextureOptions::LINEAR));
    }
    fn make_qr(&mut self,ctx:&egui::Context)->Result<(),String>{
        let data=self.fields.payload()?;
        let code=QrCode::with_error_correction_level(data.as_bytes(),EcLevel::M).map_err(|e|e.to_string())?;
        let side=code.width()+8;
        let mut bytes=vec![255u8;side*side];
        for y in 0..code.width(){ for x in 0..code.width(){
            if code[(x,y)]==Color::Dark { bytes[(y+4)*side+x+4]=0; }
        }}
        self.qr=Some(ctx.load_texture("GiroCode",ColorImage::from_gray([side,side],&bytes),TextureOptions::NEAREST));
        Ok(())
    }
}
impl eframe::App for App {
    fn update(&mut self,ctx:&egui::Context,_:&mut eframe::Frame) {
        if let Some(result)=self.pending.as_ref().and_then(|rx|rx.try_recv().ok()){
            self.pending=None;
            match result {
                Ok((pages,text))=>{self.pages=pages; (self.fields,self.accounts)=payment::extract(&text);
                    self.show_page(ctx); self.status="Zahlungsdaten mit der Rechnung abgleichen.".into();}
                Err(e)=>self.status=e,
            }
        }
        egui::TopBottomPanel::top("Werkzeuge").show(ctx,|ui|{ui.horizontal_wrapped(|ui|{
            if ui.add_enabled(self.pending.is_none(),egui::Button::new("Rechnung öffnen …")).clicked(){self.open(ctx);}
            for (label,step) in [("Schrift −",-0.1),("Schrift +",0.1)] {
                if ui.button(label).clicked(){ctx.set_zoom_factor((ctx.zoom_factor()+step).clamp(0.8,2.4));}
            }
            ui.label(&self.status);
        });});
        egui::SidePanel::right("Zahlung").resizable(true).default_width(370.0).show(ctx,|ui|{
            egui::ScrollArea::vertical().show(ui,|ui|{
                let mut changed=false;
                if self.accounts.len()>1 {
                    egui::ComboBox::from_label("Gefundene IBANs").selected_text(&self.fields.iban).show_ui(ui,|ui|{
                        for s in &self.accounts {changed|=ui.selectable_value(&mut self.fields.iban,s.clone(),s).changed();}
                    });
                }
                let p=&mut self.fields;
                for (title,v) in [("Empfänger",&mut p.name),("IBAN",&mut p.iban),("BIC",&mut p.bic),
                    ("Betrag in Euro",&mut p.amount),("Verwendungszweck",&mut p.purpose)]{
                    ui.label(title);changed|=ui.add(egui::TextEdit::singleline(v).desired_width(f32::INFINITY)).changed();
                }
                if changed{self.qr=None;}
                if ui.add_enabled(!self.pages.is_empty(),egui::Button::new("GiroCode erstellen")).clicked(){
                    match self.make_qr(ctx){Ok(())=>self.status.clear(),Err(e)=>self.status=e}
                }
                if let Some(qr)=&self.qr{
                    let unit=(ui.available_width().min(420.0)/qr.size()[0] as f32).floor().max(1.0);
                    ui.image((qr.id(),Vec2::splat(unit*qr.size()[0] as f32)));
                }
            });
        });
        egui::CentralPanel::default().show(ctx,|ui|{
            if self.pages.is_empty(){return;}
            ui.horizontal(|ui|{
                if ui.add_enabled(self.index>0,egui::Button::new("‹")).clicked(){self.index-=1;self.show_page(ctx);}
                if ui.add_enabled(self.index+1<self.pages.len(),egui::Button::new("›")).clicked(){self.index+=1;self.show_page(ctx);}
                ui.label(format!("Seite {} von {}",self.index+1,self.pages.len()));
                if ui.button("−").clicked(){self.zoom=(self.zoom-0.25).max(0.5);}
                if ui.button("+").clicked(){self.zoom=(self.zoom+0.25).min(4.0);}
            });
            if let Some(t)=&self.preview{
                let width=ui.available_width()*self.zoom;
                egui::ScrollArea::both().show(ui,|ui|{ui.image((t.id(),Vec2::new(width,width * t.size()[1] as f32 / t.size()[0] as f32)));});
            }
        });
    }
}
fn main()->eframe::Result {
    let options=eframe::NativeOptions{viewport:egui::ViewportBuilder::default().with_maximized(true),..Default::default()};
    eframe::run_native("Rechnung zu GiroCode",options,Box::new(|_| Ok(Box::new(App::default()))))
}
