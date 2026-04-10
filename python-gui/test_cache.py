from cache import MessageCache, HTTPCache


def test_message_cache():
    cache = MessageCache(limit=2)
    cache.add_message("1", {"msg": "hello"})
    cache.add_message("1", {"msg": "world"})
    cache.add_message("1", {"msg": "extra"})  # Should evict oldest

    msgs = cache.get_messages("1")
    assert len(msgs) == 2
    assert msgs[0]["msg"] == "world"
    assert msgs[1]["msg"] == "extra"


def test_http_cache():
    cache = HTTPCache(max_size=1)
    cache.set("key1", "data1")
    assert cache.get("key1") == "data1"

    cache.set("key2", "data2")  # Should evict key1
    assert cache.get("key1") is None
    assert cache.get("key2") == "data2"
