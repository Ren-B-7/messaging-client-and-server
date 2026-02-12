use bytes::Bytes;
use http_body_util::{BodyExt, Empty, Full, combinators::BoxBody};
use hyper::{Request, Response};
static SERVICE: &[u8] = b"The admin service!";
pub async fn admin_conn(
    _: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    Ok(Response::new(Full::new(Bytes::from(SERVICE))))
}
