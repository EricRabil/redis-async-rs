/*
 * Copyright 2020 Ben Ashford
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

use bytes::{BufMut, BytesMut};

use crate::protocol::resp::RespValue;

fn check_and_reserve(buf: &mut BytesMut, amt: usize) {
    let remaining_bytes = buf.remaining_mut();
    if remaining_bytes < amt {
        buf.reserve(amt);
    }
}

fn write_rn(buf: &mut BytesMut) {
    buf.put_u8(b'\r');
    buf.put_u8(b'\n');
}

fn write_simple_string(symb: u8, string: &str, buf: &mut BytesMut) {
    let bytes = string.as_bytes();
    let size = 1 + bytes.len() + 2;
    check_and_reserve(buf, size);
    buf.put_u8(symb);
    buf.extend(bytes);
    write_rn(buf);
}

fn write_header(symb: u8, len: i64, buf: &mut BytesMut) {
    let len_as_string = len.to_string();
    let len_as_bytes = len_as_string.as_bytes();
    let header_bytes = 1 + len_as_bytes.len() + 2;
    check_and_reserve(buf, header_bytes);
    buf.put_u8(symb);
    buf.extend(len_as_bytes);
    write_rn(buf);
}

pub(crate) fn encode(msg: RespValue, buf: &mut BytesMut) {
    match msg {
        RespValue::Nil => {
            write_header(b'$', -1, buf);
        }
        RespValue::Array(ary) => {
            write_header(b'*', ary.len() as i64, buf);
            for v in ary {
                encode(v, buf);
            }
        }
        RespValue::BulkString(bstr) => {
            let len = bstr.len();
            write_header(b'$', len as i64, buf);
            check_and_reserve(buf, len + 2);
            buf.extend(bstr);
            write_rn(buf);
        }
        RespValue::Error(ref string) => {
            write_simple_string(b'-', string, buf);
        }
        RespValue::Integer(val) => {
            // Simple integer are just the header
            write_header(b':', val, buf);
        }
        RespValue::SimpleString(ref string) => {
            write_simple_string(b'+', string, buf);
        }
    }
}