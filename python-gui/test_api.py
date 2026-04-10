import pytest
from api import ChatAPIClient
from config import DEFAULT_SERVER

def test_api_client_init():
    client = ChatAPIClient(DEFAULT_SERVER)
    assert client.server_url == DEFAULT_SERVER
    assert "api" in client.api_base

def test_rate_limiter():
    from api import RateLimiter
    import time
    limiter = RateLimiter(max_requests=2, window_seconds=1)
    assert limiter.try_acquire() == True
    assert limiter.try_acquire() == True
    assert limiter.try_acquire() == False
