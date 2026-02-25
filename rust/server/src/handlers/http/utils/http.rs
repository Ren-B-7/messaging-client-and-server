use tracing::info;
/// Returns true only when the request arrived over a secure (HTTPS) connection.
///
/// Checks, in order:
///   1. `X-Forwarded-Proto: https`   — set by most reverse proxies (nginx, Caddy, etc.)
///   2. `X-Forwarded-Ssl: on`        — Apache-style variant
///   3. The request URI scheme is literally "https"
///
/// Falls back to `false` so that plain HTTP dev servers work out of the box
/// without any configuration change.
pub fn is_https(req: &hyper::Request<impl hyper::body::Body>) -> bool {
    // 1. Standard proxy header
    if req
        .headers()
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("https"))
        .unwrap_or(false)
    {
        return true;
    }

    // 2. Apache-style proxy header
    if req
        .headers()
        .get("x-forwarded-ssl")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("on"))
        .unwrap_or(false)
    {
        return true;
    }

    // 3. URI scheme (only present when using an absolute-form request URI)
    info!("{:?}", req.uri().to_string());
    req.uri()
        .scheme()
        .map(|s| s.as_str() == "https")
        .unwrap_or(false)
}
