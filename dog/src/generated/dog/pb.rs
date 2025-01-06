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
    pub txs: Vec<dog::pb::Transaction>,
    pub control: Option<dog::pb::ControlMessage>,
}

impl<'a> MessageRead<'a> for RPC {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.txs.push(r.read_message::<dog::pb::Transaction>(bytes)?),
                Ok(18) => msg.control = Some(r.read_message::<dog::pb::ControlMessage>(bytes)?),
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
        + self.control.as_ref().map_or(0, |m| 1 + sizeof_len((m).get_size()))
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        for s in &self.txs { w.write_with_tag(10, |w| w.write_message(s))?; }
        if let Some(ref s) = self.control { w.write_with_tag(18, |w| w.write_message(s))?; }
        Ok(())
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Transaction {
    pub from: Vec<u8>,
    pub seqno: u64,
    pub data: Vec<u8>,
}

impl<'a> MessageRead<'a> for Transaction {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.from = r.read_bytes(bytes)?.to_owned(),
                Ok(16) => msg.seqno = r.read_uint64(bytes)?,
                Ok(26) => msg.data = r.read_bytes(bytes)?.to_owned(),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl MessageWrite for Transaction {
    fn get_size(&self) -> usize {
        0
        + if self.from.is_empty() { 0 } else { 1 + sizeof_len((&self.from).len()) }
        + if self.seqno == 0u64 { 0 } else { 1 + sizeof_varint(*(&self.seqno) as u64) }
        + if self.data.is_empty() { 0 } else { 1 + sizeof_len((&self.data).len()) }
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if !self.from.is_empty() { w.write_with_tag(10, |w| w.write_bytes(&**&self.from))?; }
        if self.seqno != 0u64 { w.write_with_tag(16, |w| w.write_uint64(*&self.seqno))?; }
        if !self.data.is_empty() { w.write_with_tag(26, |w| w.write_bytes(&**&self.data))?; }
        Ok(())
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ControlMessage {
    pub have_tx: Vec<dog::pb::ControlHaveTx>,
    pub reset_route: Vec<dog::pb::ControlResetRoute>,
}

impl<'a> MessageRead<'a> for ControlMessage {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.have_tx.push(r.read_message::<dog::pb::ControlHaveTx>(bytes)?),
                Ok(18) => msg.reset_route.push(r.read_message::<dog::pb::ControlResetRoute>(bytes)?),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl MessageWrite for ControlMessage {
    fn get_size(&self) -> usize {
        0
        + self.have_tx.iter().map(|s| 1 + sizeof_len((s).get_size())).sum::<usize>()
        + self.reset_route.iter().map(|s| 1 + sizeof_len((s).get_size())).sum::<usize>()
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        for s in &self.have_tx { w.write_with_tag(10, |w| w.write_message(s))?; }
        for s in &self.reset_route { w.write_with_tag(18, |w| w.write_message(s))?; }
        Ok(())
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ControlHaveTx {
    pub from: Vec<u8>,
}

impl<'a> MessageRead<'a> for ControlHaveTx {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.from = r.read_bytes(bytes)?.to_owned(),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl MessageWrite for ControlHaveTx {
    fn get_size(&self) -> usize {
        0
        + if self.from.is_empty() { 0 } else { 1 + sizeof_len((&self.from).len()) }
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if !self.from.is_empty() { w.write_with_tag(10, |w| w.write_bytes(&**&self.from))?; }
        Ok(())
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ControlResetRoute { }

impl<'a> MessageRead<'a> for ControlResetRoute {
    fn from_reader(r: &mut BytesReader, _: &[u8]) -> Result<Self> {
        r.read_to_end();
        Ok(Self::default())
    }
}

impl MessageWrite for ControlResetRoute { }

