use super::{
    Decode,
    Encode,
    gossipsub::GossipsubMessageHandler,
    request_response::RequestResponseMessageHandler,
};

use std::{
    borrow::Cow,
    io,
    num::NonZeroU32,
};

#[derive(Clone, Default)]
pub struct PostcardCodec;

impl RequestResponseMessageHandler<PostcardCodec> {
    pub fn new(max_block_size: NonZeroU32) -> Self {
        Self {
            codec: PostcardCodec,
            max_response_size: max_block_size,
        }
    }
}

impl GossipsubMessageHandler<PostcardCodec> {
    pub fn new() -> Self {
        GossipsubMessageHandler {
            codec: PostcardCodec,
        }
    }
}

impl<T> Encode<T> for PostcardCodec
where
    T: ?Sized + serde::Serialize,
{
    type Encoder<'a>
        = Cow<'a, [u8]>
    where
        T: 'a;
    type Error = io::Error;

    fn encode<'a>(&self, value: &'a T) -> Result<Self::Encoder<'a>, Self::Error> {
        Ok(Cow::Owned(postcard::to_allocvec(value).map_err(|e| {
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?))
    }
}

impl<T> Decode<T> for PostcardCodec
where
    T: serde::de::DeserializeOwned,
{
    type Error = io::Error;

    fn decode(&self, bytes: &[u8]) -> Result<T, Self::Error> {
        postcard::from_bytes(bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use fuel_core_types::{
        blockchain::SealedBlockHeader,
        fuel_tx::Transaction,
        services::p2p::NetworkableTransactionPool,
    };
    use libp2p::request_response::Codec;

    use super::*;
    use crate::{
        codecs::request_response::RequestResponseMessageHandler,
        request_response::{
            messages::{
                RequestMessage,
                ResponseMessageErrorCode,
                V1ResponseMessage,
                V2ResponseMessage,
            },
            protocols::RequestResponseProtocol,
        },
    };

    const MAX_REQUEST_SIZE: NonZeroU32 = NonZeroU32::new(1024).unwrap();

    #[test]
    fn test_request_size_fits() {
        let arbitrary_range = 2..6;
        let m = RequestMessage::Transactions(arbitrary_range);
        assert!(
            postcard::to_stdvec(&m).unwrap().len() <= MAX_REQUEST_SIZE.get() as usize
        );
    }

    #[tokio::test]
    async fn codec__serialization_roundtrip_using_v2_on_successful_response_returns_original_value__sealed_headers()
     {
        // Given
        let sealed_block_headers = vec![SealedBlockHeader::default()];
        let response = V2ResponseMessage::SealedHeaders(Ok(sealed_block_headers.clone()));
        let mut codec: RequestResponseMessageHandler<PostcardCodec> =
            RequestResponseMessageHandler::new(MAX_REQUEST_SIZE);
        let mut buf = Vec::with_capacity(1024);

        // When
        codec
            .write_response(&RequestResponseProtocol::V2, &mut buf, response)
            .await
            .expect("Valid Vec<SealedBlockHeader> should be serialized using v1");

        let deserialized = codec
            .read_response(&RequestResponseProtocol::V2, &mut buf.as_slice())
            .await
            .expect("Valid Vec<SealedBlockHeader> should be deserialized using v1");

        // Then
        assert!(matches!(
            deserialized,
            V2ResponseMessage::SealedHeaders(Ok(sealed_headers)) if sealed_headers == sealed_block_headers
        ));
    }

    #[tokio::test]
    async fn codec__serialization_roundtrip_using_v2_on_successful_response_returns_original_value__full_transactions()
     {
        // Given
        let full_transactions = vec![Some(NetworkableTransactionPool::Transaction(
            Transaction::default_test_tx(),
        ))];
        let response =
            V2ResponseMessage::TxPoolFullTransactions(Ok(full_transactions.clone()));
        let mut codec: RequestResponseMessageHandler<PostcardCodec> =
            RequestResponseMessageHandler::new(MAX_REQUEST_SIZE);
        let mut buf = Vec::with_capacity(1024);

        // When
        codec
            .write_response(&RequestResponseProtocol::V2, &mut buf, response)
            .await
            .expect("Valid full transactions should be serialized using v2");

        let deserialized = codec
            .read_response(&RequestResponseProtocol::V2, &mut buf.as_slice())
            .await
            .expect("Valid full transactions should be deserialized using v2");

        // Then
        assert!(matches!(
            deserialized,
            V2ResponseMessage::TxPoolFullTransactions(Ok(actual)) if actual == full_transactions
        ));
    }

    #[tokio::test]
    async fn codec__serialization_roundtrip_using_v1_on_successful_response_returns_original_value()
     {
        // Given
        let sealed_block_headers = vec![SealedBlockHeader::default()];
        let response = V2ResponseMessage::SealedHeaders(Ok(sealed_block_headers.clone()));
        let mut codec: RequestResponseMessageHandler<PostcardCodec> =
            RequestResponseMessageHandler::new(MAX_REQUEST_SIZE);
        let mut buf = Vec::with_capacity(1024);

        // When
        codec
            .write_response(&RequestResponseProtocol::V1, &mut buf, response)
            .await
            .expect("Valid Vec<SealedBlockHeader> should be serialized using v1");

        let deserialized = codec
            .read_response(&RequestResponseProtocol::V1, &mut buf.as_slice())
            .await
            .expect("Valid Vec<SealedBlockHeader> should be deserialized using v1");

        // Then
        assert!(
            matches!(deserialized, V2ResponseMessage::SealedHeaders(Ok(sealed_headers)) if sealed_headers == sealed_block_headers)
        );
    }

    #[tokio::test]
    async fn codec__serialization_roundtrip_using_v2_on_error_response_returns_original_value()
     {
        // Given
        let response = V2ResponseMessage::SealedHeaders(Err(
            ResponseMessageErrorCode::ProtocolV1EmptyResponse,
        ));
        let mut codec: RequestResponseMessageHandler<PostcardCodec> =
            RequestResponseMessageHandler::new(MAX_REQUEST_SIZE);
        let mut buf = Vec::with_capacity(1024);

        // When
        codec
            .write_response(&RequestResponseProtocol::V2, &mut buf, response.clone())
            .await
            .expect("Valid Vec<SealedBlockHeader> is serialized using v1");

        let deserialized = codec
            .read_response(&RequestResponseProtocol::V2, &mut buf.as_slice())
            .await
            .expect("Valid Vec<SealedBlockHeader> is deserialized using v1");

        // Then
        assert!(matches!(
            deserialized,
            V2ResponseMessage::SealedHeaders(Err(
                ResponseMessageErrorCode::ProtocolV1EmptyResponse
            ))
        ));
    }

    #[tokio::test]
    async fn codec__serialization_roundtrip_using_v1_on_error_response_returns_predefined_error_code()
     {
        // Given
        let response = V2ResponseMessage::SealedHeaders(Err(
            ResponseMessageErrorCode::RequestedRangeTooLarge,
        ));
        let mut codec: RequestResponseMessageHandler<PostcardCodec> =
            RequestResponseMessageHandler::new(MAX_REQUEST_SIZE);
        let mut buf = Vec::with_capacity(1024);

        // When
        codec
            .write_response(&RequestResponseProtocol::V1, &mut buf, response.clone())
            .await
            .expect("Valid Vec<SealedBlockHeader> is serialized using v1");

        let deserialized = codec
            .read_response(&RequestResponseProtocol::V1, &mut buf.as_slice())
            .await
            .expect("Valid Vec<SealedBlockHeader> is deserialized using v1");

        // Then
        assert!(matches!(
            deserialized,
            V2ResponseMessage::SealedHeaders(Err(
                ResponseMessageErrorCode::ProtocolV1EmptyResponse
            ))
        ));
    }

    #[tokio::test]
    async fn codec__write_response_is_backwards_compatible_with_v1() {
        // Given
        let response = V2ResponseMessage::SealedHeaders(Err(
            ResponseMessageErrorCode::ProtocolV1EmptyResponse,
        ));
        let mut codec: RequestResponseMessageHandler<PostcardCodec> =
            RequestResponseMessageHandler::new(MAX_REQUEST_SIZE);
        let mut buf = Vec::with_capacity(1024);

        // When
        codec
            .write_response(&RequestResponseProtocol::V1, &mut buf, response.clone())
            .await
            .expect("Valid Vec<SealedBlockHeader> is serialized using v1");

        let deserialized_as_v1 =
            // We cannot access the codec trait from an old node here, 
            // so we deserialize directly using the `V1ResponseMessage` type.
            codec.codec.decode(&buf).expect("Deserialization as V1ResponseMessage should succeed");

        // Then
        assert!(matches!(
            deserialized_as_v1,
            V1ResponseMessage::SealedHeaders(None)
        ));
    }

    #[tokio::test]
    async fn codec__read_response_is_backwards_compatible_with_v1() {
        // Given
        let response = V1ResponseMessage::SealedHeaders(None);
        let mut codec: RequestResponseMessageHandler<PostcardCodec> =
            RequestResponseMessageHandler::new(MAX_REQUEST_SIZE);

        // When
        let buf = codec
            .codec
            .encode(&response)
            .expect("Serialization as V1ResponseMessage should succeed");
        let deserialized = codec
            .read_response(&RequestResponseProtocol::V1, &mut &*buf)
            .await
            .expect("Valid Vec<SealedBlockHeader> is deserialized using v1");

        // Then
        assert!(matches!(
            deserialized,
            V2ResponseMessage::SealedHeaders(Err(
                ResponseMessageErrorCode::ProtocolV1EmptyResponse
            ))
        ));
    }
}
