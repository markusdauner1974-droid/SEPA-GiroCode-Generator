#[path = "../document.rs"] mod document;
#[path = "../ocr.rs"] mod ocr;
#[path = "../payment.rs"] mod payment;
#[path = "../selection.rs"] mod selection;
use std::path::Path;
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 2 { eprintln!("usage: giro-local-probe PDF"); std::process::exit(2); }
    match document::load(Path::new(&args[1]), false) {
        Ok((pages,text)) => {
            let (p,_) = payment::extract(&text);
            println!("{}", serde_json::json!({"pages":pages.len(),"name":p.name,"iban":p.iban,"bic":p.bic,"amount":p.amount,"purpose":p.purpose,"invoice_number":p.invoice_number,"customer_number":p.customer_number,"issuer":p.invoice_issuer}));
        },
        Err(e) => { eprintln!("{e}"); std::process::exit(1); }
    }
}
