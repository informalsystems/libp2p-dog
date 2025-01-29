use futures_timer::Delay;
use libp2p::{identity::ParseError, swarm::ConnectionId, PeerId};
use quick_protobuf::MessageWrite;

use crate::{rpc::Sender, rpc_proto::proto};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransactionId(pub Vec<u8>);

impl TransactionId {
    pub fn new(value: &[u8]) -> Self {
        Self(value.to_vec())
    }
}

impl<T: Into<Vec<u8>>> From<T> for TransactionId {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl std::fmt::Display for TransactionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex_fmt::HexFmt(&self.0))
    }
}

impl std::fmt::Debug for TransactionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TransactionId({})", hex_fmt::HexFmt(&self.0))
    }
}

#[derive(Debug)]
pub(crate) struct PeerConnections {
    /// The peer's connections.
    pub(crate) connections: Vec<ConnectionId>,
    /// The rpc sender to the connection handler(s).
    pub(crate) sender: Sender,
}

/// A transaction received by the dog system.
#[derive(Debug, Clone)]
pub struct RawTransaction {
    /// The peer that published the transaction.
    pub from: PeerId,
    /// The sequence number of the transaction.
    pub seqno: u64,
    /// The content of the transaction.
    pub data: Vec<u8>,

    /// FIELDS BELOW ARE NOT PART OF THE SIGNATURE

    /// The signature of the transaction if it is signed.
    pub signature: Option<Vec<u8>>,
    /// The public key of the transaction if it is signed.
    pub key: Option<Vec<u8>>,
}

impl RawTransaction {
    pub fn raw_protobuf_len(&self) -> usize {
        let transaction: proto::Transaction = self.clone().into();

        transaction.get_size()
    }
}

impl From<RawTransaction> for proto::Transaction {
    fn from(tx: RawTransaction) -> Self {
        proto::Transaction {
            from: tx.from.to_bytes(),
            seqno: tx.seqno,
            data: tx.data.to_vec(),
            signature: match tx.signature {
                Some(sig) => sig.to_vec(),
                None => vec![],
            },
            key: match tx.key {
                Some(key) => key.to_vec(),
                None => vec![],
            },
        }
    }
}

/// The transaction sent to the user after a [`RawTransaction`] has been transformed by a
/// [`crate::transform::DataTransform`].
#[derive(Clone)]
pub struct Transaction {
    /// The peer that published the transaction.
    pub from: PeerId,
    /// The sequence number of the transaction.
    pub seqno: u64,
    /// The content of the transaction.
    pub data: Vec<u8>,
}

impl std::fmt::Debug for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match String::from_utf8(self.data.clone()) {
            Ok(data) => write!(
                f,
                "Transaction {{ from: {}, seqno: {}, data: {} }}",
                self.from, self.seqno, data
            ),
            Err(_) => write!(
                f,
                "Transaction {{ from: {}, seqno: {}, data: {:?} }}",
                self.from, self.seqno, self.data
            ),
        }
    }
}

/// A control message received by the dog system.
#[derive(Debug)]
pub enum ControlAction {
    /// Node requests the local node to stop routing transactions originating from a specific peer
    /// (identified by the transaction id) to the requesting node.
    HaveTx(HaveTx),
    /// Node requests the local node to re-open a closed route to the requesting node.
    ResetRoute(ResetRoute),
}

#[derive(Debug, Clone)]
pub struct HaveTx {
    pub tx_id: TransactionId,
}

impl TryFrom<proto::ControlHaveTx> for HaveTx {
    type Error = ParseError;

    fn try_from(have_tx: proto::ControlHaveTx) -> Result<Self, Self::Error> {
        Ok(HaveTx {
            tx_id: TransactionId::new(&have_tx.tx_id),
        })
    }
}

impl From<HaveTx> for proto::ControlHaveTx {
    fn from(have_tx: HaveTx) -> Self {
        proto::ControlHaveTx {
            tx_id: have_tx.tx_id.0,
        }
    }
}

