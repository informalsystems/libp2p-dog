use crate::types::{RawTransaction, Transaction};

/// A general trait of transforming a [`RawTransaction`] into a [`Transaction`] and vice versa.
///
/// [`RawTransaction`] is obtained from the wire and the [`Transaction`] is used to
/// compute the [`crate::types::TransactionId`] of the transaction and is what is sent to the application.
///
/// The inbound/outbound transforms must be inverses. Applying the inbound transform and then the
/// outbound transform MUST leave the underlying data un-modified.
///
/// By default, this is the identity transform for all fields in [`Transaction`].
pub trait DataTransform {
    /// Takes a [`RawTransaction`] received and converts it to a [`Transaction`].
    fn inbound_transform(
        &self,
        raw_transaction: RawTransaction,
    ) -> Result<Transaction, std::io::Error>;

    /// Takes and transforms the data to be published. The transformed data will then be used
    /// to create a [`RawTransaction`] to be sent to peers.
    fn outbound_transform(&self, data: Vec<u8>) -> Result<Vec<u8>, std::io::Error>;
}

/// The default transform, the raw data is propagated as is to the application layer dog.
#[derive(Default, Clone)]
pub struct IdentityTransform;

impl DataTransform for IdentityTransform {
    fn inbound_transform(
        &self,
        raw_transaction: RawTransaction,
    ) -> Result<Transaction, std::io::Error> {
        Ok(Transaction {
            from: raw_transaction.from,
            seqno: raw_transaction.seqno,
            data: raw_transaction.data,
        })
    }

    fn outbound_transform(&self, data: Vec<u8>) -> Result<Vec<u8>, std::io::Error> {
        Ok(data)
    }
}
