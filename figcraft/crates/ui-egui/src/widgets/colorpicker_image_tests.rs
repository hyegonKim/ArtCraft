use super::*;
use crate::layers_drag_tests::Ui;
use figcraft_engine::Session;

#[test]
fn crop_on_canvas_applies_only_to_single_layer_image_fills() {
    let mut s = Session::new();
    let png = {
        let mut buf = Vec::new();
        let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]));
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).unwrap();
        buf
    };
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
    let id = NodeId(s.execute("image.place", &json!({"data": b64, "x": 0, "y": 0})).unwrap()["id"].as_u64().unwrap());
    let other = NodeId(s.execute("shape.rectangle", &json!({"width": 20, "height": 20})).unwrap()["id"].as_u64().unwrap());
    let u = Ui::new(s);

    assert_eq!(crop_on_canvas(&u.app, &PickerTarget::Paint { fills: true, index: 0, ids: vec![id] }), Some((id, 0)));
    assert_eq!(crop_on_canvas(&u.app, &PickerTarget::Paint { fills: false, index: 0, ids: vec![id] }), None);
    assert_eq!(crop_on_canvas(&u.app, &PickerTarget::Paint { fills: true, index: 0, ids: vec![id, other] }), None);
    assert_eq!(crop_on_canvas(&u.app, &PickerTarget::Paint { fills: true, index: 0, ids: vec![other] }), None);
}
