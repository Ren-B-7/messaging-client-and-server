static SERVICE: &[u8] = b"The user service!";
async fn user_conn(
    _: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    Ok(Response::new(Full::new(Bytes::from(SERVICE))))
}
