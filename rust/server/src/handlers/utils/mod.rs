pub mod deliver_page;
pub mod error_response;
pub mod headers;
pub mod upgrade;

// Re-export delivery functions
pub use deliver_page::{
    deliver_error_page, deliver_html_page, deliver_html_file, deliver_html_page_with_status,
    deliver_page_with_status, deliver_static_page_with_status, deliver_static_page_with_etag,
    deliver_json, deliver_redirect, deliver_text, empty, full,
};

// Re-export error response utilities
pub use error_response::{ErrorResponse, deliver_error_json};

// Re-export header utilities
pub use headers::{
    accepts_content_type, add_cors_headers, add_security_headers,
    add_static_cache_headers, add_no_cache_headers, add_cache_headers_with_max_age,
    add_etag_header, check_etag_match, add_last_modified_header, check_if_modified_since,
    create_persistent_cookie, create_session_cookie, delete_cookie,
    get_basic_auth, get_bearer_token, get_client_ip, get_cookie,
    get_header_value, get_user_agent, header_matches, set_cookie,
};

// Re-export upgrade utilities
pub use upgrade::{
    accept_upgrade, get_upgrade_protocol, handle_custom_upgrade, handle_websocket_upgrade,
    is_upgrade_request, reject_upgrade,
};
