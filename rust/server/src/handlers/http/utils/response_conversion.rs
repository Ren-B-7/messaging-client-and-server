use anyhow::Result;
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use hyper::Response;
use std::convert::Infallible;

/// Helper to convert Full<Bytes> response to BoxBody<Bytes, Infallible>
/// 
/// This is useful for converting between different body types in the Hyper ecosystem.
/// BoxBody provides type erasure and allows different body types to be used interchangeably.
pub fn convert_response_body(
    response: Response<http_body_util::Full<Bytes>>,
) -> Response<BoxBody<Bytes, Infallible>> {
    let (parts, body) = response.into_parts();
    let boxed_body: BoxBody<Bytes, Infallible> = http_body_util::BodyExt::boxed(body);
    Response::from_parts(parts, boxed_body)
}

/// Helper to convert Result with Full<Bytes> body to Result with BoxBody<Bytes, Infallible>
/// 
/// This is a convenience wrapper around `convert_response_body` that works with Results.
/// Useful for converting handler return types that return Result<Response<Full<Bytes>>>.
pub fn convert_result_body(
    result: Result<Response<http_body_util::Full<Bytes>>>,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    result.map(convert_response_body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::StatusCode;

    #[test]
    fn test_convert_response_body() {
        let response = Response::builder()
            .status(StatusCode::OK)
            .body(http_body_util::Full::new(Bytes::from("test")))
            .unwrap();

        let converted = convert_response_body(response);
        assert_eq!(converted.status(), StatusCode::OK);
    }

    #[test]
    fn test_convert_result_body() {
        let response = Response::builder()
            .status(StatusCode::OK)
            .body(http_body_util::Full::new(Bytes::from("test")))
            .unwrap();

        let result = Ok(response);
        let converted = convert_result_body(result);
        assert!(converted.is_ok());
        assert_eq!(converted.unwrap().status(), StatusCode::OK);
    }
}
