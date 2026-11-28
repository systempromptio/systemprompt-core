"""Stream encoders for the two model wire protocols.

The recording relay uses them to answer the session-title call, which the
scripted instance has no way to script, and the stub under tests/ uses them to
stand in for the scripted instance where its binary is not built.
"""

import json

MESSAGES = "messages"
CHAT = "chat"


def protocol_of(path):
    """The wire protocol a request path belongs to, or None."""
    if path.endswith("/messages"):
        return MESSAGES
    if path.endswith("/chat/completions"):
        return CHAT
    return None


def _event(name, payload):
    return "event: %s\ndata: %s\n\n" % (name, json.dumps(payload, separators=(",", ":")))


def _data(payload):
    return "data: %s\n\n" % json.dumps(payload, separators=(",", ":"))


def messages_frames(model, sequence, respond, stop, usage):
    """Messages frames for one turn: (body frames, terminal frames)."""
    body = [
        _event(
            "message_start",
            {
                "type": "message_start",
                "message": {
                    "id": "msg_scripted_%d" % sequence,
                    "type": "message",
                    "role": "assistant",
                    "model": model,
                    "content": [],
                    "stop_reason": None,
                    "stop_sequence": None,
                    "usage": {"input_tokens": usage["input"], "output_tokens": 0},
                },
            },
        )
    ]
    for index, item in enumerate(respond):
        if "text" in item:
            body.append(
                _event(
                    "content_block_start",
                    {"type": "content_block_start", "index": index, "content_block": {"type": "text", "text": ""}},
                )
            )
            body.append(
                _event(
                    "content_block_delta",
                    {"type": "content_block_delta", "index": index, "delta": {"type": "text_delta", "text": item["text"]}},
                )
            )
        else:
            call = item["tool_call"]
            body.append(
                _event(
                    "content_block_start",
                    {
                        "type": "content_block_start",
                        "index": index,
                        "content_block": {"type": "tool_use", "id": call["id"], "name": call["name"], "input": {}},
                    },
                )
            )
            for chunk in argument_chunks(call):
                body.append(
                    _event(
                        "content_block_delta",
                        {"type": "content_block_delta", "index": index, "delta": {"type": "input_json_delta", "partial_json": chunk}},
                    )
                )
        body.append(_event("content_block_stop", {"type": "content_block_stop", "index": index}))
    terminal = [
        _event(
            "message_delta",
            {
                "type": "message_delta",
                "delta": {"stop_reason": "tool_use" if stop == "tool_use" else "end_turn", "stop_sequence": None},
                "usage": {"output_tokens": usage["output"]},
            },
        ),
        _event("message_stop", {"type": "message_stop"}),
    ]
    return body, terminal


def chat_frames(model, sequence, respond, stop, usage):
    """Chat Completions frames for one turn: (body frames, terminal frames)."""
    base = {"id": "chatcmpl-scripted-%d" % sequence, "object": "chat.completion.chunk", "created": 0, "model": model}

    def chunk(delta, finish=None):
        return _data(dict(base, choices=[{"index": 0, "delta": delta, "finish_reason": finish}]))

    body = [chunk({"role": "assistant", "content": ""})]
    tool_index = 0
    for item in respond:
        if "text" in item:
            body.append(chunk({"content": item["text"]}))
            continue
        call = item["tool_call"]
        body.append(
            chunk(
                {
                    "tool_calls": [
                        {"index": tool_index, "id": call["id"], "type": "function", "function": {"name": call["name"], "arguments": ""}}
                    ]
                }
            )
        )
        for part in argument_chunks(call):
            body.append(chunk({"tool_calls": [{"index": tool_index, "function": {"arguments": part}}]}))
        tool_index += 1
    total = usage["input"] + usage["output"]
    terminal = [
        chunk({}, "tool_calls" if stop == "tool_use" else "stop"),
        _data(
            dict(
                base,
                choices=[],
                usage={"prompt_tokens": usage["input"], "completion_tokens": usage["output"], "total_tokens": total},
            )
        ),
        "data: [DONE]\n\n",
    ]
    return body, terminal


def argument_chunks(call):
    chunks = call.get("argument_chunks") or []
    if chunks:
        return list(chunks)
    return [json.dumps(call["input"], separators=(",", ":"))]


def frames(protocol, model, sequence, respond, stop=None, usage=None):
    if stop is None:
        stop = "tool_use" if any("tool_call" in item for item in respond) else "end"
    usage = usage or {"input": 0, "output": 0}
    encode = messages_frames if protocol == MESSAGES else chat_frames
    return encode(model, sequence, respond, stop, usage)


def error_body(protocol, status, message):
    if protocol == MESSAGES:
        kind = {401: "authentication_error", 429: "rate_limit_error", 529: "overloaded_error"}.get(status, "api_error")
        return {"type": "error", "error": {"type": kind, "message": message}}
    return {"error": {"message": message, "type": "scripted_error", "code": status}}
