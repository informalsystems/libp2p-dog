use libp2p::identity::SigningError;

#[derive(Debug)]
pub enum PublishError {
    /// This transaction has already been published.
    Duplicate,
    /// An error occurred while signing the transaction.
    SigningError(SigningError),
    /// There were no peers to send this transaction to.
    InsufficientPeers,
    /// The overal transaction was too large.
    TransactionTooLarge,
    /// the compression algorithm failed.
    TransformFailed(std::io::Error),
    /// Transaction could not be sent because the queues for all peers were full. The usize represents
    /// the number of peers that were attempted.
    AllQueuesFull(usize),
}

impl std::fmt::Display for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PublishError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TransformFailed(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PublishError {
    fn from(err: std::io::Error) -> Self {
        Self::TransformFailed(err)
    }
}

impl From<SigningError> for PublishError {
    fn from(error: SigningError) -> Self {
        PublishError::SigningError(error)
    }
}

#[derive(Debug)]
pub enum ValidationError {
    /// The PeerId was invalid.
    InvalidPeerId,
    /// The signature was invalid.
    InvalidSignature,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ValidationError {}
