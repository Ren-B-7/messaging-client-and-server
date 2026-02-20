#[allow(dead_code)]
pub mod deliver_page;
#[allow(dead_code)]
pub mod headers;
#[allow(dead_code)]
pub mod json_response;

// Re-export commonly used utilities
#[allow(unused_imports)]
pub use deliver_page::{
    deliver_html_page, deliver_page_with_etag, deliver_page_with_status, deliver_redirect,
    deliver_redirect_with_cookie, deliver_text, empty, full,
};
#[allow(unused_imports)]
pub use headers::{
    accepts_content_type, add_cache_headers_with_max_age, add_etag_header,
    add_last_modified_header, add_no_cache_headers, check_etag_match, check_if_modified_since,
    create_persistent_cookie, create_session_cookie, delete_cookie, get_basic_auth,
    get_bearer_token, get_client_ip, get_cookie, get_header_value, get_user_agent, header_matches,
    set_cookie,
};
#[allow(unused_imports)]
pub use json_response::{
    deliver_error_json, deliver_json, deliver_serialized_json, deliver_serialized_json_with_cookie,
    deliver_success_json,
};
