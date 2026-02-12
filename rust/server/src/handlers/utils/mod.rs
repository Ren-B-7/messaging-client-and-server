pub mod deliver_page;
pub mod headers;
pub mod upgrade;

pub use deliver_page::{
    deliver_error_page, deliver_html_page, deliver_html_page_with_status, deliver_json,
    deliver_redirect, deliver_text,
};

pub use headers::{
    accepts_content_type, add_cors_headers, add_security_headers, create_persistent_cookie,
    create_session_cookie, delete_cookie, get_basic_auth, get_bearer_token, get_client_ip,
    get_cookie, get_header_value, get_user_agent, header_matches, set_cookie,
};

pub use upgrade::{
    accept_upgrade, get_upgrade_protocol, handle_custom_upgrade, handle_websocket_upgrade,
    is_upgrade_request, reject_upgrade,
};
