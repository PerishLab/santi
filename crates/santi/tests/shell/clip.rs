#[path = "../../src/client/tui/paint/clip.rs"]
#[allow(dead_code)]
mod inner;

use inner::{encode, osc52};

#[test]
fn padding() {
    assert_eq!(encode(b""), "");
    assert_eq!(encode(b"f"), "Zg==");
    assert_eq!(encode(b"fo"), "Zm8=");
    assert_eq!(encode(b"foo"), "Zm9v");
    assert_eq!(encode(b"foob"), "Zm9vYg==");
    assert_eq!(encode(b"fooba"), "Zm9vYmE=");
    assert_eq!(encode(b"foobar"), "Zm9vYmFy");
}

#[test]
fn multibyte() {
    assert_eq!(encode("你".as_bytes()), "5L2g");
    assert_eq!(osc52("你"), "\x1b]52;c;5L2g\x07");
}
