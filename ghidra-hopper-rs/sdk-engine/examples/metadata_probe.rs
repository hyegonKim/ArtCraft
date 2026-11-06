use ghidra_hopper_rs::macho_objc::MachOImage;
fn main() {
    let path = std::env::args().nth(1).expect("binary path");
    let bytes = std::fs::read(path).unwrap();
    let image = MachOImage::parse(&bytes).unwrap();
    let refs: Vec<_> = image
        .objc
        .selector_references
        .iter()
        .map(|(a, n)| serde_json::json!({"address":format!("0x{a:x}"),"selector":n}))
        .collect();
    let stubs:Vec<_>=image.objc.selector_stub_proofs.iter().map(|(a,p)|serde_json::json!({"address":format!("0x{a:x}"),"selref":format!("0x{:x}",p.selref),"target":format!("0x{:x}",p.target),"selector":p.selector})).collect();
    println!("{}", serde_json::json!({"selectors":refs,"stubs":stubs}));
}
