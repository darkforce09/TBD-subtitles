//! The name of an ONNX model's input, read from the model file.
//!
//! **Role:** walk the model's protobuf encoding far enough to find the graph's first input that
//! is not an initializer, the name TensorRT's optimisation profile must use before any session
//! exists to ask.
//!
//! **Position:** used by `ort_detector.rs` before it opens a TensorRT session; the session's own
//! input name is checked against it afterwards.
//!
//! **Signals and state:** none; reads a byte slice.
//!
//! **Invariants:** malformed or truncated encodings are errors, never a guessed name; only the
//! fields named below are read (`ModelProto.graph` 7, `GraphProto.initializer` 5 and `input` 11,
//! `TensorProto.name` 8, `ValueInfoProto.name` 1), everything else is skipped by its wire type.

use std::collections::HashSet;

use crate::ocr::OcrError;

const MODEL_GRAPH: u64 = 7;
const GRAPH_INITIALIZER: u64 = 5;
const GRAPH_INPUT: u64 = 11;
const TENSOR_NAME: u64 = 8;
const VALUE_INFO_NAME: u64 = 1;

/// One field of a protobuf message: its number and, for length-delimited fields, its bytes.
struct Field<'a> {
    number: u64,
    bytes: Option<&'a [u8]>,
}

/// The fields of one message, in order.
fn fields(mut message: &[u8]) -> Result<Vec<Field<'_>>, OcrError> {
    let mut found = Vec::new();
    while !message.is_empty() {
        let tag = varint(&mut message)?;
        let number = tag >> 3;
        let bytes = match tag & 7 {
            0 => {
                varint(&mut message)?;
                None
            }
            1 => {
                take(&mut message, 8)?;
                None
            }
            2 => {
                let length = usize::try_from(varint(&mut message)?)
                    .map_err(|_| "an ONNX field is longer than memory")?;
                Some(take(&mut message, length)?)
            }
            5 => {
                take(&mut message, 4)?;
                None
            }
            wire => return Err(format!("the ONNX model has a field of wire type {wire}").into()),
        };
        found.push(Field { number, bytes });
    }
    Ok(found)
}

fn varint(message: &mut &[u8]) -> Result<u64, OcrError> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let (&byte, rest) = message
            .split_first()
            .ok_or("the ONNX model ends inside a number")?;
        *message = rest;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err("the ONNX model holds a number longer than 64 bits".into())
}

fn take<'a>(message: &mut &'a [u8], length: usize) -> Result<&'a [u8], OcrError> {
    if message.len() < length {
        return Err("the ONNX model ends inside a field".into());
    }
    let (taken, rest) = message.split_at(length);
    *message = rest;
    Ok(taken)
}

/// The text of the first length-delimited field `number` of `message`.
fn name(message: &[u8], number: u64) -> Result<String, OcrError> {
    let bytes = fields(message)?
        .into_iter()
        .find(|field| field.number == number)
        .and_then(|field| field.bytes)
        .ok_or("an ONNX value has no name")?;
    Ok(String::from_utf8(bytes.to_vec()).map_err(|_| "an ONNX name is not UTF-8")?)
}

/// The name of the graph's first input that is not an initializer.
pub fn first_input_name(model: &[u8]) -> Result<String, OcrError> {
    let graph = fields(model)?
        .into_iter()
        .find(|field| field.number == MODEL_GRAPH)
        .and_then(|field| field.bytes)
        .ok_or("the ONNX model has no graph")?;
    let mut initializers = HashSet::new();
    let mut inputs = Vec::new();
    for field in fields(graph)? {
        match (field.number, field.bytes) {
            (GRAPH_INITIALIZER, Some(bytes)) => {
                initializers.insert(name(bytes, TENSOR_NAME)?);
            }
            (GRAPH_INPUT, Some(bytes)) => inputs.push(name(bytes, VALUE_INFO_NAME)?),
            _ => {}
        }
    }
    inputs
        .into_iter()
        .find(|input| !initializers.contains(input))
        .ok_or_else(|| "the ONNX graph has no input".into())
}

#[cfg(test)]
#[path = "tests/onnx_input.rs"]
mod tests;
