// Automatically generated rust module for 'rpc.proto' file

#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(unused_imports)]
#![allow(unknown_lints)]
#![allow(clippy::all)]
#![cfg_attr(rustfmt, rustfmt_skip)]


use quick_protobuf::{MessageInfo, MessageRead, MessageWrite, BytesReader, Writer, WriterBackend, Result};
use quick_protobuf::sizeofs::*;
use super::super::*;

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RPC {
    pub txs: Vec<dog::pb::Tx>,
}

impl<'a> MessageRead<'a> for RPC {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.txs.push(r.read_message::<dog::pb::Tx>(bytes)?),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl MessageWrite for RPC {
    fn get_size(&self) -> usize {
        0
        + self.txs.iter().map(|s| 1 + sizeof_len((s).get_size())).sum::<usize>()
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        for s in &self.txs { w.write_with_tag(10, |w| w.write_message(s))?; }
        Ok(())
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Tx {
    pub from: Vec<u8>,
    pub tx_id: Vec<u8>,
    pub data: Vec<u8>,
}

impl<'a> MessageRead<'a> for Tx {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.from = r.read_bytes(bytes)?.to_owned(),
                Ok(18) => msg.tx_id = r.read_bytes(bytes)?.to_owned(),
                Ok(26) => msg.data = r.read_bytes(bytes)?.to_owned(),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl MessageWrite for Tx {
    fn get_size(&self) -> usize {
        0
        + if self.from.is_empty() { 0 } else { 1 + sizeof_len((&self.from).len()) }
        + if self.tx_id.is_empty() { 0 } else { 1 + sizeof_len((&self.tx_id).len()) }
        + if self.data.is_empty() { 0 } else { 1 + sizeof_len((&self.data).len()) }
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if !self.from.is_empty() { w.write_with_tag(10, |w| w.write_bytes(&**&self.from))?; }
        if !self.tx_id.is_empty() { w.write_with_tag(18, |w| w.write_bytes(&**&self.tx_id))?; }
        if !self.data.is_empty() { w.write_with_tag(26, |w| w.write_bytes(&**&self.data))?; }
        Ok(())
    }
}