impl std::fmt::Display for HaveTx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HaveTx {{ tx_id: {} }}", self.tx_id)
    }
}

#[derive(Debug, Clone)]
pub struct ResetRoute {}

impl From<ResetRoute> for proto::ControlResetRoute {
    fn from(_: ResetRoute) -> Self {
        proto::ControlResetRoute {}
    }
}

impl std::fmt::Display for ResetRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ResetRoute {{}}")
    }
}

/// A dog RPC transaction sent.
#[derive(Debug)]
pub enum RpcOut {
    /// Publish a dog transaction on the network. `timeout` limits the duration the transaction
    /// can wait to be sent before it is abandoned.
    Publish { tx: RawTransaction, timeout: Delay },
    /// Forward a dog transaction on the network. `timeout` limits the duration the transaction
    /// can wait to be sent before it is abandoned.
    Forward { tx: RawTransaction, timeout: Delay },
    /// Send a HaveTx control message.
    HaveTx(HaveTx),
    /// Send a ResetRoute control message.
    ResetRoute(ResetRoute),
}

impl RpcOut {
    pub fn into_protobuf(self) -> proto::RPC {
        self.into()
    }
}

impl From<RpcOut> for proto::RPC {
    fn from(rpc: RpcOut) -> Self {
        match rpc {
            RpcOut::Publish { tx, timeout: _ } => proto::RPC {
                txs: vec![tx.into()],
                control: None,
            },
            RpcOut::Forward { tx, timeout: _ } => proto::RPC {
                txs: vec![tx.into()],
                control: None,
            },
            RpcOut::HaveTx(have_tx) => proto::RPC {
                txs: vec![],
                control: Some(proto::ControlMessage {
                    have_tx: vec![have_tx.into()],
                    reset_route: vec![],
                }),
            },
            RpcOut::ResetRoute(reset_route) => proto::RPC {
                txs: vec![],
                control: Some(proto::ControlMessage {
                    have_tx: vec![],
                    reset_route: vec![reset_route.into()],
                }),
            },
        }
    }
}

impl std::fmt::Display for RpcOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RpcOut::Publish { tx, .. } => {
                write!(f, "Publish {{ from: {}, seqno: {} }}", tx.from, tx.seqno)
            }
            RpcOut::Forward { tx, .. } => {
                write!(f, "Forward {{ from: {}, seqno: {} }}", tx.from, tx.seqno)
            }
            RpcOut::HaveTx(have_tx) => write!(f, "HaveTx {{ have_tx: {} }}", have_tx),
            RpcOut::ResetRoute(reset_route) => {
                write!(f, "ResetRoute {{ reset_route: {} }}", reset_route)
            }
        }
    }
}

/// An RPC received/sent.
#[derive(Debug)]
pub struct Rpc {
    /// List of transaction sthat were part of this RPC query.
    pub transactions: Vec<RawTransaction>,
    /// List of dog control messages.
    pub control_msgs: Vec<ControlAction>,
}

impl Rpc {
    pub fn into_protobuf(self) -> proto::RPC {
        self.into()
    }
}

impl From<Rpc> for proto::RPC {
    fn from(rpc: Rpc) -> Self {
        proto::RPC {
            txs: rpc.transactions.into_iter().map(Into::into).collect(),
            control: Some(proto::ControlMessage {
                have_tx: rpc
                    .control_msgs
                    .iter()
                    .filter_map(|msg| match msg {
                        ControlAction::HaveTx(have_tx) => Some((*have_tx).clone().into()),
                        _ => None,
                    })
                    .collect(),
                reset_route: rpc
                    .control_msgs
                    .iter()
                    .filter_map(|msg| match msg {
                        ControlAction::ResetRoute(reset_route) => {
                            Some((*reset_route).clone().into())
                        }
                        _ => None,
                    })
                    .collect(),
            }),
        }
    }
}
