static SERVICE: &[u8] = b"The admin service!";
async fn admin_conn(
    _: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    Ok(Response::new(Full::new(Bytes::from(SERVICE))))
}
