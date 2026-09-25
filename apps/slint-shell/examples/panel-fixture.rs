use spell_framework::{
    SpellAssociatedNew,
    layer_properties::{BoardType, LayerAnchor, LayerType, WindowConf},
    wayland_adapter::SpellWin,
};
slint::slint! {
    export component Panel inherits Window {
        width: 1920px; height: 48px;
        Text { text: "Reserved 48px panel fixture"; }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !std::env::var("WAYLAND_DISPLAY").is_ok_and(|v| v.starts_with("lhc-stage3a-")) {
        return Err("panel fixture requires isolated virtual compositor".into());
    }
    let mut config = WindowConf::builder();
    config
        .width(1920_u32)
        .height(48_u32)
        .anchor_1(LayerAnchor::BOTTOM)
        .layer_type(LayerType::Top)
        .exclusive_zone(48)
        .board_interactivity(BoardType::None);
    let mut way = SpellWin::invoke_spell("lhc-panel-fixture", config.build()?);
    let _ui = Panel::new()?;
    loop {
        way.on_call()?;
    }
}
