pub const AFF: &str = "SET UTF-8\nTRY aeiouyáéíóúýčďěňřšťžbcdfghjklmnpqrstvwxz\nMAP 3\nMAP cč\nMAP ií\nMAP sš\nREP 1\nREP pocitac počítač\n";
pub const DIC: &str = "3\nměsto\npočítač\nškola\n";

pub fn install() {
    let _turn = super::dictionaries_turn();
    super::install_dictionary(AFF, DIC).unwrap();
}
