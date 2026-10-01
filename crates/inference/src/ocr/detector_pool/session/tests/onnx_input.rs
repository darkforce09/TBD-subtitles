use super::*;

fn varint_bytes(mut value: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            bytes.push(byte);
            return bytes;
        }
        bytes.push(byte | 0x80);
    }
}

fn length_field(number: u64, body: &[u8]) -> Vec<u8> {
    let mut field = varint_bytes(number << 3 | 2);
    field.extend(varint_bytes(body.len() as u64));
    field.extend_from_slice(body);
    field
}

fn varint_field(number: u64, value: u64) -> Vec<u8> {
    let mut field = varint_bytes(number << 3);
    field.extend(varint_bytes(value));
    field
}

fn model(graph: &[Vec<u8>]) -> Vec<u8> {
    let mut model = varint_field(1, 8);
    model.extend(length_field(2, b"paddle2onnx"));
    model.extend(length_field(7, &graph.concat()));
    model
}

fn input(name: &str) -> Vec<u8> {
    let mut value_info = length_field(1, name.as_bytes());
    value_info.extend(length_field(2, &[0x0a, 0x00]));
    length_field(11, &value_info)
}

fn initializer(name: &str) -> Vec<u8> {
    let mut tensor = varint_field(1, 3);
    tensor.extend(length_field(13, &[0u8; 300]));
    tensor.extend(length_field(8, name.as_bytes()));
    length_field(5, &tensor)
}

#[test]
fn the_first_input_that_is_not_an_initializer_is_named() {
    let graph = [
        length_field(1, b"a node"),
        initializer("conv.weight"),
        input("conv.weight"),
        input("x"),
        input("y"),
    ];
    assert_eq!(first_input_name(&model(&graph)).unwrap(), "x");
}

#[test]
fn fixed_width_fields_are_skipped() {
    let mut fixed = varint_bytes(20 << 3 | 1);
    fixed.extend([0u8; 8]);
    fixed.extend(varint_bytes(21 << 3 | 5));
    fixed.extend([0u8; 4]);
    let graph = [fixed, input("images")];
    assert_eq!(first_input_name(&model(&graph)).unwrap(), "images");
}

#[test]
fn malformed_models_are_errors() {
    assert!(first_input_name(&varint_field(1, 8)).is_err());
    let mut truncated = model(&[input("x")]);
    truncated.truncate(truncated.len() - 2);
    assert!(first_input_name(&truncated).is_err());
    assert!(first_input_name(&model(&[initializer("w"), input("w")])).is_err());
    assert!(first_input_name(&[0x0b]).is_err());
}
