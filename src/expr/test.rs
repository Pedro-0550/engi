use crate::{symbol::Symbol, symbols};

#[test]
fn formatting() {
    symbols!(x, y, z);
    let expr = (x * y * z * 10) + 5;

    assert_eq!("{expr}", "")
}
